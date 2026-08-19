//! Versioned, read-only product configuration contract.

use std::fmt;
use std::fs;
use std::path::Path;

use serde::Deserialize;

pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const MAX_RUNTIME_TIMEOUT_MS: u32 = 300_000;
pub const MAX_RUNTIME_IN_FLIGHT: u32 = 64;
pub const MAX_RUNTIME_QUEUE_CAPACITY: u32 = 256;
pub const MAX_STDERR_TAIL_BYTES: u32 = 65_536;
pub const MAX_MOCK_WORK_ITERATIONS: u32 = 1_000_000;

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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProductConfig {
    pub schema_version: u32,
    pub runtime: RuntimeConfig,
    pub backend: BackendConfig,
    pub debug: DebugConfig,
    pub audio: AudioConfig,
}

impl ProductConfig {
    pub fn load_from_bytes(bytes: &[u8]) -> Result<Self, ConfigError> {
        let config: Self = serde_json::from_slice(bytes).map_err(|error| {
            ConfigError::new(
                ConfigErrorKind::InvalidDocument,
                format!(
                    "configuration JSON is invalid at line {}, column {}: {error}",
                    error.line(),
                    error.column()
                ),
            )
        })?;
        config.validate()?;
        Ok(config)
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

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::new(
                ConfigErrorKind::UnsupportedSchemaVersion,
                format!(
                    "configuration schema_version {} is unsupported; expected {CONFIG_SCHEMA_VERSION}",
                    self.schema_version
                ),
            ));
        }
        self.runtime.validate()?;
        validate_inclusive(
            "backend.mock.work_iterations",
            self.backend.mock.work_iterations,
            0,
            MAX_MOCK_WORK_ITERATIONS,
        )?;
        if self.audio.enabled
            || self.audio.input_device != AudioDeviceSelection::Unconfigured
            || self.audio.output_device != AudioDeviceSelection::Unconfigured
            || self.audio.sample_rate_hz.is_some()
            || self.audio.buffer_frames.is_some()
        {
            return Err(ConfigError::new(
                ConfigErrorKind::Semantic,
                "audio must remain disabled and unconfigured in Phase 0.5",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    pub handshake_timeout_ms: u32,
    pub request_timeout_ms: u32,
    pub shutdown_timeout_ms: u32,
    pub max_in_flight: u32,
    pub command_queue_capacity: u32,
    pub event_queue_capacity: u32,
    pub stderr_tail_bytes: u32,
    pub restart_max_attempts: u32,
}

impl RuntimeConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        for (name, value) in [
            ("handshake_timeout_ms", self.handshake_timeout_ms),
            ("request_timeout_ms", self.request_timeout_ms),
            ("shutdown_timeout_ms", self.shutdown_timeout_ms),
        ] {
            validate_inclusive(name, value, 1, MAX_RUNTIME_TIMEOUT_MS)?;
        }
        validate_inclusive(
            "max_in_flight",
            self.max_in_flight,
            1,
            MAX_RUNTIME_IN_FLIGHT,
        )?;
        validate_inclusive(
            "command_queue_capacity",
            self.command_queue_capacity,
            1,
            MAX_RUNTIME_QUEUE_CAPACITY,
        )?;
        validate_inclusive(
            "event_queue_capacity",
            self.event_queue_capacity,
            1,
            MAX_RUNTIME_QUEUE_CAPACITY,
        )?;
        validate_inclusive(
            "stderr_tail_bytes",
            self.stderr_tail_bytes,
            0,
            MAX_STDERR_TAIL_BYTES,
        )?;
        Ok(())
    }
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BackendConfig {
    pub kind: BackendKind,
    pub mock: MockBackendConfig,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    Mock,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MockBackendConfig {
    pub work_iterations: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DebugConfig {
    pub enabled: bool,
    pub log_level: LogLevel,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AudioConfig {
    pub enabled: bool,
    pub input_device: AudioDeviceSelection,
    pub output_device: AudioDeviceSelection,
    pub sample_rate_hz: Option<u32>,
    pub buffer_frames: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AudioDeviceSelection {
    Unconfigured,
}
