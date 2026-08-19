use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;

use ai_voice_capability::{
    Architecture, CAPABILITY_SCHEMA_VERSION, CapabilityAvailability, EngineIdentity,
    NativeRuntimeCapabilities, Platform, RuntimeBackend,
};
use ai_voice_config::{
    BackendKind, MAX_MOCK_WORK_ITERATIONS, MAX_RUNTIME_IN_FLIGHT, MAX_RUNTIME_QUEUE_CAPACITY,
    MAX_RUNTIME_TIMEOUT_MS, MAX_STDERR_TAIL_BYTES, ProductConfig,
};
use ai_voice_contracts::runtime_message::{
    Command, Decoder, MAX_PING_PAYLOAD_BYTES, MessageKind, RuntimeMessage, encode,
};
use ai_voice_contracts::{ErrorCode, IPC_PROTOCOL_CURRENT_VERSION, RUNTIME_VERSION};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Instant, MissedTickBehavior, timeout};

use crate::payload::{
    Hello, MockPipelineSummary, PayloadError, parse_capabilities, parse_hello,
    parse_mock_pipeline_summary,
};

const MAX_PATH_UNITS: usize = 4_096;
const MAX_TIMEOUT: Duration = Duration::from_millis(MAX_RUNTIME_TIMEOUT_MS as u64);
const STDOUT_READ_BYTES: usize = 2_048;
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(25);
const PIPE_DRAIN_TIMEOUT: Duration = Duration::from_millis(100);
const MAX_PIPE_DRAIN_EVENTS: usize = 64;
const REAP_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeState {
    Stopped,
    Starting,
    Connected,
    Stopping,
    Crashed,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeExitReason {
    RequestedShutdown,
    UnexpectedExit,
    ProtocolFailure,
    Timeout,
    ProcessFailure,
    ManagerDropped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExit {
    pub reason: RuntimeExitReason,
    pub code: Option<i32>,
    pub success: bool,
}

impl RuntimeExit {
    #[cfg(test)]
    fn requested(code: i32) -> Self {
        Self {
            reason: RuntimeExitReason::RequestedShutdown,
            code: Some(code),
            success: code == 0,
        }
    }

    #[cfg(test)]
    fn unexpected(code: Option<i32>) -> Self {
        Self {
            reason: RuntimeExitReason::UnexpectedExit,
            code,
            success: false,
        }
    }

    fn from_status(reason: RuntimeExitReason, status: Option<ExitStatus>) -> Self {
        Self {
            reason,
            code: status.and_then(|value| value.code()),
            success: status.is_some_and(|value| value.success()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagerErrorKind {
    InvalidConfiguration,
    InvalidState,
    Capacity,
    RequestIdExhausted,
    Timeout,
    Protocol,
    Process,
    Remote,
    Payload,
    ManagerClosed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagerError {
    pub code: ErrorCode,
    pub kind: ManagerErrorKind,
    pub message: String,
}

impl ManagerError {
    fn new(code: ErrorCode, kind: ManagerErrorKind, message: impl Into<String>) -> Self {
        Self {
            code,
            kind,
            message: message.into(),
        }
    }

    fn invalid_configuration(message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::InvalidArgument,
            ManagerErrorKind::InvalidConfiguration,
            message,
        )
    }

    fn invalid_state(message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::InvalidState,
            ManagerErrorKind::InvalidState,
            message,
        )
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::RuntimeUnavailable,
            ManagerErrorKind::InvalidState,
            message,
        )
    }

    fn timeout(message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::RuntimeUnavailable,
            ManagerErrorKind::Timeout,
            message,
        )
    }

    fn protocol(message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::MalformedFrame,
            ManagerErrorKind::Protocol,
            message,
        )
    }

    fn process(message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::RuntimeUnavailable,
            ManagerErrorKind::Process,
            message,
        )
    }

    fn payload(error: PayloadError, command: Command) -> Self {
        Self::new(
            ErrorCode::MalformedFrame,
            ManagerErrorKind::Payload,
            format!("invalid {command:?} payload: {error}"),
        )
    }

    fn closed() -> Self {
        Self::new(
            ErrorCode::RuntimeUnavailable,
            ManagerErrorKind::ManagerClosed,
            "RuntimeManager actor is closed",
        )
    }
}

impl fmt::Display for ManagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} ({}): {}",
            self.kind,
            self.code.value(),
            self.message
        )
    }
}

impl std::error::Error for ManagerError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityObservationKind {
    NotEvaluated,
    Inconclusive,
    Observed,
}

#[derive(Debug, Eq, PartialEq)]
enum CapabilityObservationOutcome {
    NotEvaluated,
    Inconclusive(ManagerError),
    Observed(NativeRuntimeCapabilities),
}

/// An opaque capability outcome stamped by the RuntimeManager actor.
///
/// There are intentionally no public constructors: callers may inspect an
/// outcome, but cannot attach copied data or a failed query to a generation.
#[derive(Debug, Eq, PartialEq)]
pub struct CapabilityObservation {
    generation: u64,
    outcome: CapabilityObservationOutcome,
}

impl CapabilityObservation {
    fn not_evaluated(generation: u64) -> Self {
        Self {
            generation,
            outcome: CapabilityObservationOutcome::NotEvaluated,
        }
    }

    fn inconclusive(generation: u64, error: ManagerError) -> Self {
        Self {
            generation,
            outcome: CapabilityObservationOutcome::Inconclusive(error),
        }
    }

