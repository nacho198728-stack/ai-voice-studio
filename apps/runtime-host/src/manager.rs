use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;

use ai_voice_contracts::runtime_message::{
    Command, Decoder, MAX_PING_PAYLOAD_BYTES, MessageKind, RuntimeMessage, encode,
};
use ai_voice_contracts::{ErrorCode, IPC_PROTOCOL_CURRENT_VERSION};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Instant, MissedTickBehavior, timeout};

use crate::payload::{
    Hello, MockPipelineSummary, PayloadError, RuntimeCapabilities, parse_capabilities, parse_hello,
    parse_mock_pipeline_summary,
};

const MAX_PATH_UNITS: usize = 4_096;
const MAX_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_IN_FLIGHT: usize = 64;
const MAX_QUEUE_CAPACITY: usize = 256;
const MAX_STDERR_TAIL_BYTES: usize = 65_536;
const MAX_MOCK_WORK_ITERATIONS: u32 = 1_000_000;
const STDOUT_READ_BYTES: usize = 2_048;
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(25);

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
            format!("invalid {command:?} payload: {error:?}"),
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
        validate_nonzero_limit("max in-flight requests", self.max_in_flight, MAX_IN_FLIGHT)?;
        validate_nonzero_limit(
            "command queue capacity",
            self.command_queue_capacity,
            MAX_QUEUE_CAPACITY,
        )?;
        validate_nonzero_limit(
            "event queue capacity",
            self.event_queue_capacity,
            MAX_QUEUE_CAPACITY,
        )?;
        if self.stderr_tail_bytes > MAX_STDERR_TAIL_BYTES {
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
        self.request(Command::Ping, payload.to_vec()).await
    }

    pub async fn get_capabilities(&self) -> Result<RuntimeCapabilities, ManagerError> {
        let payload = self.request(Command::GetCapabilities, Vec::new()).await?;
        parse_capabilities(&payload)
            .map_err(|error| ManagerError::payload(error, Command::GetCapabilities))
    }

    pub async fn run_mock_pipeline(&self) -> Result<MockPipelineSummary, ManagerError> {
        let payload = self.request(Command::RunMockPipeline, Vec::new()).await?;
        parse_mock_pipeline_summary(&payload)
            .map_err(|error| ManagerError::payload(error, Command::RunMockPipeline))
    }

    async fn request(&self, command: Command, payload: Vec<u8>) -> Result<Vec<u8>, ManagerError> {
        let (reply, receiver) = oneshot::channel();
        self.send(ActorCommand::Request {
            command,
            payload,
            reply,
        })
        .await?;
        receive(receiver).await?
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
    Request {
        command: Command,
        payload: Vec<u8>,
        reply: oneshot::Sender<Result<Vec<u8>, ManagerError>>,
    },
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
    command: Command,
    deadline: Instant,
    reply: oneshot::Sender<Result<Vec<u8>, ManagerError>>,
}

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
        command: Command,
        deadline: Instant,
        reply: oneshot::Sender<Result<Vec<u8>, ManagerError>>,
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
        result: Result<Vec<u8>, ManagerError>,
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
        let _ = pending.reply.send(result);
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

    fn fail_one(&mut self, request_id: u64, error: ManagerError) {
        if let Some(pending) = self.entries.remove(&request_id) {
            let _ = pending.reply.send(Err(error));
        }
    }

    fn fail_all(&mut self, error: ManagerError) {
        for (_, pending) in std::mem::take(&mut self.entries) {
            let _ = pending.reply.send(Err(error.clone()));
        }
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

enum StdoutEvent {
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
    },
}

struct ProcessResources {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout_task: JoinHandle<()>,
    stderr_task: JoinHandle<()>,
    stderr_tail: Arc<Mutex<StderrTail>>,
}

struct StartContext {
    deadline: Instant,
    reply: oneshot::Sender<Result<RuntimeStatus, ManagerError>>,
}

struct StopContext {
    request_id: u64,
    deadline: Instant,
    response_received: bool,
    reply: oneshot::Sender<Result<RuntimeStatus, ManagerError>>,
}

