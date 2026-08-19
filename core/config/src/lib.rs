//! Versioned, read-only product configuration contract.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const MAX_RUNTIME_TIMEOUT_MS: u32 = 300_000;
pub const MAX_RUNTIME_IN_FLIGHT: u32 = 64;
pub const MAX_RUNTIME_QUEUE_CAPACITY: u32 = 256;
pub const MAX_STDERR_TAIL_BYTES: u32 = 65_536;
pub const MAX_MOCK_WORK_ITERATIONS: u32 = 1_000_000;
pub const MAX_DEVELOPMENT_LOG_DIRECTORY_BYTES: usize = 240;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigErrorKind {
    Io,
    InvalidDocument,
    UnsupportedSchemaVersion,
    Semantic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError {
    kind: ConfigErrorKind,
    message: String,
}

impl ConfigError {
    fn new(kind: ConfigErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub const fn kind(&self) -> ConfigErrorKind {
        self.kind
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for ConfigError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProductConfig {
    schema_version: u32,
    runtime: RawRuntimeConfig,
    backend: RawBackendConfig,
    debug: RawDebugConfig,
    audio: RawAudioConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRuntimeConfig {
    handshake_timeout_ms: u32,
    request_timeout_ms: u32,
    shutdown_timeout_ms: u32,
    max_in_flight: u32,
    command_queue_capacity: u32,
    event_queue_capacity: u32,
    stderr_tail_bytes: u32,
    restart_max_attempts: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBackendConfig {
    kind: BackendKind,
    mock: RawMockBackendConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMockBackendConfig {
    work_iterations: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDebugConfig {
    enabled: bool,
    log_level: LogLevel,
    development_log_directory: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAudioConfig {
    enabled: bool,
    input_device: AudioDeviceSelection,
    output_device: AudioDeviceSelection,
    sample_rate_hz: Option<u32>,
    buffer_frames: Option<u32>,
}

/// A validated product configuration.
///
/// Composite configuration deliberately has no public Serde deserialization
/// path; callers must use the validating loaders.
///
/// ```compile_fail
/// use ai_voice_config::ProductConfig;
/// let _: ProductConfig = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductConfig {
    schema_version: u32,
    runtime: RuntimeConfig,
    backend: BackendConfig,
    debug: DebugConfig,
    audio: AudioConfig,
}

impl ProductConfig {
    pub fn load_from_bytes(bytes: &[u8]) -> Result<Self, ConfigError> {
        let raw: RawProductConfig = serde_json::from_slice(bytes).map_err(|error| {
            ConfigError::new(
                ConfigErrorKind::InvalidDocument,
                format!(
                    "configuration JSON is invalid at line {}, column {}: {error}",
                    error.line(),
                    error.column()
                ),
            )
        })?;
        validate_raw(&raw)?;
        Ok(Self {
            schema_version: raw.schema_version,
            runtime: RuntimeConfig {
                handshake_timeout_ms: raw.runtime.handshake_timeout_ms,
                request_timeout_ms: raw.runtime.request_timeout_ms,
                shutdown_timeout_ms: raw.runtime.shutdown_timeout_ms,
                max_in_flight: raw.runtime.max_in_flight,
                command_queue_capacity: raw.runtime.command_queue_capacity,
                event_queue_capacity: raw.runtime.event_queue_capacity,
                stderr_tail_bytes: raw.runtime.stderr_tail_bytes,
                restart_max_attempts: raw.runtime.restart_max_attempts,
            },
            backend: BackendConfig {
                kind: raw.backend.kind,
                mock: MockBackendConfig {
                    work_iterations: raw.backend.mock.work_iterations,
                },
            },
            debug: DebugConfig {
                enabled: raw.debug.enabled,
                log_level: raw.debug.log_level,
                development_log_directory: PathBuf::from(&raw.debug.development_log_directory),
            },
            audio: AudioConfig {
                enabled: raw.audio.enabled,
                input_device: raw.audio.input_device,
                output_device: raw.audio.output_device,
                sample_rate_hz: raw.audio.sample_rate_hz,
                buffer_frames: raw.audio.buffer_frames,
            },
        })
    }

    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let bytes = fs::read(path).map_err(|error| {
            ConfigError::new(
                ConfigErrorKind::Io,
                format!("cannot read configuration {}: {error}", path.display()),
            )
        })?;
        Self::load_from_bytes(&bytes)
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn runtime(&self) -> &RuntimeConfig {
        &self.runtime
    }

    pub const fn backend(&self) -> &BackendConfig {
        &self.backend
    }

    pub const fn debug(&self) -> &DebugConfig {
        &self.debug
    }

    pub const fn audio(&self) -> &AudioConfig {
        &self.audio
    }
}

fn validate_raw(raw: &RawProductConfig) -> Result<(), ConfigError> {
    if raw.schema_version != CONFIG_SCHEMA_VERSION {
        return Err(ConfigError::new(
            ConfigErrorKind::UnsupportedSchemaVersion,
            format!(
                "configuration schema_version {} is unsupported; expected {CONFIG_SCHEMA_VERSION}",
                raw.schema_version
            ),
        ));
    }
    for (name, value) in [
        ("handshake_timeout_ms", raw.runtime.handshake_timeout_ms),
        ("request_timeout_ms", raw.runtime.request_timeout_ms),
        ("shutdown_timeout_ms", raw.runtime.shutdown_timeout_ms),
    ] {
        validate_inclusive(name, value, 1, MAX_RUNTIME_TIMEOUT_MS)?;
    }
    validate_inclusive(
        "max_in_flight",
        raw.runtime.max_in_flight,
        1,
        MAX_RUNTIME_IN_FLIGHT,
    )?;
    validate_inclusive(
        "command_queue_capacity",
        raw.runtime.command_queue_capacity,
        1,
        MAX_RUNTIME_QUEUE_CAPACITY,
    )?;
    validate_inclusive(
        "event_queue_capacity",
        raw.runtime.event_queue_capacity,
        1,
        MAX_RUNTIME_QUEUE_CAPACITY,
    )?;
    validate_inclusive(
        "stderr_tail_bytes",
        raw.runtime.stderr_tail_bytes,
        0,
        MAX_STDERR_TAIL_BYTES,
    )?;
    validate_inclusive(
        "backend.mock.work_iterations",
        raw.backend.mock.work_iterations,
        0,
        MAX_MOCK_WORK_ITERATIONS,
    )?;
    validate_development_log_directory(&raw.debug.development_log_directory)?;
    if raw.audio.enabled
        || raw.audio.input_device != AudioDeviceSelection::Unconfigured
        || raw.audio.output_device != AudioDeviceSelection::Unconfigured
        || raw.audio.sample_rate_hz.is_some()
        || raw.audio.buffer_frames.is_some()
    {
        return Err(ConfigError::new(
            ConfigErrorKind::Semantic,
            "audio must remain disabled and unconfigured in Phase 0.5",
        ));
    }
    Ok(())
}

fn validate_development_log_directory(value: &str) -> Result<(), ConfigError> {
    let invalid = value.is_empty()
        || value.len() > MAX_DEVELOPMENT_LOG_DIRECTORY_BYTES
        || value.starts_with('/')
        || value.contains(['\\', ':', '\0'])
        || value
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..");
    if invalid {
        return Err(ConfigError::new(
            ConfigErrorKind::Semantic,
            "development_log_directory must be a bounded portable relative path without traversal",
        ));
    }
    Ok(())
}

fn validate_inclusive(
    name: &str,
    value: u32,
    minimum: u32,
    maximum: u32,
) -> Result<(), ConfigError> {
    if value < minimum || value > maximum {
        return Err(ConfigError::new(
            ConfigErrorKind::Semantic,
            format!("{name} must be between {minimum} and {maximum}, got {value}"),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    handshake_timeout_ms: u32,
    request_timeout_ms: u32,
    shutdown_timeout_ms: u32,
    max_in_flight: u32,
    command_queue_capacity: u32,
    event_queue_capacity: u32,
    stderr_tail_bytes: u32,
    restart_max_attempts: u32,
}

impl RuntimeConfig {
    pub const fn handshake_timeout_ms(&self) -> u32 {
        self.handshake_timeout_ms
    }

    pub const fn request_timeout_ms(&self) -> u32 {
        self.request_timeout_ms
    }

    pub const fn shutdown_timeout_ms(&self) -> u32 {
        self.shutdown_timeout_ms
    }

    pub const fn max_in_flight(&self) -> u32 {
        self.max_in_flight
    }

    pub const fn command_queue_capacity(&self) -> u32 {
        self.command_queue_capacity
    }

    pub const fn event_queue_capacity(&self) -> u32 {
        self.event_queue_capacity
    }

    pub const fn stderr_tail_bytes(&self) -> u32 {
        self.stderr_tail_bytes
    }

    pub const fn restart_max_attempts(&self) -> u32 {
        self.restart_max_attempts
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendConfig {
    kind: BackendKind,
    mock: MockBackendConfig,
}

impl BackendConfig {
    pub const fn kind(&self) -> BackendKind {
        self.kind
    }

    pub const fn mock(&self) -> &MockBackendConfig {
        &self.mock
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    Mock,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MockBackendConfig {
    work_iterations: u32,
}

impl MockBackendConfig {
    pub const fn work_iterations(&self) -> u32 {
        self.work_iterations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugConfig {
    enabled: bool,
    log_level: LogLevel,
    development_log_directory: PathBuf,
}

impl DebugConfig {
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn log_level(&self) -> LogLevel {
        self.log_level
    }

    pub const fn effective_log_level(&self) -> LogLevel {
        match (self.enabled, self.log_level) {
            (false, LogLevel::Trace | LogLevel::Debug) => LogLevel::Info,
            (_, level) => level,
        }
    }

    pub fn development_log_directory(&self) -> &Path {
        &self.development_log_directory
    }

    pub fn resolve_development_log_directory(
        &self,
        host_base: &Path,
    ) -> Result<PathBuf, ConfigError> {
        if !host_base.is_absolute() {
            return Err(ConfigError::new(
                ConfigErrorKind::Semantic,
                "development log host base must be an explicit absolute path",
            ));
        }
        Ok(host_base.join(&self.development_log_directory))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioConfig {
    enabled: bool,
    input_device: AudioDeviceSelection,
    output_device: AudioDeviceSelection,
    sample_rate_hz: Option<u32>,
    buffer_frames: Option<u32>,
}

impl AudioConfig {
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn input_device(&self) -> AudioDeviceSelection {
        self.input_device
    }

    pub const fn output_device(&self) -> AudioDeviceSelection {
        self.output_device
    }

    pub const fn sample_rate_hz(&self) -> Option<u32> {
        self.sample_rate_hz
    }

    pub const fn buffer_frames(&self) -> Option<u32> {
        self.buffer_frames
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AudioDeviceSelection {
    Unconfigured,
}