    fn observed(generation: u64, capabilities: NativeRuntimeCapabilities) -> Self {
        Self {
            generation,
            outcome: CapabilityObservationOutcome::Observed(capabilities),
        }
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn kind(&self) -> CapabilityObservationKind {
        match self.outcome {
            CapabilityObservationOutcome::NotEvaluated => CapabilityObservationKind::NotEvaluated,
            CapabilityObservationOutcome::Inconclusive(_) => {
                CapabilityObservationKind::Inconclusive
            }
            CapabilityObservationOutcome::Observed(_) => CapabilityObservationKind::Observed,
        }
    }

    pub const fn capabilities(&self) -> Option<&NativeRuntimeCapabilities> {
        match &self.outcome {
            CapabilityObservationOutcome::Observed(capabilities) => Some(capabilities),
            CapabilityObservationOutcome::NotEvaluated
            | CapabilityObservationOutcome::Inconclusive(_) => None,
        }
    }

    pub const fn error(&self) -> Option<&ManagerError> {
        match &self.outcome {
            CapabilityObservationOutcome::Inconclusive(error) => Some(error),
            CapabilityObservationOutcome::NotEvaluated
            | CapabilityObservationOutcome::Observed(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagerHealth {
    Stopped,
    Starting,
    Connected,
    Stopping,
    Crashed,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ManagerCapability {
    health: ManagerHealth,
    generation: u64,
}

impl ManagerCapability {
    pub const fn health(&self) -> ManagerHealth {
        self.health
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityProfileError {
    InvalidManagerGeneration,
    StaleObservation {
        manager_generation: u64,
        observation_generation: u64,
    },
    ObservationNotAllowed {
        health: ManagerHealth,
    },
}

impl fmt::Display for CapabilityProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManagerGeneration => {
                formatter.write_str("non-stopped manager generation must be nonzero")
            }
            Self::StaleObservation {
                manager_generation,
                observation_generation,
            } => write!(
                formatter,
                "capability observation generation {observation_generation} does not match manager generation {manager_generation}"
            ),
            Self::ObservationNotAllowed { health } => write!(
                formatter,
                "capability query observation is invalid while manager is {health:?}"
            ),
        }
    }
}

impl std::error::Error for CapabilityProfileError {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeCapability {
    version: String,
    protocol_version: u32,
    backend: RuntimeBackend,
    availability: CapabilityAvailability,
}

impl RuntimeCapability {
    pub fn version(&self) -> &str {
        &self.version
    }

    pub const fn protocol_version(&self) -> u32 {
        self.protocol_version
    }

    pub const fn backend(&self) -> RuntimeBackend {
        self.backend
    }

    pub const fn availability(&self) -> CapabilityAvailability {
        self.availability
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EngineCapability {
    identity: Option<EngineIdentity>,
    availability: CapabilityAvailability,
}

impl EngineCapability {
    pub const fn identity(&self) -> Option<EngineIdentity> {
        self.identity
    }

    pub const fn availability(&self) -> CapabilityAvailability {
        self.availability
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityProfile {
    schema_version: u32,
    platform: Platform,
    architecture: Architecture,
    runtime: RuntimeCapability,
    engine: EngineCapability,
    manager: ManagerCapability,
}

impl CapabilityProfile {
    fn from_actor_observation(
        configured_backend: RuntimeBackend,
        health: ManagerHealth,
        generation: u64,
        observation: &CapabilityObservation,
    ) -> Result<Self, CapabilityProfileError> {
        if health != ManagerHealth::Stopped && generation == 0 {
            return Err(CapabilityProfileError::InvalidManagerGeneration);
        }
        if health != ManagerHealth::Connected
            && !matches!(
                observation.outcome,
                CapabilityObservationOutcome::NotEvaluated
            )
        {
            return Err(CapabilityProfileError::ObservationNotAllowed { health });
        }

        let configured_identity = match configured_backend {
            RuntimeBackend::Mock => Some(EngineIdentity::AivsMockV1),
            RuntimeBackend::Unavailable => None,
        };
        let (platform, architecture, backend, runtime_availability, identity, engine_availability) =
            match health {
                ManagerHealth::Starting => (
                    Platform::Unknown,
                    Architecture::Unknown,
                    configured_backend,
                    CapabilityAvailability::NotEvaluated,
                    configured_identity,
                    CapabilityAvailability::NotEvaluated,
                ),
                ManagerHealth::Connected => match &observation.outcome {
                    CapabilityObservationOutcome::NotEvaluated => (
                        Platform::Unknown,
                        Architecture::Unknown,
                        configured_backend,
                        CapabilityAvailability::Available,
                        configured_identity,
                        CapabilityAvailability::NotEvaluated,
                    ),
                    CapabilityObservationOutcome::Inconclusive(_) => (
                        Platform::Unknown,
                        Architecture::Unknown,
                        configured_backend,
                        CapabilityAvailability::Available,
                        configured_identity,
                        CapabilityAvailability::Unknown,
                    ),
                    CapabilityObservationOutcome::Observed(native) => (
                        native.platform(),
                        native.architecture(),
                        native.backend(),
                        CapabilityAvailability::Available,
                        native.engine_identity(),
                        if native.engine_identity().is_some() {
                            CapabilityAvailability::Available
                        } else {
                            CapabilityAvailability::Unavailable
                        },
                    ),
                },
                ManagerHealth::Stopped
                | ManagerHealth::Stopping
                | ManagerHealth::Crashed
                | ManagerHealth::Error => (
                    Platform::Unknown,
                    Architecture::Unknown,
                    configured_backend,
                    CapabilityAvailability::Unavailable,
                    configured_identity,
                    CapabilityAvailability::Unavailable,
                ),
            };

        Ok(Self {
            schema_version: CAPABILITY_SCHEMA_VERSION,
            platform,
            architecture,
            runtime: RuntimeCapability {
                version: RUNTIME_VERSION.to_owned(),
                protocol_version: IPC_PROTOCOL_CURRENT_VERSION,
                backend,
                availability: runtime_availability,
            },
            engine: EngineCapability {
                identity,
                availability: engine_availability,
            },
            manager: ManagerCapability { health, generation },
        })
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn platform(&self) -> Platform {
        self.platform
    }

    pub const fn architecture(&self) -> Architecture {
        self.architecture
    }

    pub const fn runtime(&self) -> &RuntimeCapability {
        &self.runtime
    }

    pub const fn engine(&self) -> &EngineCapability {
        &self.engine
    }

    pub const fn manager(&self) -> &ManagerCapability {
        &self.manager
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeManagerConfig {
    pub runtime_path: PathBuf,
    pub plugin_path: PathBuf,
    pub mock_work_iterations: u32,
    pub handshake_timeout: Duration,
    pub request_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub max_in_flight: usize,
    pub command_queue_capacity: usize,
    pub event_queue_capacity: usize,
    pub stderr_tail_bytes: usize,
    pub restart_max_attempts: u32,
}

impl RuntimeManagerConfig {
    pub fn new(runtime_path: PathBuf, plugin_path: PathBuf) -> Self {
        Self {
            runtime_path,
            plugin_path,
            mock_work_iterations: 0,
            handshake_timeout: Duration::from_secs(2),
            request_timeout: Duration::from_secs(2),
            shutdown_timeout: Duration::from_secs(2),
            max_in_flight: 16,
            command_queue_capacity: 32,
            event_queue_capacity: 32,
            stderr_tail_bytes: 8_192,
            restart_max_attempts: 0,
        }
    }

    pub fn from_product_config(
        product: &ProductConfig,
        runtime_path: PathBuf,
        plugin_path: PathBuf,
    ) -> Result<Self, ManagerError> {
        let mock_work_iterations = match product.backend().kind() {
            BackendKind::Mock => product.backend().mock().work_iterations(),
        };
        let config = Self {
            runtime_path,
            plugin_path,
            mock_work_iterations,
            handshake_timeout: Duration::from_millis(u64::from(
                product.runtime().handshake_timeout_ms(),
            )),
            request_timeout: Duration::from_millis(u64::from(
                product.runtime().request_timeout_ms(),
            )),
            shutdown_timeout: Duration::from_millis(u64::from(
                product.runtime().shutdown_timeout_ms(),
            )),
            max_in_flight: product.runtime().max_in_flight() as usize,
            command_queue_capacity: product.runtime().command_queue_capacity() as usize,
            event_queue_capacity: product.runtime().event_queue_capacity() as usize,
            stderr_tail_bytes: product.runtime().stderr_tail_bytes() as usize,
            restart_max_attempts: product.runtime().restart_max_attempts(),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn with_handshake_timeout(mut self, value: Duration) -> Self {
        self.handshake_timeout = value;
        self
    }

    pub fn with_request_timeout(mut self, value: Duration) -> Self {
        self.request_timeout = value;
        self
    }

    pub fn with_shutdown_timeout(mut self, value: Duration) -> Self {
        self.shutdown_timeout = value;
        self
    }

    fn validate(&self) -> Result<(), ManagerError> {
        validate_path(&self.runtime_path, "Runtime")?;
        validate_path(&self.plugin_path, "plugin")?;
        if self.mock_work_iterations > MAX_MOCK_WORK_ITERATIONS {
            return Err(ManagerError::invalid_configuration(
                "mock work iterations exceed the fixed limit",
            ));
        }
        for (name, value) in [
            ("handshake", self.handshake_timeout),
            ("request", self.request_timeout),
            ("shutdown", self.shutdown_timeout),
        ] {
            if value.is_zero() || value > MAX_TIMEOUT {
                return Err(ManagerError::invalid_configuration(format!(
                    "{name} timeout must be nonzero and at most 300 seconds"
                )));
            }
        }
        validate_nonzero_limit(
            "max in-flight requests",
            self.max_in_flight,
            MAX_RUNTIME_IN_FLIGHT as usize,
        )?;
        validate_nonzero_limit(
            "command queue capacity",
            self.command_queue_capacity,
            MAX_RUNTIME_QUEUE_CAPACITY as usize,
        )?;
        validate_nonzero_limit(
            "event queue capacity",
            self.event_queue_capacity,
            MAX_RUNTIME_QUEUE_CAPACITY as usize,
        )?;
        if self.stderr_tail_bytes > MAX_STDERR_TAIL_BYTES as usize {
            return Err(ManagerError::invalid_configuration(
                "stderr retention exceeds the fixed limit",
            ));
        }
        Ok(())
    }
}

fn validate_nonzero_limit(name: &str, value: usize, maximum: usize) -> Result<(), ManagerError> {
    if value == 0 || value > maximum {
        return Err(ManagerError::invalid_configuration(format!(
            "{name} must be between 1 and {maximum}"
        )));
    }
    Ok(())
}

fn validate_path(path: &Path, name: &str) -> Result<(), ManagerError> {
    if !path.is_absolute() {
        return Err(ManagerError::invalid_configuration(format!(
            "{name} path must be explicit and absolute"
        )));
    }
    let units = os_units(path.as_os_str());
    if units == 0 || units > MAX_PATH_UNITS || os_contains_nul(path.as_os_str()) {
        return Err(ManagerError::invalid_configuration(format!(
            "{name} path is empty, contains NUL, or exceeds the fixed limit"
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn os_units(value: &OsStr) -> usize {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes().len()
}

#[cfg(unix)]
fn os_contains_nul(value: &OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes().contains(&0)
}

#[cfg(windows)]
fn os_units(value: &OsStr) -> usize {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().count()
}

#[cfg(windows)]
fn os_contains_nul(value: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().any(|unit| unit == 0)
}

#[cfg(not(any(unix, windows)))]
fn os_units(value: &OsStr) -> usize {
    value.to_string_lossy().len()
}

#[cfg(not(any(unix, windows)))]
fn os_contains_nul(value: &OsStr) -> bool {
    value.to_string_lossy().contains('\0')
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestartPolicyStatus {
    pub automatic_restart: bool,
    pub max_attempts: u32,
    pub attempts_observed: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStatus {
    pub state: RuntimeState,
    pub generation: u64,
    pub pid: Option<u32>,
    pub hello: Option<Hello>,
    pub last_exit: Option<RuntimeExit>,
    pub last_error: Option<ManagerError>,
    pub stderr_tail: Vec<u8>,
    pub restart_policy: RestartPolicyStatus,
}

impl RuntimeStatus {
    pub fn capability_profile(
        &self,
        product: &ProductConfig,
        observation: &CapabilityObservation,
    ) -> Result<CapabilityProfile, CapabilityProfileError> {
        if self.generation != observation.generation {
            return Err(CapabilityProfileError::StaleObservation {
                manager_generation: self.generation,
                observation_generation: observation.generation,
            });
        }
        let configured_backend = match product.backend().kind() {
            BackendKind::Mock => RuntimeBackend::Mock,
        };
        CapabilityProfile::from_actor_observation(
            configured_backend,
            match self.state {
                RuntimeState::Stopped => ManagerHealth::Stopped,
                RuntimeState::Starting => ManagerHealth::Starting,
                RuntimeState::Connected => ManagerHealth::Connected,
                RuntimeState::Stopping => ManagerHealth::Stopping,
                RuntimeState::Crashed => ManagerHealth::Crashed,
                RuntimeState::Error => ManagerHealth::Error,
            },
            self.generation,
            observation,
        )
    }
}

#[derive(Clone)]
pub struct RuntimeManager {
    commands: mpsc::Sender<ActorCommand>,
}

impl RuntimeManager {
    pub fn new(config: RuntimeManagerConfig) -> Result<Self, ManagerError> {
        config.validate()?;
        tokio::runtime::Handle::try_current().map_err(|_| {
            ManagerError::invalid_configuration("RuntimeManager requires an active Tokio runtime")
        })?;
        let (commands, command_rx) = mpsc::channel(config.command_queue_capacity);
        let (event_tx, event_rx) = mpsc::channel(config.event_queue_capacity);
        tokio::spawn(Actor::new(config, command_rx, event_tx, event_rx).run());
        Ok(Self { commands })
    }

    pub async fn start_runtime(&self) -> Result<RuntimeStatus, ManagerError> {
        let (reply, receiver) = oneshot::channel();
        self.send(ActorCommand::Start { reply }).await?;
        receive(receiver).await?
    }

    pub async fn stop_runtime(&self) -> Result<RuntimeStatus, ManagerError> {
        let (reply, receiver) = oneshot::channel();
        self.send(ActorCommand::Stop { reply }).await?;
        receive(receiver).await?
    }

    pub async fn get_runtime_status(&self) -> Result<RuntimeStatus, ManagerError> {
        let (reply, receiver) = oneshot::channel();
        self.send(ActorCommand::Status { reply }).await?;
        receive(receiver).await
    }

    pub async fn ping(&self, payload: &[u8]) -> Result<Vec<u8>, ManagerError> {
        if payload.len() > MAX_PING_PAYLOAD_BYTES {
            return Err(ManagerError::new(
                ErrorCode::InvalidArgument,
                ManagerErrorKind::Payload,
                "Ping payload exceeds the fixed command limit",
            ));
        }
        self.request(Command::Ping, payload.to_vec())
            .await
            .map(|response| response.payload)
    }

    pub async fn get_capabilities(&self) -> Result<CapabilityObservation, ManagerError> {
        let (reply, receiver) = oneshot::channel();
        self.send(ActorCommand::ObserveCapabilities { reply })
            .await?;
        receive(receiver).await
    }

    pub async fn capabilities_not_evaluated(&self) -> Result<CapabilityObservation, ManagerError> {
        let (reply, receiver) = oneshot::channel();
        self.send(ActorCommand::CapabilitiesNotEvaluated { reply })
            .await?;
        receive(receiver).await
    }

    pub async fn run_mock_pipeline(&self) -> Result<MockPipelineSummary, ManagerError> {
        let response = self.request(Command::RunMockPipeline, Vec::new()).await?;
        parse_mock_pipeline_summary(&response.payload)
            .map_err(|error| ManagerError::payload(error, Command::RunMockPipeline))
    }

    async fn request(
        &self,
        command: Command,
        payload: Vec<u8>,
    ) -> Result<RuntimeResponse, ManagerError> {
        self.request_bound(command, payload)
            .await?
            .map_err(|failure| failure.error)
    }

    async fn request_bound(
        &self,
        command: Command,
        payload: Vec<u8>,
    ) -> Result<Result<RuntimeResponse, RuntimeRequestFailure>, ManagerError> {
        let (reply, receiver) = oneshot::channel();
        self.send(ActorCommand::Request {
            command,
            payload,
            reply,
        })
        .await?;
        receive(receiver).await
    }

    async fn send(&self, command: ActorCommand) -> Result<(), ManagerError> {
        self.commands
            .send(command)
            .await
            .map_err(|_| ManagerError::closed())
    }
}

async fn receive<T>(receiver: oneshot::Receiver<T>) -> Result<T, ManagerError> {
    receiver.await.map_err(|_| ManagerError::closed())
}

enum ActorCommand {
    Start {
        reply: oneshot::Sender<Result<RuntimeStatus, ManagerError>>,
    },
    Stop {
        reply: oneshot::Sender<Result<RuntimeStatus, ManagerError>>,
    },
    Status {
        reply: oneshot::Sender<RuntimeStatus>,
    },
    CapabilitiesNotEvaluated {
        reply: oneshot::Sender<CapabilityObservation>,
    },
    ObserveCapabilities {
        reply: oneshot::Sender<CapabilityObservation>,
    },
    Request {
        command: Command,
        payload: Vec<u8>,
        reply: RuntimeReply,
    },
}

#[derive(Debug, Eq, PartialEq)]
struct RuntimeResponse {
    generation: u64,
    payload: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq)]
struct RuntimeRequestFailure {
    generation: u64,
    error: ManagerError,
}

#[derive(Debug, Eq, PartialEq)]
enum CorrelationError {
    RequestIdExhausted,
    PendingLimit,
    DuplicateRequest,
    UnknownRequest,
    CommandMismatch,
}

struct RequestIds {
    next: u64,
}

impl RequestIds {
    const fn new() -> Self {
        Self { next: 1 }
    }

    fn allocate(&mut self) -> Result<u64, CorrelationError> {
        if self.next == 0 {
            return Err(CorrelationError::RequestIdExhausted);
        }
        let allocated = self.next;
        self.next = self.next.checked_add(1).unwrap_or(0);
        Ok(allocated)
    }
}

struct PendingRequest {
    generation: u64,
    command: Command,
    deadline: Instant,
    reply: PendingReply,
}

type RuntimeReply = oneshot::Sender<Result<RuntimeResponse, RuntimeRequestFailure>>;

enum PendingReply {
    Runtime(RuntimeReply),
    Capabilities(oneshot::Sender<CapabilityObservation>),
}
type StatusReply = oneshot::Sender<Result<RuntimeStatus, ManagerError>>;

struct PendingRequests {
    maximum: usize,
    entries: BTreeMap<u64, PendingRequest>,
}

impl PendingRequests {
    fn new(maximum: usize) -> Self {
        Self {
            maximum,
            entries: BTreeMap::new(),
        }
    }

    fn insert(
        &mut self,
        request_id: u64,
        generation: u64,
        command: Command,
        deadline: Instant,
        reply: PendingReply,
    ) -> Result<(), CorrelationError> {
        if self.entries.len() >= self.maximum {
            return Err(CorrelationError::PendingLimit);
        }
        if self.entries.contains_key(&request_id) {
            return Err(CorrelationError::DuplicateRequest);
        }
        self.entries.insert(
            request_id,
            PendingRequest {
                generation,
                command,
                deadline,
                reply,
            },
        );
        Ok(())
    }

    fn complete(
        &mut self,
        request_id: u64,
        command: Command,
        result: Result<RuntimeResponse, ManagerError>,
    ) -> Result<(), CorrelationError> {
        let pending = self
            .entries
            .get(&request_id)
            .ok_or(CorrelationError::UnknownRequest)?;
        if pending.command != command {
            return Err(CorrelationError::CommandMismatch);
        }
        let pending = self
            .entries
            .remove(&request_id)
            .expect("correlated pending request exists");
        pending.complete(result);
        Ok(())
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.entries.values().map(|pending| pending.deadline).min()
    }

    fn expired_ids(&self, now: Instant) -> Vec<u64> {
        self.entries
            .iter()
            .filter_map(|(id, pending)| (pending.deadline <= now).then_some(*id))
            .collect()
    }

    #[cfg(test)]
    fn fail_one(&mut self, request_id: u64, error: ManagerError) {
        if let Some(pending) = self.entries.remove(&request_id) {
            pending.fail(error);
        }
    }

    fn take(&mut self, request_id: u64) -> Option<PendingRequest> {
        self.entries.remove(&request_id)
    }

    fn fail_all(&mut self, error: ManagerError) {
        for (_, pending) in std::mem::take(&mut self.entries) {
            pending.fail(error.clone());
        }
    }
}

impl PendingRequest {
    fn complete(self, result: Result<RuntimeResponse, ManagerError>) {
        match self.reply {
            PendingReply::Runtime(reply) => {
                let result = result.map_err(|error| RuntimeRequestFailure {
                    generation: self.generation,
                    error,
                });
                let _ = reply.send(result);
            }
            PendingReply::Capabilities(reply) => {
                let observation = match result {
                    Ok(response) => match parse_capabilities(&response.payload) {
                        Ok(capabilities) => {
                            CapabilityObservation::observed(self.generation, capabilities)
                        }
                        Err(error) => CapabilityObservation::inconclusive(
                            self.generation,
                            ManagerError::payload(error, Command::GetCapabilities),
                        ),
                    },
                    Err(error) => CapabilityObservation::inconclusive(self.generation, error),
                };
                let _ = reply.send(observation);
            }
        }
    }

    fn fail(self, error: ManagerError) {
        self.complete(Err(error));
    }
}

struct StateModel {
    state: RuntimeState,
    generation: u64,
    pid: Option<u32>,
    hello: Option<Hello>,
    last_exit: Option<RuntimeExit>,
    last_error: Option<ManagerError>,
    stderr_tail: Vec<u8>,
    restart_policy: RestartPolicyStatus,
}

impl Default for StateModel {
    fn default() -> Self {
        Self {
            state: RuntimeState::Stopped,
            generation: 0,
            pid: None,
            hello: None,
            last_exit: None,
            last_error: None,
            stderr_tail: Vec::new(),
            restart_policy: RestartPolicyStatus {
                automatic_restart: false,
                max_attempts: 0,
                attempts_observed: 0,
            },
        }
    }
}

impl StateModel {
    fn with_restart_policy(max_attempts: u32) -> Self {
        Self {
            restart_policy: RestartPolicyStatus {
                max_attempts,
                ..Self::default().restart_policy
            },
            ..Self::default()
        }
    }

    fn begin_start(&mut self, generation: u64) -> Result<(), ManagerError> {
        if !matches!(
            self.state,
            RuntimeState::Stopped | RuntimeState::Crashed | RuntimeState::Error
        ) {
            return Err(ManagerError::invalid_state(
                "Runtime can only start from stopped, crashed, or error",
            ));
        }
        if generation == 0 || generation <= self.generation {
            return Err(ManagerError::new(
                ErrorCode::InternalError,
                ManagerErrorKind::RequestIdExhausted,
                "process generation exhausted",
            ));
        }
        if self.generation != 0 {
            self.restart_policy.attempts_observed =
                self.restart_policy.attempts_observed.saturating_add(1);
        }
        self.state = RuntimeState::Starting;
        self.generation = generation;
        self.pid = None;
        self.hello = None;
        self.last_error = None;
        self.stderr_tail.clear();
        Ok(())
    }

    fn connected(&mut self, pid: u32, hello: Hello) {
        self.state = RuntimeState::Connected;
        self.pid = Some(pid);
        self.hello = Some(hello);
    }

    fn begin_stop(&mut self) -> Result<(), ManagerError> {
        if self.state != RuntimeState::Connected {
            return Err(ManagerError::invalid_state(
                "Runtime can only request shutdown while connected",
            ));
        }
        self.state = RuntimeState::Stopping;
        Ok(())
    }

    fn stopped(&mut self, exit: RuntimeExit) {
        self.state = RuntimeState::Stopped;
        self.pid = None;
        self.last_exit = Some(exit);
    }

    fn crashed(&mut self, exit: RuntimeExit) {
        self.state = RuntimeState::Crashed;
        self.pid = None;
        self.last_exit = Some(exit);
    }

    fn failed(&mut self, error: ManagerError) {
        self.state = RuntimeState::Error;
        self.pid = None;
        self.last_error = Some(error);
    }

    fn snapshot(&self) -> RuntimeStatus {
        RuntimeStatus {
            state: self.state,
            generation: self.generation,
            pid: self.pid,
            hello: self.hello.clone(),
            last_exit: self.last_exit.clone(),
            last_error: self.last_error.clone(),
            stderr_tail: self.stderr_tail.clone(),
            restart_policy: self.restart_policy.clone(),
        }
    }
}

struct StderrTail {
    maximum: usize,
    bytes: Vec<u8>,
}

impl StderrTail {
    fn new(maximum: usize) -> Self {
        Self {
            maximum,
            bytes: Vec::with_capacity(maximum),
        }
    }

    fn extend(&mut self, bytes: &[u8]) {
        if self.maximum == 0 {
            return;
        }
        if bytes.len() >= self.maximum {
            self.bytes.clear();
            self.bytes
                .extend_from_slice(&bytes[bytes.len() - self.maximum..]);
            return;
        }
        let overflow = self
            .bytes
            .len()
            .saturating_add(bytes.len())
            .saturating_sub(self.maximum);
        if overflow != 0 {
            self.bytes.drain(..overflow);
        }
        self.bytes.extend_from_slice(bytes);
    }

    fn snapshot(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

enum ProcessEvent {
    Message {
        generation: u64,
        message: RuntimeMessage,
    },
    Eof {
        generation: u64,
        clean: bool,
    },
    Failure {
        generation: u64,
        error: ManagerError,
        reason: RuntimeExitReason,
    },
    WriteComplete {
        generation: u64,
        request_id: u64,
    },
    WriteFailure {
        generation: u64,
        request_id: u64,
        error: ManagerError,
    },
}

struct WriteCommand {
    generation: u64,
    request_id: u64,
    frame: Vec<u8>,
}

struct ProcessResources {
    child: Child,
    writer_tx: Option<mpsc::Sender<WriteCommand>>,
    writer_task: JoinHandle<()>,
    stdout_task: JoinHandle<()>,
    stderr_task: JoinHandle<()>,
    stderr_tail: Arc<Mutex<StderrTail>>,
    exit_status: Option<ExitStatus>,
    exit_drain_deadline: Option<Instant>,
}

struct StartContext {
    deadline: Instant,
    reply: oneshot::Sender<Result<RuntimeStatus, ManagerError>>,
}

struct StopContext {
    request_id: u64,
    deadline: Instant,
    response_received: bool,
    stdout_eof: bool,
    reply: oneshot::Sender<Result<RuntimeStatus, ManagerError>>,
}

struct Actor {
    config: RuntimeManagerConfig,
    commands: mpsc::Receiver<ActorCommand>,
    event_tx: mpsc::Sender<ProcessEvent>,
    events: mpsc::Receiver<ProcessEvent>,
    state: StateModel,
    ids: RequestIds,
    pending: PendingRequests,
    process: Option<ProcessResources>,
    start: Option<StartContext>,
    stop: Option<StopContext>,
}

impl Actor {
    fn new(
        config: RuntimeManagerConfig,
        commands: mpsc::Receiver<ActorCommand>,
        event_tx: mpsc::Sender<ProcessEvent>,
        events: mpsc::Receiver<ProcessEvent>,
    ) -> Self {
        Self {
            state: StateModel::with_restart_policy(config.restart_max_attempts),
            ids: RequestIds::new(),
            pending: PendingRequests::new(config.max_in_flight),
            config,
            commands,
            event_tx,
            events,
            process: None,
            start: None,
            stop: None,
        }
    }

    async fn run(mut self) {
        let mut exit_poll = tokio::time::interval(EXIT_POLL_INTERVAL);
        exit_poll.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            let deadline = self
                .next_deadline()
                .unwrap_or_else(|| Instant::now() + MAX_TIMEOUT);
            tokio::select! {
                biased;
                _ = tokio::time::sleep_until(deadline) => self.handle_deadlines().await,
                _ = exit_poll.tick() => self.poll_process_exit().await,
                event = self.events.recv() => {
                    if let Some(event) = event {
                        self.handle_event(event).await;
                    }
                }
                command = self.commands.recv() => {
                    let Some(command) = command else {
                        self.manager_dropped().await;
                        return;
                    };
                    self.handle_command(command).await;
                }
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        [
            self.start.as_ref().map(|start| start.deadline),
            self.stop.as_ref().map(|stop| stop.deadline),
            self.pending.next_deadline(),
            self.process
                .as_ref()
                .and_then(|process| process.exit_drain_deadline),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    async fn handle_command(&mut self, command: ActorCommand) {
        match command {
            ActorCommand::Start { reply } => self.start_runtime(reply).await,
            ActorCommand::Stop { reply } => self.stop_runtime(reply).await,
            ActorCommand::Status { reply } => {
                let _ = reply.send(self.status().await);
            }
            ActorCommand::CapabilitiesNotEvaluated { reply } => {
                let _ = reply.send(CapabilityObservation::not_evaluated(self.state.generation));
            }
            ActorCommand::ObserveCapabilities { reply } => {
                self.send_request(
                    Command::GetCapabilities,
                    Vec::new(),
                    PendingReply::Capabilities(reply),
                )
                .await;
            }
            ActorCommand::Request {
                command,
                payload,
                reply,
            } => {
                self.send_request(command, payload, PendingReply::Runtime(reply))
                    .await;
            }
        }
    }

    async fn start_runtime(&mut self, reply: oneshot::Sender<Result<RuntimeStatus, ManagerError>>) {
        let generation = match self.state.generation.checked_add(1) {
            Some(value) if value != 0 => value,
            _ => {
                let _ = reply.send(Err(ManagerError::new(
                    ErrorCode::InternalError,
                    ManagerErrorKind::RequestIdExhausted,
                    "process generation exhausted",
                )));
                return;
            }
        };
        if let Err(error) = self.state.begin_start(generation) {
            let _ = reply.send(Err(error));
            return;
        }
        match spawn_process(&self.config, generation, self.event_tx.clone()) {
            Ok(process) => {
                self.state.pid = process.child.id();
                self.process = Some(process);
                self.start = Some(StartContext {
                    deadline: Instant::now() + self.config.handshake_timeout,
                    reply,
                });
            }
            Err(error) => {
                self.state.failed(error.clone());
                let _ = reply.send(Err(error));
            }
        }
    }

    async fn stop_runtime(&mut self, reply: oneshot::Sender<Result<RuntimeStatus, ManagerError>>) {
        match self.state.state {
            RuntimeState::Stopped => {
                let _ = reply.send(Ok(self.status().await));
                return;
            }
            RuntimeState::Crashed | RuntimeState::Error => {
                self.state.state = RuntimeState::Stopped;
                let _ = reply.send(Ok(self.status().await));
                return;
            }
            RuntimeState::Starting => {
                let start = self.start.take();
                let exit = self
                    .reap_process(RuntimeExitReason::RequestedShutdown, true)
                    .await;
                self.state.stopped(exit);
                if let Some(start) = start {
                    let _ = start.reply.send(Err(ManagerError::unavailable(
                        "Runtime start was cancelled by stop",
                    )));
                }
                let _ = reply.send(Ok(self.status().await));
                return;
            }
            RuntimeState::Stopping => {
                let _ = reply.send(Err(ManagerError::invalid_state(
                    "Runtime shutdown is already in progress",
                )));
                return;
            }
            RuntimeState::Connected => {}
        }

        if let Err(error) = self.state.begin_stop() {
            let _ = reply.send(Err(error));
            return;
        }
        let request_id = match self.ids.allocate() {
            Ok(value) => value,
            Err(_) => {
                let error = ManagerError::new(
                    ErrorCode::InternalError,
                    ManagerErrorKind::RequestIdExhausted,
                    "request ID space exhausted",
                );
                self.fail_stream_deferred(
                    error,
                    RuntimeExitReason::ProtocolFailure,
                    Some(reply),
                    None,
                )
                .await;
                return;
            }
        };
        let message = RuntimeMessage {
            kind: MessageKind::Request,
            protocol_version: IPC_PROTOCOL_CURRENT_VERSION,
            request_id,
            command: Command::Shutdown,
            error_code: ErrorCode::Success,
            payload: Vec::new(),
        };
        let frame = match encode(&message) {
            Ok(value) => value,
            Err(error) => {
                let error =
                    ManagerError::protocol(format!("could not encode Shutdown request: {error:?}"));
                self.fail_stream_deferred(
                    error,
                    RuntimeExitReason::ProtocolFailure,
                    Some(reply),
                    None,
                )
                .await;
                return;
            }
        };
        self.stop = Some(StopContext {
            request_id,
            deadline: Instant::now() + self.config.shutdown_timeout,
            response_received: false,
            stdout_eof: false,
            reply,
        });
        if let Err(error) = self.queue_frame(request_id, frame) {
            self.fail_stream(error, RuntimeExitReason::ProcessFailure)
                .await;
        }
    }

    async fn send_request(&mut self, command: Command, payload: Vec<u8>, reply: PendingReply) {
        if self.state.state != RuntimeState::Connected {
            let error = if self.state.state == RuntimeState::Stopping {
                ManagerError::new(
                    ErrorCode::RuntimeShuttingDown,
                    ManagerErrorKind::InvalidState,
                    "Runtime is stopping",
                )
            } else {
                ManagerError::unavailable("Runtime is not connected")
            };
            PendingRequest {
                generation: self.state.generation,
                command,
                deadline: Instant::now(),
                reply,
            }
            .fail(error);
            return;
        }
        if self.pending.entries.len() >= self.pending.maximum {
            PendingRequest {
                generation: self.state.generation,
                command,
                deadline: Instant::now(),
                reply,
            }
            .fail(ManagerError::new(
                ErrorCode::RuntimeUnavailable,
                ManagerErrorKind::Capacity,
                "maximum in-flight request count reached",
            ));
            return;
        }
        let request_id = match self.ids.allocate() {
            Ok(value) => value,
            Err(_) => {
                PendingRequest {
                    generation: self.state.generation,
                    command,
                    deadline: Instant::now(),
                    reply,
                }
                .fail(ManagerError::new(
                    ErrorCode::InternalError,
                    ManagerErrorKind::RequestIdExhausted,
                    "request ID space exhausted",
                ));
                return;
            }
        };
        let message = RuntimeMessage {
            kind: MessageKind::Request,
            protocol_version: IPC_PROTOCOL_CURRENT_VERSION,
            request_id,
            command,
            error_code: ErrorCode::Success,
            payload,
        };
        let frame = match encode(&message) {
            Ok(value) => value,
            Err(error) => {
                PendingRequest {
                    generation: self.state.generation,
                    command,
                    deadline: Instant::now(),
                    reply,
                }
                .fail(ManagerError::new(
                    error.code,
                    ManagerErrorKind::Payload,
                    format!("request violates the command contract: {:?}", error.kind),
                ));
                return;
            }
        };
        self.pending
            .insert(
                request_id,
                self.state.generation,
                command,
                Instant::now() + self.config.request_timeout,
                reply,
            )
            .expect("pending capacity and fresh monotonic ID were checked");
        if let Err(error) = self.queue_frame(request_id, frame) {
            let pending = self.pending.take(request_id);
            self.fail_stream_deferred(error, RuntimeExitReason::ProcessFailure, None, pending)
                .await;
        }
    }

    fn queue_frame(&mut self, request_id: u64, frame: Vec<u8>) -> Result<(), ManagerError> {
        let Some(process) = self.process.as_mut() else {
            return Err(ManagerError::unavailable("Runtime process is unavailable"));
        };
        let Some(writer) = process.writer_tx.as_ref() else {
            return Err(ManagerError::process("Runtime stdin writer is closed"));
        };
        writer
            .try_send(WriteCommand {
                generation: self.state.generation,
                request_id,
                frame,
            })
            .map_err(|error| {
                ManagerError::process(format!("Runtime stdin writer queue failed: {error}"))
            })
    }

    async fn handle_event(&mut self, event: ProcessEvent) {
        let generation = match &event {
            ProcessEvent::Message { generation, .. }
            | ProcessEvent::Eof { generation, .. }
            | ProcessEvent::Failure { generation, .. }
            | ProcessEvent::WriteComplete { generation, .. }
            | ProcessEvent::WriteFailure { generation, .. } => *generation,
        };
        if generation != self.state.generation || self.process.is_none() {
            return;
        }
        match event {
            ProcessEvent::Message { message, .. } => self.handle_message(generation, message).await,
            ProcessEvent::Eof { clean, .. } => {
                if !clean {
                    self.fail_stream(
                        ManagerError::protocol("Runtime stdout ended with a truncated frame"),
                        RuntimeExitReason::ProtocolFailure,
                    )
                    .await;
                } else if self.state.state == RuntimeState::Stopping {
                    if let Some(stop) = self.stop.as_mut() {
                        stop.stdout_eof = true;
                    }
                    self.try_complete_shutdown().await;
                } else {
                    self.handle_unexpected_stdout_eof().await;
                }
            }
            ProcessEvent::Failure { error, reason, .. } => {
                self.fail_stream(error, reason).await;
            }
            ProcessEvent::WriteComplete { request_id, .. } => {
                let _ = request_id;
            }
            ProcessEvent::WriteFailure {
                request_id, error, ..
            } => {
                let primary = self.pending.take(request_id);
                self.fail_stream_deferred(error, RuntimeExitReason::ProcessFailure, None, primary)
                    .await;
            }
        }
    }

    async fn handle_message(&mut self, generation: u64, message: RuntimeMessage) {
        if self.state.state == RuntimeState::Starting {
            if message.kind != MessageKind::Hello {
                self.fail_stream(
                    ManagerError::protocol("first Runtime frame was not Hello"),
                    RuntimeExitReason::ProtocolFailure,
                )
                .await;
                return;
            }
            let hello = match parse_hello(&message.payload) {
                Ok(value) => value,
                Err(error) => {
                    self.fail_stream(
                        ManagerError::payload(error, Command::None),
                        RuntimeExitReason::ProtocolFailure,
                    )
                    .await;
                    return;
                }
            };
            let Some(pid) = self.state.pid else {
                self.fail_stream(
                    ManagerError::process("spawned Runtime has no process ID"),
                    RuntimeExitReason::ProcessFailure,
                )
                .await;
                return;
            };
            self.state.connected(pid, hello);
            if let Some(start) = self.start.take() {
                let _ = start.reply.send(Ok(self.status().await));
            }
            return;
        }
        if message.kind != MessageKind::Response {
            self.fail_stream(
                ManagerError::protocol("Runtime emitted an unexpected non-response frame"),
                RuntimeExitReason::ProtocolFailure,
            )
            .await;
            return;
        }
        if self
            .stop
            .as_ref()
            .is_some_and(|stop| stop.request_id == message.request_id)
        {
            self.handle_shutdown_response(message).await;
            return;
        }
        let result = if message.error_code == ErrorCode::Success {
            if let Err(error) = validate_success_payload(message.command, &message.payload) {
                self.fail_stream(error, RuntimeExitReason::ProtocolFailure)
                    .await;
                return;
            }
            Ok(RuntimeResponse {
                generation,
                payload: message.payload,
            })
        } else {
            Err(remote_error(message.error_code, &message.payload))
        };
        if let Err(error) = self
            .pending
            .complete(message.request_id, message.command, result)
        {
            self.fail_stream(
                ManagerError::protocol(format!("response correlation failed: {error:?}")),
                RuntimeExitReason::ProtocolFailure,
            )
            .await;
        }
    }

    async fn handle_shutdown_response(&mut self, message: RuntimeMessage) {
        let Some(stop) = self.stop.as_mut() else {
            return;
        };
        if stop.response_received || message.command != Command::Shutdown {
            self.fail_stream(
                ManagerError::protocol("Shutdown response was duplicate or mismatched"),
                RuntimeExitReason::ProtocolFailure,
            )
            .await;
            return;
        }
        if message.error_code != ErrorCode::Success {
            let error = remote_error(message.error_code, &message.payload);
            self.fail_stream(error, RuntimeExitReason::ProcessFailure)
                .await;
            return;
        }
        stop.response_received = true;
        self.try_complete_shutdown().await;
    }

    async fn handle_unexpected_stdout_eof(&mut self) {
        let poll = match self.process.as_mut() {
            Some(process) if process.exit_status.is_none() => process.child.try_wait(),
            Some(process) => Ok(process.exit_status),
            None => return,
        };
        match poll {
            Ok(Some(status)) => {
                if let Some(process) = self.process.as_mut() {
                    process.exit_status = Some(status);
                }
                self.crash_stream(ManagerError::process("Runtime exited unexpectedly"))
                    .await;
            }
            Ok(None) => {
                self.fail_stream(
                    ManagerError::protocol("Runtime stdout closed before a completed shutdown"),
                    RuntimeExitReason::ProtocolFailure,
                )
                .await;
            }
            Err(error) => {
                self.fail_stream(
                    ManagerError::process(format!("Runtime exit monitoring failed: {error}")),
                    RuntimeExitReason::ProcessFailure,
                )
                .await;
            }
        }
    }

    async fn handle_deadlines(&mut self) {
        let now = Instant::now();
        if self
            .start
            .as_ref()
            .is_some_and(|start| start.deadline <= now)
        {
            self.fail_stream(
                ManagerError::timeout("Runtime Hello timed out"),
                RuntimeExitReason::Timeout,
            )
            .await;
            return;
        }
        if self.stop.as_ref().is_some_and(|stop| stop.deadline <= now) {
            self.fail_stream(
                ManagerError::timeout("Runtime shutdown timed out"),
                RuntimeExitReason::Timeout,
            )
            .await;
            return;
        }
        if let Some(request_id) = self.pending.expired_ids(now).first().copied() {
            let timeout_error =
                ManagerError::timeout(format!("Runtime request {request_id} timed out"));
            let primary = self.pending.take(request_id);
            self.fail_stream_deferred(timeout_error, RuntimeExitReason::Timeout, None, primary)
                .await;
            return;
        }
        if self
            .process
            .as_ref()
            .and_then(|process| process.exit_drain_deadline)
            .is_some_and(|deadline| deadline <= now)
        {
            self.crash_stream(ManagerError::process(
                "Runtime exited without closing its protocol pipe",
            ))
            .await;
        }
    }

    async fn poll_process_exit(&mut self) {
        let result = match self.process.as_mut() {
            Some(process) if process.exit_status.is_none() => process.child.try_wait(),
            _ => return,
        };
        match result {
            Ok(None) => {}
            Ok(Some(status)) => {
                if let Some(process) = self.process.as_mut() {
                    process.exit_status = Some(status);
                    if self.state.state != RuntimeState::Stopping {
                        process.exit_drain_deadline = Some(Instant::now() + PIPE_DRAIN_TIMEOUT);
                    }
                }
                if self.state.state == RuntimeState::Stopping {
                    self.try_complete_shutdown().await;
                }
            }
            Err(error) => {
                self.fail_stream(
                    ManagerError::process(format!("Runtime exit monitoring failed: {error}")),
                    RuntimeExitReason::ProcessFailure,
                )
                .await;
            }
        }
    }

    async fn try_complete_shutdown(&mut self) {
        let Some(stop) = self.stop.as_ref() else {
            return;
        };
        if stop.stdout_eof && !stop.response_received {
            self.fail_stream(
                ManagerError::protocol("Runtime stdout closed before the Shutdown response"),
                RuntimeExitReason::ProtocolFailure,
            )
            .await;
            return;
        }
        if !stop.stdout_eof || !stop.response_received {
            return;
        }
        let status = self
            .process
            .as_ref()
            .and_then(|process| process.exit_status);
        let Some(status) = status else {
            return;
        };
        if !status.success() {
            self.fail_stream(
                ManagerError::process("Runtime exited unsuccessfully during Shutdown"),
                RuntimeExitReason::ProcessFailure,
            )
            .await;
            return;
        }
        let stop = self.stop.take().expect("validated Shutdown context");
        let exit = self
            .reap_process(RuntimeExitReason::RequestedShutdown, false)
            .await;
        self.state.stopped(exit);
        self.pending.fail_all(ManagerError::unavailable(
            "Runtime stopped before request completion",
        ));
        let _ = stop.reply.send(Ok(self.status().await));
    }

    async fn fail_stream(&mut self, error: ManagerError, reason: RuntimeExitReason) {
        self.fail_stream_deferred(error, reason, None, None).await;
    }

    async fn fail_stream_deferred(
        &mut self,
        error: ManagerError,
        reason: RuntimeExitReason,
        extra_stop: Option<StatusReply>,
        primary: Option<PendingRequest>,
    ) {
        let start = self.start.take();
        let stop = self.stop.take();
        let has_primary = primary.is_some();
        let exit = self.reap_process(reason, true).await;
        self.state.last_exit = Some(exit);
        self.state.failed(error.clone());
        let pending_error = if has_primary {
            ManagerError::unavailable(format!("Runtime stream invalidated: {}", error.message))
        } else {
            error.clone()
        };
        self.pending.fail_all(pending_error);
        if let Some(primary) = primary {
            primary.fail(error.clone());
        }
        if let Some(start) = start {
            let _ = start.reply.send(Err(error.clone()));
        }
        if let Some(stop) = stop {
            let _ = stop.reply.send(Err(error.clone()));
        }
        if let Some(stop) = extra_stop {
            let _ = stop.send(Err(error));
        }
    }

    async fn crash_stream(&mut self, error: ManagerError) {
        let start = self.start.take();
        let stop = self.stop.take();
        let exit = self
            .reap_process(RuntimeExitReason::UnexpectedExit, true)
            .await;
        self.state.crashed(exit);
        self.state.last_error = Some(error.clone());
        self.pending.fail_all(error.clone());
        if let Some(start) = start {
            let _ = start.reply.send(Err(error.clone()));
        }
        if let Some(stop) = stop {
            let _ = stop.reply.send(Err(error));
        }
    }

    async fn reap_process(&mut self, reason: RuntimeExitReason, force: bool) -> RuntimeExit {
        let Some(mut process) = self.process.take() else {
            return RuntimeExit::from_status(reason, None);
        };
        drop(process.writer_tx.take());
        if force {
            process.writer_task.abort();
        }
        let mut status = process.exit_status.take();
        if force && status.is_none() {
            let _ = process.child.start_kill();
        }
        if status.is_none() {
            status = timeout(REAP_TIMEOUT, process.child.wait())
                .await
                .ok()
                .and_then(Result::ok);
        }
        if status.is_none() {
            let _ = process.child.start_kill();
            status = timeout(REAP_TIMEOUT, process.child.wait())
                .await
                .ok()
                .and_then(Result::ok);
        }
        self.finish_process_tasks(process).await;
        RuntimeExit::from_status(reason, status)
    }

    async fn finish_process_tasks(&mut self, mut process: ProcessResources) {
        let deadline = Instant::now() + PIPE_DRAIN_TIMEOUT;
        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut writer_done = false;
        let mut discarded_stdout_events = 0_usize;
        let mut stdout_abort_requested = false;
        while !stdout_done || !stderr_done || !writer_done {
            if discarded_stdout_events >= MAX_PIPE_DRAIN_EVENTS
                && !stdout_done
                && !stdout_abort_requested
            {
                process.stdout_task.abort();
                stdout_abort_requested = true;
            }
            tokio::select! {
                biased;
                _ = tokio::time::sleep_until(deadline) => break,
                event = self.events.recv(), if !stdout_done && !stdout_abort_requested => {
                    match event {
                        Some(ProcessEvent::Message { .. } | ProcessEvent::Eof { .. } | ProcessEvent::Failure { .. }) => {
                            discarded_stdout_events += 1;
                        }
                        Some(ProcessEvent::WriteComplete { .. } | ProcessEvent::WriteFailure { .. }) => {}
                        None => stdout_done = true,
                    }
                }
                _ = &mut process.stdout_task, if !stdout_done => stdout_done = true,
                _ = &mut process.stderr_task, if !stderr_done => stderr_done = true,
                _ = &mut process.writer_task, if !writer_done => writer_done = true,
            }
        }
        if !stdout_done {
            process.stdout_task.abort();
            let _ = process.stdout_task.await;
        }
        if !stderr_done {
            process.stderr_task.abort();
            let _ = process.stderr_task.await;
        }
        if !writer_done {
            process.writer_task.abort();
            let _ = process.writer_task.await;
        }
        self.state.stderr_tail = process.stderr_tail.lock().await.snapshot();
        self.state.pid = None;
    }

    async fn status(&self) -> RuntimeStatus {
        let mut status = self.state.snapshot();
        if let Some(process) = &self.process {
            status.stderr_tail = process.stderr_tail.lock().await.snapshot();
        }
        status
    }

    async fn manager_dropped(&mut self) {
        let error = ManagerError::unavailable("RuntimeManager was dropped");
        let start = self.start.take();
        let stop = self.stop.take();
        let _ = self
            .reap_process(RuntimeExitReason::ManagerDropped, true)
            .await;
        self.pending.fail_all(error.clone());
        if let Some(start) = start {
            let _ = start.reply.send(Err(error.clone()));
        }
        if let Some(stop) = stop {
            let _ = stop.reply.send(Err(error));
        }
    }
}

fn spawn_process(
    config: &RuntimeManagerConfig,
    generation: u64,
    events: mpsc::Sender<ProcessEvent>,
) -> Result<ProcessResources, ManagerError> {
    let mut command = tokio::process::Command::new(&config.runtime_path);
    command
        .arg("--plugin")
        .arg(&config.plugin_path)
        .arg("--mock-work-iterations")
        .arg(config.mock_work_iterations.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        // The Runtime receives no parent environment. Explicit executable and
        // plugin paths are sufficient on supported platforms, and clearing the
        // environment prevents loader variables or future UI secrets crossing
        // the isolation boundary.
        .env_clear();
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);

    let mut child = command
        .spawn()
        .map_err(|error| ManagerError::process(format!("Runtime spawn failed: {error}")))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| ManagerError::process("Runtime stdin pipe was not created"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ManagerError::process("Runtime stdout pipe was not created"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ManagerError::process("Runtime stderr pipe was not created"))?;
    let stderr_tail = Arc::new(Mutex::new(StderrTail::new(config.stderr_tail_bytes)));
    let (writer_tx, writer_rx) = mpsc::channel(config.max_in_flight + 1);
    let writer_task = tokio::spawn(write_stdin(stdin, writer_rx, events.clone()));
    let stdout_task = tokio::spawn(read_stdout(stdout, generation, events));
    let stderr_task = tokio::spawn(read_stderr(stderr, Arc::clone(&stderr_tail)));
    Ok(ProcessResources {
        child,
        writer_tx: Some(writer_tx),
        writer_task,
        stdout_task,
        stderr_task,
        stderr_tail,
        exit_status: None,
        exit_drain_deadline: None,
    })
}

async fn write_stdin(
    mut stdin: ChildStdin,
    mut commands: mpsc::Receiver<WriteCommand>,
    events: mpsc::Sender<ProcessEvent>,
) {
    while let Some(command) = commands.recv().await {
        let result = async {
            stdin.write_all(&command.frame).await?;
            stdin.flush().await
        }
        .await;
        let event = match result {
            Ok(()) => ProcessEvent::WriteComplete {
                generation: command.generation,
                request_id: command.request_id,
            },
            Err(error) => ProcessEvent::WriteFailure {
                generation: command.generation,
                request_id: command.request_id,
                error: ManagerError::process(format!("Runtime stdin write failed: {error}")),
            },
        };
        let failed = matches!(event, ProcessEvent::WriteFailure { .. });
        if events.send(event).await.is_err() || failed {
            return;
        }
    }
}

async fn read_stdout(
    mut stdout: tokio::process::ChildStdout,
    generation: u64,
    events: mpsc::Sender<ProcessEvent>,
) {
    let mut decoder = Decoder::new();
    let mut buffer = [0_u8; STDOUT_READ_BYTES];
    loop {
        match stdout.read(&mut buffer).await {
            Ok(0) => {
                let clean = decoder.finish().is_ok();
                let _ = events.send(ProcessEvent::Eof { generation, clean }).await;
                return;
            }
            Ok(count) => match decoder.feed(&buffer[..count]) {
                Ok(messages) => {
                    for message in messages {
                        if events
                            .send(ProcessEvent::Message {
                                generation,
                                message,
                            })
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                }
                Err(error) => {
                    let _ = events
                        .send(ProcessEvent::Failure {
                            generation,
                            error: ManagerError::new(
                                error.code,
                                ManagerErrorKind::Protocol,
                                format!("Runtime stdout framing failed: {:?}", error.kind),
                            ),
                            reason: RuntimeExitReason::ProtocolFailure,
                        })
                        .await;
                    return;
                }
            },
            Err(error) => {
                let _ = events
                    .send(ProcessEvent::Failure {
                        generation,
                        error: ManagerError::process(format!(
                            "Runtime stdout read failed: {error}"
                        )),
                        reason: RuntimeExitReason::ProcessFailure,
                    })
                    .await;
                return;
            }
        }
    }
}

async fn read_stderr(mut stderr: tokio::process::ChildStderr, tail: Arc<Mutex<StderrTail>>) {
    let mut buffer = [0_u8; 1_024];
    loop {
        match stderr.read(&mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(count) => tail.lock().await.extend(&buffer[..count]),
        }
    }
}

fn remote_error(code: ErrorCode, payload: &[u8]) -> ManagerError {
    let diagnostic = String::from_utf8_lossy(payload);
    ManagerError::new(
        code,
        ManagerErrorKind::Remote,
        format!("Runtime returned {code:?}: {diagnostic}"),
    )
}

fn validate_success_payload(command: Command, payload: &[u8]) -> Result<(), ManagerError> {
    match command {
        Command::Ping if payload.len() <= MAX_PING_PAYLOAD_BYTES => Ok(()),
        Command::GetCapabilities => parse_capabilities(payload)
            .map(|_| ())
            .map_err(|error| ManagerError::payload(error, command)),
        Command::RunMockPipeline => parse_mock_pipeline_summary(payload)
            .map(|_| ())
            .map_err(|error| ManagerError::payload(error, command)),
        Command::Shutdown if payload.is_empty() => Ok(()),
        Command::Ping | Command::Shutdown | Command::None => Err(ManagerError::protocol(format!(
            "invalid successful {command:?} payload"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::future::pending;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    use ai_voice_contracts::runtime_message::Command;
    use tokio::io::AsyncWrite;
    use tokio::sync::oneshot;
    use tokio::time::Instant;

    use super::*;

    const MOCK_NATIVE: &[u8] = br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#;
    const UNAVAILABLE_NATIVE: &[u8] = br#"{"platform":"windows","architecture":"x86_64","runtime_version":"0.0.0","protocol_version":1,"backend":"unavailable","engine":"unavailable"}"#;

    #[derive(Clone, Copy)]
    enum TestObservation {
        NotEvaluated,
        Inconclusive,
        Mock,
        Unavailable,
    }

    fn test_observation(kind: TestObservation, generation: u64) -> CapabilityObservation {
        match kind {
            TestObservation::NotEvaluated => CapabilityObservation::not_evaluated(generation),
            TestObservation::Inconclusive => CapabilityObservation::inconclusive(
                generation,
                ManagerError::unavailable("controlled inconclusive query"),
            ),
            TestObservation::Mock => CapabilityObservation::observed(
                generation,
                NativeRuntimeCapabilities::from_canonical_json(MOCK_NATIVE).unwrap(),
            ),
            TestObservation::Unavailable => CapabilityObservation::observed(
                generation,
                NativeRuntimeCapabilities::from_canonical_json(UNAVAILABLE_NATIVE).unwrap(),
            ),
        }
    }

    fn canonical_test_hello() -> Hello {
        Hello {
            runtime_version: "0.0.0".to_owned(),
            protocol_version: 1,
            generation: 1,
            health: "starting".to_owned(),
        }
    }

    fn fill_stdin_pipe(stdin: &mut ChildStdin) {
        let mut context = Context::from_waker(Waker::noop());
        let bytes = [0_u8; 4_096];
        for _ in 0..1_024 {
            match Pin::new(&mut *stdin).poll_write(&mut context, &bytes) {
                Poll::Ready(Ok(count)) if count != 0 => {}
                Poll::Pending => return,
                other => panic!("could not fill controlled stdin pipe: {other:?}"),
            }
        }
        panic!("controlled stdin pipe never became full");
    }

    #[test]
    fn literal_profile_truth_table_covers_every_manager_state_and_query_outcome() {
        let cases = [
            (
                "stopped",
                ManagerHealth::Stopped,
                0,
                TestObservation::NotEvaluated,
                r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"unavailable"},"engine":{"identity":"aivs-mock-v1","availability":"unavailable"},"manager":{"health":"stopped","generation":0}}"#,
            ),
            (
                "starting",
                ManagerHealth::Starting,
                1,
                TestObservation::NotEvaluated,
                r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"not_evaluated"},"engine":{"identity":"aivs-mock-v1","availability":"not_evaluated"},"manager":{"health":"starting","generation":1}}"#,
            ),
            (
                "connected-not-evaluated",
                ManagerHealth::Connected,
                1,
                TestObservation::NotEvaluated,
                r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"available"},"engine":{"identity":"aivs-mock-v1","availability":"not_evaluated"},"manager":{"health":"connected","generation":1}}"#,
            ),
            (
                "connected-inconclusive",
                ManagerHealth::Connected,
                1,
                TestObservation::Inconclusive,
                r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"available"},"engine":{"identity":"aivs-mock-v1","availability":"unknown"},"manager":{"health":"connected","generation":1}}"#,
            ),
            (
                "connected-mock",
                ManagerHealth::Connected,
                1,
                TestObservation::Mock,
                r#"{"schema_version":1,"platform":"macos","architecture":"arm64","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"available"},"engine":{"identity":"aivs-mock-v1","availability":"available"},"manager":{"health":"connected","generation":1}}"#,
            ),
            (
                "connected-unavailable-engine",
                ManagerHealth::Connected,
                1,
                TestObservation::Unavailable,
                r#"{"schema_version":1,"platform":"windows","architecture":"x86_64","runtime":{"version":"0.0.0","protocol_version":1,"backend":"unavailable","availability":"available"},"engine":{"identity":null,"availability":"unavailable"},"manager":{"health":"connected","generation":1}}"#,
            ),
            (
                "stopping",
                ManagerHealth::Stopping,
                1,
                TestObservation::NotEvaluated,
                r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"unavailable"},"engine":{"identity":"aivs-mock-v1","availability":"unavailable"},"manager":{"health":"stopping","generation":1}}"#,
            ),
            (
                "crashed",
                ManagerHealth::Crashed,
                1,
                TestObservation::NotEvaluated,
                r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"unavailable"},"engine":{"identity":"aivs-mock-v1","availability":"unavailable"},"manager":{"health":"crashed","generation":1}}"#,
            ),
            (
                "error",
                ManagerHealth::Error,
                1,
                TestObservation::NotEvaluated,
                r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"unavailable"},"engine":{"identity":"aivs-mock-v1","availability":"unavailable"},"manager":{"health":"error","generation":1}}"#,
            ),
        ];

        for (name, health, generation, observation, expected) in cases {
            let observation = test_observation(observation, generation);
            let profile = CapabilityProfile::from_actor_observation(
                RuntimeBackend::Mock,
                health,
                generation,
                &observation,
            )
            .unwrap();
            assert_eq!(serde_json::to_string(&profile).unwrap(), expected, "{name}");
        }
    }

    #[test]
    fn profile_builder_rejects_invalid_internal_state_and_omits_hardware_claims() {
        let observation = test_observation(TestObservation::NotEvaluated, 0);
        assert_eq!(
            CapabilityProfile::from_actor_observation(
                RuntimeBackend::Mock,
                ManagerHealth::Connected,
                0,
                &observation,
            )
            .unwrap_err(),
            CapabilityProfileError::InvalidManagerGeneration
        );

        let inconclusive = test_observation(TestObservation::Inconclusive, 1);
        assert_eq!(
            CapabilityProfile::from_actor_observation(
                RuntimeBackend::Mock,
                ManagerHealth::Crashed,
                1,
                &inconclusive,
            )
            .unwrap_err(),
            CapabilityProfileError::ObservationNotAllowed {
                health: ManagerHealth::Crashed,
            }
        );

        let native = test_observation(TestObservation::Mock, 7);
        let profile = CapabilityProfile::from_actor_observation(
            RuntimeBackend::Mock,
            ManagerHealth::Connected,
            7,
            &native,
        )
        .unwrap();
        let serialized = serde_json::to_string(&profile).unwrap();
        for forbidden in [
            "cpu",
            "gpu",
            "ram",
            "npu",
            "audio",
            "device",
            "driver",
            "benchmark",
            "machine",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[cfg(unix)]
    fn blocked_stdin_child() -> Child {
        tokio::process::Command::new("/bin/sh")
            .args(["-c", "sleep 30"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap()
    }

    #[cfg(windows)]
    fn blocked_stdin_child() -> Child {
        tokio::process::Command::new("cmd.exe")
            .args(["/D", "/C", "ping -n 31 127.0.0.1 >NUL"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap()
    }

    #[cfg(unix)]
    async fn test_process_exists(pid: u32) -> bool {
        tokio::process::Command::new("/bin/kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .unwrap()
            .success()
    }

    #[cfg(windows)]
    async fn test_process_exists(pid: u32) -> bool {
        let output = tokio::process::Command::new("tasklist.exe")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .await
            .unwrap();
        String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn full_stdin_pipe_cannot_mask_an_existing_earlier_deadline() {
        let mut child = blocked_stdin_child();
        let pid = child.id().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        fill_stdin_pipe(&mut stdin);
        let runtime_path = std::env::current_exe().unwrap();
        let mut config = RuntimeManagerConfig::new(runtime_path.clone(), runtime_path);
        config.request_timeout = Duration::from_secs(2);
        let (command_tx, command_rx) = mpsc::channel(config.command_queue_capacity);
        let (event_tx, event_rx) = mpsc::channel(config.event_queue_capacity);
        let mut actor = Actor::new(config, command_rx, event_tx.clone(), event_rx);
        actor.state.begin_start(1).unwrap();
        actor.state.connected(pid, canonical_test_hello());
        let (writer_tx, writer_rx) = mpsc::channel(actor.config.max_in_flight + 1);
        actor.process = Some(ProcessResources {
            child,
            writer_tx: Some(writer_tx),
            writer_task: tokio::spawn(write_stdin(stdin, writer_rx, event_tx)),
            stdout_task: tokio::spawn(pending()),
            stderr_task: tokio::spawn(pending()),
            stderr_tail: Arc::new(Mutex::new(StderrTail::new(64))),
            exit_status: None,
            exit_drain_deadline: None,
        });
        let (expired_tx, expired_rx) = oneshot::channel();
        actor
            .pending
            .insert(
                41,
                1,
                Command::Ping,
                Instant::now() + Duration::from_millis(50),
                PendingReply::Runtime(expired_tx),
            )
            .unwrap();
        let actor_task = tokio::spawn(actor.run());
        let (blocked_tx, _blocked_rx) = oneshot::channel();
        command_tx
            .send(ActorCommand::Request {
                command: Command::Ping,
                payload: vec![0; MAX_PING_PAYLOAD_BYTES],
                reply: blocked_tx,
            })
            .await
            .unwrap();

        let error = timeout(Duration::from_millis(500), expired_rx)
            .await
            .expect("blocked stdin write masked the existing earlier deadline")
            .unwrap()
            .unwrap_err();

        assert_eq!(error.generation, 1);
        assert_eq!(error.error.kind, ManagerErrorKind::Timeout);
        assert!(!test_process_exists(pid).await);
        let (status_tx, status_rx) = oneshot::channel();
        command_tx
            .send(ActorCommand::Status { reply: status_tx })
            .await
            .unwrap();
        assert_eq!(status_rx.await.unwrap().pid, None);
        drop(command_tx);
        actor_task.await.unwrap();
    }

    #[test]
    fn request_ids_are_nonzero_monotonic_and_exhaust_explicitly() {
        let mut ids = RequestIds::new();
        assert_eq!(ids.allocate().unwrap(), 1);
        assert_eq!(ids.allocate().unwrap(), 2);

        ids.next = u64::MAX;
        assert_eq!(ids.allocate().unwrap(), u64::MAX);
        assert_eq!(ids.allocate(), Err(CorrelationError::RequestIdExhausted));
    }

    #[tokio::test]
    async fn pending_requests_bound_and_correlate_exact_command_once() {
        let mut pending = PendingRequests::new(2);
        let deadline = Instant::now() + Duration::from_secs(1);
        let (first_tx, first_rx) = oneshot::channel();
        let (second_tx, _second_rx) = oneshot::channel();
        let (overflow_tx, _overflow_rx) = oneshot::channel();

        pending
            .insert(
                7,
                3,
                Command::Ping,
                deadline,
                PendingReply::Runtime(first_tx),
            )
            .unwrap();
        pending
            .insert(
                8,
                3,
                Command::GetCapabilities,
                deadline,
                PendingReply::Runtime(second_tx),
            )
            .unwrap();
        assert_eq!(
            pending.insert(
                9,
                3,
                Command::Ping,
                deadline,
                PendingReply::Runtime(overflow_tx),
            ),
            Err(CorrelationError::PendingLimit)
        );

        pending
            .complete(
                7,
                Command::Ping,
                Ok(RuntimeResponse {
                    generation: 3,
                    payload: vec![1, 2, 3],
                }),
            )
            .unwrap();
        assert_eq!(
            first_rx.await.unwrap().unwrap(),
            RuntimeResponse {
                generation: 3,
                payload: vec![1, 2, 3],
            }
        );
        assert_eq!(
            pending.complete(
                7,
                Command::Ping,
                Ok(RuntimeResponse {
                    generation: 3,
                    payload: Vec::new(),
                }),
            ),
            Err(CorrelationError::UnknownRequest)
        );
        assert_eq!(
            pending.complete(
                8,
                Command::Ping,
                Ok(RuntimeResponse {
                    generation: 3,
                    payload: Vec::new(),
                }),
            ),
            Err(CorrelationError::CommandMismatch)
        );
    }

    #[tokio::test]
    async fn pending_timeout_selection_and_failure_are_deterministic() {
        let mut pending = PendingRequests::new(3);
        let now = Instant::now();
        let (later_tx, later_rx) = oneshot::channel();
        let (first_tx, first_rx) = oneshot::channel();
        pending
            .insert(
                20,
                9,
                Command::Ping,
                now + Duration::from_secs(2),
                PendingReply::Runtime(later_tx),
            )
            .unwrap();
        pending
            .insert(
                10,
                9,
                Command::GetCapabilities,
                now + Duration::from_secs(1),
                PendingReply::Runtime(first_tx),
            )
            .unwrap();

        assert_eq!(pending.next_deadline(), Some(now + Duration::from_secs(1)));
        assert_eq!(pending.expired_ids(now + Duration::from_secs(1)), vec![10]);

        let timeout = ManagerError::timeout("request 10 timed out");
        pending.fail_one(10, timeout.clone());
        pending.fail_all(ManagerError::unavailable("stream invalidated"));
        let first_failure = first_rx.await.unwrap().unwrap_err();
        assert_eq!(first_failure.generation, 9);
        assert_eq!(first_failure.error, timeout);
        assert_eq!(
            later_rx.await.unwrap().unwrap_err().error.code,
            ErrorCode::RuntimeUnavailable
        );
        assert!(pending.entries.is_empty());
    }

    #[test]
    fn state_model_covers_start_connect_stop_crash_and_error() {
        let mut state = StateModel::default();
        assert_eq!(state.state, RuntimeState::Stopped);
        assert!(state.begin_start(41).is_ok());
        assert_eq!(state.state, RuntimeState::Starting);
        assert!(state.begin_start(42).is_err());
        state.connected(9001, canonical_test_hello());
        assert_eq!(state.state, RuntimeState::Connected);
        assert_eq!(state.pid, Some(9001));
        state.begin_stop().unwrap();
        assert_eq!(state.state, RuntimeState::Stopping);
        state.stopped(RuntimeExit::requested(0));
        assert_eq!(state.state, RuntimeState::Stopped);
        assert_eq!(state.pid, None);

        state.begin_start(42).unwrap();
        state.crashed(RuntimeExit::unexpected(Some(4)));
        assert_eq!(state.state, RuntimeState::Crashed);
        state.begin_start(43).unwrap();
        state.failed(ManagerError::protocol("bad hello"));
        assert_eq!(state.state, RuntimeState::Error);
    }

    #[test]
    fn stderr_tail_retains_only_the_newest_bytes() {
        let mut tail = StderrTail::new(5);
        tail.extend(b"abc");
        tail.extend(b"defg");
        assert_eq!(tail.snapshot(), b"cdefg");
        tail.extend(b"0123456789");
        assert_eq!(tail.snapshot(), b"56789");
    }

    #[test]
    fn configuration_rejects_ambiguous_or_unbounded_process_inputs() {
        let relative = RuntimeManagerConfig::new(
            PathBuf::from("voice-runtime"),
            PathBuf::from("/tmp/mock-engine"),
        );
        assert_eq!(
            relative.validate().unwrap_err().kind,
            ManagerErrorKind::InvalidConfiguration
        );

        let mut zero_timeout = RuntimeManagerConfig::new(
            PathBuf::from("/tmp/voice-runtime"),
            PathBuf::from("/tmp/mock-engine"),
        );
        zero_timeout.request_timeout = Duration::ZERO;
        assert!(zero_timeout.validate().is_err());

        let mut no_pending = RuntimeManagerConfig::new(
            PathBuf::from("/tmp/voice-runtime"),
            PathBuf::from("/tmp/mock-engine"),
        );
        no_pending.max_in_flight = 0;
        assert!(no_pending.validate().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn configuration_rejects_embedded_nul_before_spawn() {
        use std::os::unix::ffi::OsStringExt;

        let invalid = PathBuf::from(std::ffi::OsString::from_vec(
            b"/tmp/voice\0-runtime".to_vec(),
        ));
        let config = RuntimeManagerConfig::new(invalid, PathBuf::from("/tmp/mock-engine"));
        assert_eq!(
            config.validate().unwrap_err().kind,
            ManagerErrorKind::InvalidConfiguration
        );
    }

    #[test]
    fn successful_payload_validation_is_command_specific() {
        assert!(validate_success_payload(Command::Ping, b"\x00\xff").is_ok());
        assert!(validate_success_payload(Command::GetCapabilities, b"{}").is_err());
        assert!(validate_success_payload(Command::RunMockPipeline, &[0; 80]).is_err());
        assert!(validate_success_payload(Command::Shutdown, b"").is_ok());
        assert!(validate_success_payload(Command::None, b"").is_err());
    }
}