struct Actor {
    config: RuntimeManagerConfig,
    commands: mpsc::Receiver<ActorCommand>,
    event_tx: mpsc::Sender<StdoutEvent>,
    events: mpsc::Receiver<StdoutEvent>,
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
        event_tx: mpsc::Sender<StdoutEvent>,
        events: mpsc::Receiver<StdoutEvent>,
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
                _ = exit_poll.tick() => self.poll_process_exit().await,
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        [
            self.start.as_ref().map(|start| start.deadline),
            self.stop.as_ref().map(|stop| stop.deadline),
            self.pending.next_deadline(),
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
            ActorCommand::Request {
                command,
                payload,
                reply,
            } => self.send_request(command, payload, reply).await,
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
                if let Some(start) = self.start.take() {
                    let _ = start.reply.send(Err(ManagerError::unavailable(
                        "Runtime start was cancelled by stop",
                    )));
                }
                let exit = self
                    .reap_process(RuntimeExitReason::RequestedShutdown, true)
                    .await;
                self.state.stopped(exit);
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

        if self.pending.entries.len() >= self.pending.maximum {
            let _ = reply.send(Err(ManagerError::new(
                ErrorCode::RuntimeUnavailable,
                ManagerErrorKind::Capacity,
                "maximum in-flight request count leaves no room for Shutdown",
            )));
            return;
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
                let _ = reply.send(Err(error.clone()));
                self.fail_stream(error, RuntimeExitReason::ProtocolFailure)
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
                let _ = reply.send(Err(error.clone()));
                self.fail_stream(error, RuntimeExitReason::ProtocolFailure)
                    .await;
                return;
            }
        };
        self.stop = Some(StopContext {
            request_id,
            deadline: Instant::now() + self.config.shutdown_timeout,
            response_received: false,
            reply,
        });
        if let Err(error) = self.write_frame(&frame, self.config.shutdown_timeout).await {
            self.fail_stream(error, RuntimeExitReason::ProcessFailure)
                .await;
        }
    }

    async fn send_request(
        &mut self,
        command: Command,
        payload: Vec<u8>,
        reply: oneshot::Sender<Result<Vec<u8>, ManagerError>>,
    ) {
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
            let _ = reply.send(Err(error));
            return;
        }
        if self.pending.entries.len() >= self.pending.maximum {
            let _ = reply.send(Err(ManagerError::new(
                ErrorCode::RuntimeUnavailable,
                ManagerErrorKind::Capacity,
                "maximum in-flight request count reached",
            )));
            return;
        }
        let request_id = match self.ids.allocate() {
            Ok(value) => value,
            Err(_) => {
                let _ = reply.send(Err(ManagerError::new(
                    ErrorCode::InternalError,
                    ManagerErrorKind::RequestIdExhausted,
                    "request ID space exhausted",
                )));
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
                let _ = reply.send(Err(ManagerError::new(
                    error.code,
                    ManagerErrorKind::Payload,
                    format!("request violates the command contract: {:?}", error.kind),
                )));
                return;
            }
        };
        self.pending
            .insert(
                request_id,
                command,
                Instant::now() + self.config.request_timeout,
                reply,
            )
            .expect("pending capacity and fresh monotonic ID were checked");
        if let Err(error) = self.write_frame(&frame, self.config.request_timeout).await {
            self.fail_stream(error, RuntimeExitReason::ProcessFailure)
                .await;
        }
    }

    async fn write_frame(&mut self, frame: &[u8], limit: Duration) -> Result<(), ManagerError> {
        let Some(process) = self.process.as_mut() else {
            return Err(ManagerError::unavailable("Runtime process is unavailable"));
        };
        let Some(stdin) = process.stdin.as_mut() else {
            return Err(ManagerError::unavailable("Runtime stdin is closed"));
        };
        let write = async {
            stdin.write_all(frame).await?;
            stdin.flush().await
        };
        match timeout(limit, write).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(ManagerError::process(format!(
                "Runtime stdin write failed: {error}"
            ))),
            Err(_) => Err(ManagerError::timeout("Runtime stdin write timed out")),
        }
    }

    async fn handle_event(&mut self, event: StdoutEvent) {
        let generation = match &event {
            StdoutEvent::Message { generation, .. }
            | StdoutEvent::Eof { generation, .. }
            | StdoutEvent::Failure { generation, .. } => *generation,
        };
        if generation != self.state.generation || self.process.is_none() {
            return;
        }
        match event {
            StdoutEvent::Message { message, .. } => self.handle_message(message).await,
            StdoutEvent::Eof { clean, .. } => {
                if clean {
                    self.handle_exit(None).await;
                } else {
                    self.fail_stream(
                        ManagerError::protocol("Runtime stdout ended with a truncated frame"),
                        RuntimeExitReason::ProtocolFailure,
                    )
                    .await;
                }
            }
            StdoutEvent::Failure { error, .. } => {
                self.fail_stream(error, RuntimeExitReason::ProtocolFailure)
                    .await;
            }
        }
    }

    async fn handle_message(&mut self, message: RuntimeMessage) {
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
            Ok(message.payload)
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
            self.pending.fail_one(request_id, timeout_error.clone());
            self.fail_stream(timeout_error, RuntimeExitReason::Timeout)
                .await;
        }
    }

    async fn poll_process_exit(&mut self) {
        let exited = self
            .process
            .as_mut()
            .and_then(|process| process.child.try_wait().ok().flatten());
        if let Some(status) = exited {
            self.handle_exit(Some(status)).await;
        }
    }

    async fn handle_exit(&mut self, known_status: Option<ExitStatus>) {
        let state_before = self.state.state;
        let status = match known_status {
            Some(value) => {
                self.finish_known_exit().await;
                Some(value)
            }
            None => self.wait_for_natural_exit().await,
        };
        let clean_shutdown = state_before == RuntimeState::Stopping
            && self
                .stop
                .as_ref()
                .is_some_and(|stop| stop.response_received)
            && status.is_some_and(|value| value.success());
        if clean_shutdown {
            let exit = RuntimeExit::from_status(RuntimeExitReason::RequestedShutdown, status);
            self.state.stopped(exit);
            if let Some(stop) = self.stop.take() {
                let _ = stop.reply.send(Ok(self.status().await));
            }
            return;
        }

        let exit = RuntimeExit::from_status(RuntimeExitReason::UnexpectedExit, status);
        self.state.crashed(exit);
        let error = ManagerError::process("Runtime exited unexpectedly");
        self.state.last_error = Some(error.clone());
        self.pending.fail_all(error.clone());
        if let Some(start) = self.start.take() {
            let _ = start.reply.send(Err(error.clone()));
        }
        if let Some(stop) = self.stop.take() {
            let _ = stop.reply.send(Err(error));
        }
    }

    async fn wait_for_natural_exit(&mut self) -> Option<ExitStatus> {
        let mut process = self.process.take()?;
        drop(process.stdin.take());
        let status = match timeout(self.config.shutdown_timeout, process.child.wait()).await {
            Ok(Ok(value)) => Some(value),
            _ => {
                let _ = process.child.start_kill();
                timeout(self.config.shutdown_timeout, process.child.wait())
                    .await
                    .ok()
                    .and_then(Result::ok)
            }
        };
        self.finish_process_tasks(process).await;
        status
    }

    async fn finish_known_exit(&mut self) {
        if let Some(mut process) = self.process.take() {
            drop(process.stdin.take());
            self.finish_process_tasks(process).await;
        }
    }

    async fn fail_stream(&mut self, error: ManagerError, reason: RuntimeExitReason) {
        if let Some(start) = self.start.take() {
            let _ = start.reply.send(Err(error.clone()));
        }
        if let Some(stop) = self.stop.take() {
            let _ = stop.reply.send(Err(error.clone()));
        }
        self.pending.fail_all(ManagerError::unavailable(format!(
            "Runtime stream invalidated: {}",
            error.message
        )));
        let exit = self.reap_process(reason, true).await;
        self.state.last_exit = Some(exit);
        self.state.failed(error);
    }

    async fn reap_process(&mut self, reason: RuntimeExitReason, force: bool) -> RuntimeExit {
        let Some(mut process) = self.process.take() else {
            return RuntimeExit::from_status(reason, None);
        };
        drop(process.stdin.take());
        if force {
            let _ = process.child.start_kill();
        }
        let mut status = timeout(self.config.shutdown_timeout, process.child.wait())
            .await
            .ok()
            .and_then(Result::ok);
        if status.is_none() {
            let _ = process.child.start_kill();
            status = timeout(self.config.shutdown_timeout, process.child.wait())
                .await
                .ok()
                .and_then(Result::ok);
        }
        self.finish_process_tasks(process).await;
        RuntimeExit::from_status(reason, status)
    }

    async fn finish_process_tasks(&mut self, process: ProcessResources) {
        process.stdout_task.abort();
        process.stderr_task.abort();
        let _ = process.stdout_task.await;
        let _ = process.stderr_task.await;
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
        if let Some(start) = self.start.take() {
            let _ = start.reply.send(Err(error.clone()));
        }
        if let Some(stop) = self.stop.take() {
            let _ = stop.reply.send(Err(error.clone()));
        }
        self.pending.fail_all(error);
        let _ = self
            .reap_process(RuntimeExitReason::ManagerDropped, true)
            .await;
    }
}

