//! Unified UTF-8 JSONL telemetry for Rust control-plane components.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use tracing::Level as TracingLevel;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::fmt::writer::MakeWriterExt;

pub const MAX_COMPONENT_BYTES: usize = 64;
pub const MAX_MESSAGE_BYTES: usize = 512;
pub const LOG_FILE_NAME: &str = "runtime-host.jsonl";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    const fn tracing(self) -> TracingLevel {
        match self {
            Self::Trace => TracingLevel::TRACE,
            Self::Debug => TracingLevel::DEBUG,
            Self::Info => TracingLevel::INFO,
            Self::Warn => TracingLevel::WARN,
            Self::Error => TracingLevel::ERROR,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EventFields {
    pub request_id: Option<u64>,
    pub generation: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TelemetryConfig {
    directory: PathBuf,
    level: Level,
}

impl TelemetryConfig {
    pub fn new(directory: PathBuf, level: Level) -> Self {
        Self { directory, level }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub const fn level(&self) -> Level {
        self.level
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TelemetryErrorKind {
    InvalidConfiguration,
    Directory,
    File,
    AlreadyInitialized,
}

#[derive(Debug)]
pub struct TelemetryError {
    kind: TelemetryErrorKind,
    message: String,
}

impl TelemetryError {
    fn new(kind: TelemetryErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub const fn kind(&self) -> TelemetryErrorKind {
        self.kind
    }
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for TelemetryError {}

/// Owns the bounded background writer. Dropping it drains and flushes records.
#[derive(Debug)]
pub struct TelemetryGuard {
    _file_guard: WorkerGuard,
}

pub fn initialize(config: TelemetryConfig) -> Result<TelemetryGuard, TelemetryError> {
    validate_directory(config.directory())?;
    fs::create_dir_all(config.directory()).map_err(|error| {
        TelemetryError::new(
            TelemetryErrorKind::Directory,
            format!("cannot create telemetry directory: {error}"),
        )
    })?;
    if !config.directory().is_dir() {
        return Err(TelemetryError::new(
            TelemetryErrorKind::Directory,
            "telemetry directory path is not a directory",
        ));
    }

    let file = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::NEVER)
        .filename_prefix(LOG_FILE_NAME)
        .build(config.directory())
        .map_err(|error| {
            TelemetryError::new(
                TelemetryErrorKind::File,
                format!("cannot open telemetry file: {error}"),
            )
        })?;
    let (file_writer, file_guard) = tracing_appender::non_blocking(file);
    let writer = file_writer.and(std::io::stderr);
    let subscriber = tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_target(false)
        .with_level(false)
        .with_timer(UtcTime::rfc_3339())
        .with_max_level(config.level().tracing())
        .with_writer(writer)
        .finish();
    tracing::subscriber::set_global_default(subscriber).map_err(|_| {
        TelemetryError::new(
            TelemetryErrorKind::AlreadyInitialized,
            "global telemetry subscriber is already initialized",
        )
    })?;
    Ok(TelemetryGuard {
        _file_guard: file_guard,
    })
}

fn validate_directory(directory: &Path) -> Result<(), TelemetryError> {
    if !directory.is_absolute() {
        return Err(TelemetryError::new(
            TelemetryErrorKind::InvalidConfiguration,
            "telemetry directory must be an explicit absolute path",
        ));
    }
    Ok(())
}

pub fn emit(level: Level, component: &str, message: &str, fields: EventFields) {
    let component = truncate_utf8(component, MAX_COMPONENT_BYTES);
    if component.is_empty() {
        return;
    }
    let message = truncate_utf8(message, MAX_MESSAGE_BYTES);
    match (fields.request_id, fields.generation) {
        (Some(request_id), Some(generation)) => {
            emit_both(level, component, message, request_id, generation)
        }
        (Some(request_id), None) => emit_request(level, component, message, request_id),
        (None, Some(generation)) => emit_generation(level, component, message, generation),
        (None, None) => emit_plain(level, component, message),
    }
}

fn emit_plain(level: Level, component: &str, message: &str) {
    let level_name = level.as_str();
    match level {
        Level::Trace => tracing::trace!(component, level = level_name, message),
        Level::Debug => tracing::debug!(component, level = level_name, message),
        Level::Info => tracing::info!(component, level = level_name, message),
        Level::Warn => tracing::warn!(component, level = level_name, message),
        Level::Error => tracing::error!(component, level = level_name, message),
    }
}

fn emit_request(level: Level, component: &str, message: &str, request_id: u64) {
    let level_name = level.as_str();
    match level {
        Level::Trace => tracing::trace!(component, level = level_name, message, request_id),
        Level::Debug => tracing::debug!(component, level = level_name, message, request_id),
        Level::Info => tracing::info!(component, level = level_name, message, request_id),
        Level::Warn => tracing::warn!(component, level = level_name, message, request_id),
        Level::Error => tracing::error!(component, level = level_name, message, request_id),
    }
}

fn emit_generation(level: Level, component: &str, message: &str, generation: u64) {
    let level_name = level.as_str();
    match level {
        Level::Trace => tracing::trace!(component, level = level_name, message, generation),
        Level::Debug => tracing::debug!(component, level = level_name, message, generation),
        Level::Info => tracing::info!(component, level = level_name, message, generation),
        Level::Warn => tracing::warn!(component, level = level_name, message, generation),
        Level::Error => tracing::error!(component, level = level_name, message, generation),
    }
}

fn emit_both(level: Level, component: &str, message: &str, request_id: u64, generation: u64) {
    let level_name = level.as_str();
    match level {
        Level::Trace => {
            tracing::trace!(
                component,
                level = level_name,
                message,
                request_id,
                generation
            )
        }
        Level::Debug => {
            tracing::debug!(
                component,
                level = level_name,
                message,
                request_id,
                generation
            )
        }
        Level::Info => {
            tracing::info!(
                component,
                level = level_name,
                message,
                request_id,
                generation
            )
        }
        Level::Warn => {
            tracing::warn!(
                component,
                level = level_name,
                message,
                request_id,
                generation
            )
        }
        Level::Error => {
            tracing::error!(
                component,
                level = level_name,
                message,
                request_id,
                generation
            )
        }
    }
}

fn truncate_utf8(value: &str, maximum: usize) -> &str {
    if value.len() <= maximum {
        return value;
    }
    let mut boundary = maximum;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &value[..boundary]
}
