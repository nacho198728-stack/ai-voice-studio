//! Unified UTF-8 JSONL telemetry for Rust control-plane components.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use tracing::Event;
use tracing::Level as TracingLevel;
use tracing::Subscriber;
use tracing::field::{Field, Visit};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::fmt::time::{FormatTime, UtcTime};
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::registry::LookupSpan;

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

/// Validated logging authority. Verbose levels are clamped unless debug was
/// explicitly enabled when the policy was constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoggingPolicy {
    debug_enabled: bool,
    effective_level: Level,
}

impl LoggingPolicy {
    pub const fn new(debug_enabled: bool, requested_level: Level) -> Self {
        let effective_level = match (debug_enabled, requested_level) {
            (false, Level::Trace | Level::Debug) => Level::Info,
            (_, level) => level,
        };
        Self {
            debug_enabled,
            effective_level,
        }
    }

    pub const fn debug_enabled(self) -> bool {
        self.debug_enabled
    }

    pub const fn effective_level(self) -> Level {
        self.effective_level
    }

    pub const fn as_str(self) -> &'static str {
        self.effective_level.as_str()
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
    policy: LoggingPolicy,
}

impl TelemetryConfig {
    pub fn new(directory: PathBuf, policy: LoggingPolicy) -> Self {
        Self { directory, policy }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub const fn policy(&self) -> LoggingPolicy {
        self.policy
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
        .log_internal_errors(false)
        .event_format(SchemaFormatter)
        .with_max_level(config.policy().effective_level().tracing())
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

#[derive(Default)]
struct SchemaFields {
    component: Option<String>,
    message: Option<String>,
    request_id: Option<u64>,
    generation: Option<u64>,
}

struct BoundedText {
    value: String,
    maximum: usize,
}

impl BoundedText {
    fn new(maximum: usize) -> Self {
        Self {
            value: String::with_capacity(maximum),
            maximum,
        }
    }
}

impl fmt::Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let remaining = self.maximum.saturating_sub(self.value.len());
        self.value.push_str(truncate_utf8(value, remaining));
        Ok(())
    }
}

fn bounded_owned(value: &str, maximum: usize) -> String {
    truncate_utf8(value, maximum).to_owned()
}

impl Visit for SchemaFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "component" => self.component = Some(bounded_owned(value, MAX_COMPONENT_BYTES)),
            "message" => self.message = Some(bounded_owned(value, MAX_MESSAGE_BYTES)),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "request_id" => self.request_id = Some(value),
            "generation" => self.generation = Some(value),
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if let Ok(value) = u64::try_from(value) {
            self.record_u64(field, value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let maximum = match field.name() {
            "component" => MAX_COMPONENT_BYTES,
            "message" => MAX_MESSAGE_BYTES,
            _ => return,
        };
        let mut output = BoundedText::new(maximum);
        if fmt::write(&mut output, format_args!("{value:?}")).is_ok() {
            match field.name() {
                "component" => self.component = Some(output.value),
                "message" => self.message = Some(output.value),
                _ => {}
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SchemaFormatter;

impl<S, N> FormatEvent<S, N> for SchemaFormatter
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _context: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut fields = SchemaFields::default();
        event.record(&mut fields);
        let component = fields
            .component
            .as_deref()
            .filter(|value| !value.is_empty())
            .or_else(|| {
                let target = event.metadata().target();
                (!target.is_empty()).then_some(target)
            })
            .unwrap_or("unknown");
        let message = fields.message.as_deref().unwrap_or("event");
        let component = truncate_utf8(component, MAX_COMPONENT_BYTES);
        let message = truncate_utf8(message, MAX_MESSAGE_BYTES);
        let level = match *event.metadata().level() {
            TracingLevel::TRACE => "trace",
            TracingLevel::DEBUG => "debug",
            TracingLevel::INFO => "info",
            TracingLevel::WARN => "warn",
            TracingLevel::ERROR => "error",
        };

        let mut timestamp = String::new();
        UtcTime::rfc_3339().format_time(&mut Writer::new(&mut timestamp))?;
        let timestamp = json_string(&timestamp)?;
        let component = json_string(component)?;
        let level = json_string(level)?;
        let message = json_string(message)?;
        write!(
            writer,
            "{{\"timestamp\":{timestamp},\"component\":{component},\"level\":{level},\"message\":{message}"
        )?;
        if let Some(request_id) = fields.request_id {
            write!(writer, ",\"request_id\":{request_id}")?;
        }
        if let Some(generation) = fields.generation {
            write!(writer, ",\"generation\":{generation}")?;
        }
        writer.write_str("}\n")
    }
}

fn json_string(value: &str) -> Result<String, fmt::Error> {
    serde_json::to_string(value).map_err(|_| fmt::Error)
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