fn spawn_process(
    config: &RuntimeManagerConfig,
    generation: u64,
    events: mpsc::Sender<StdoutEvent>,
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
        .kill_on_drop(true);
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
    let stdout_task = tokio::spawn(read_stdout(stdout, generation, events));
    let stderr_task = tokio::spawn(read_stderr(stderr, Arc::clone(&stderr_tail)));
    Ok(ProcessResources {
        child,
        stdin: Some(stdin),
        stdout_task,
        stderr_task,
        stderr_tail,
    })
}

async fn read_stdout(
    mut stdout: tokio::process::ChildStdout,
    generation: u64,
    events: mpsc::Sender<StdoutEvent>,
) {
    let mut decoder = Decoder::new();
    let mut buffer = [0_u8; STDOUT_READ_BYTES];
    loop {
        match stdout.read(&mut buffer).await {
            Ok(0) => {
                let clean = decoder.finish().is_ok();
                let _ = events.send(StdoutEvent::Eof { generation, clean }).await;
                return;
            }
            Ok(count) => match decoder.feed(&buffer[..count]) {
                Ok(messages) => {
                    for message in messages {
                        if events
                            .send(StdoutEvent::Message {
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
                        .send(StdoutEvent::Failure {
                            generation,
                            error: ManagerError::new(
                                error.code,
                                ManagerErrorKind::Protocol,
                                format!("Runtime stdout framing failed: {:?}", error.kind),
                            ),
                        })
                        .await;
                    return;
                }
            },
            Err(error) => {
                let _ = events
                    .send(StdoutEvent::Failure {
                        generation,
                        error: ManagerError::process(format!(
                            "Runtime stdout read failed: {error}"
                        )),
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
    use std::time::Duration;

    use ai_voice_contracts::runtime_message::Command;
    use tokio::sync::oneshot;
    use tokio::time::Instant;

    use super::*;

    fn canonical_test_hello() -> Hello {
        Hello {
            runtime_version: "0.0.0".to_owned(),
            protocol_version: 1,
            generation: 1,
            health: "starting".to_owned(),
        }
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
            .insert(7, Command::Ping, deadline, first_tx)
            .unwrap();
        pending
            .insert(8, Command::GetCapabilities, deadline, second_tx)
            .unwrap();
        assert_eq!(
            pending.insert(9, Command::Ping, deadline, overflow_tx),
            Err(CorrelationError::PendingLimit)
        );

        pending
            .complete(7, Command::Ping, Ok(vec![1, 2, 3]))
            .unwrap();
        assert_eq!(first_rx.await.unwrap().unwrap(), vec![1, 2, 3]);
        assert_eq!(
            pending.complete(7, Command::Ping, Ok(Vec::new())),
            Err(CorrelationError::UnknownRequest)
        );
        assert_eq!(
            pending.complete(8, Command::Ping, Ok(Vec::new())),
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
            .insert(20, Command::Ping, now + Duration::from_secs(2), later_tx)
            .unwrap();
        pending
            .insert(
                10,
                Command::GetCapabilities,
                now + Duration::from_secs(1),
                first_tx,
            )
            .unwrap();

        assert_eq!(pending.next_deadline(), Some(now + Duration::from_secs(1)));
        assert_eq!(pending.expired_ids(now + Duration::from_secs(1)), vec![10]);

        let timeout = ManagerError::timeout("request 10 timed out");
        pending.fail_one(10, timeout.clone());
        pending.fail_all(ManagerError::unavailable("stream invalidated"));
        assert_eq!(first_rx.await.unwrap().unwrap_err(), timeout);
        assert_eq!(
            later_rx.await.unwrap().unwrap_err().code,
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
