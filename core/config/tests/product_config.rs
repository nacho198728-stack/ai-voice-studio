use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ai_voice_config::{
    AudioDeviceSelection, BackendKind, ConfigErrorKind, LogLevel, ProductConfig,
};

const VALID: &str = r#"{
  "schema_version": 1,
  "runtime": {
    "handshake_timeout_ms": 2000,
    "request_timeout_ms": 2000,
    "shutdown_timeout_ms": 2000,
    "max_in_flight": 16,
    "command_queue_capacity": 32,
    "event_queue_capacity": 32,
    "stderr_tail_bytes": 8192,
    "restart_max_attempts": 0
  },
  "backend": { "kind": "mock", "mock": { "work_iterations": 0 } },
  "debug": { "enabled": false, "log_level": "info" },
  "audio": {
    "enabled": false,
    "input_device": "unconfigured",
    "output_device": "unconfigured",
    "sample_rate_hz": null,
    "buffer_frames": null
  }
}"#;

fn parse(text: &str) -> Result<ProductConfig, ai_voice_config::ConfigError> {
    ProductConfig::load_from_bytes(text.as_bytes())
}

#[test]
fn checked_in_default_is_the_exact_validated_development_contract() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/config.json");
    let config = ProductConfig::load_from_path(&path).expect("checked-in config must load");

    assert_eq!(config.schema_version, 1);
    assert_eq!(config.runtime.handshake_timeout_ms, 2_000);
    assert_eq!(config.runtime.request_timeout_ms, 2_000);
    assert_eq!(config.runtime.shutdown_timeout_ms, 2_000);
    assert_eq!(config.runtime.max_in_flight, 16);
    assert_eq!(config.runtime.command_queue_capacity, 32);
    assert_eq!(config.runtime.event_queue_capacity, 32);
    assert_eq!(config.runtime.stderr_tail_bytes, 8_192);
    assert_eq!(config.runtime.restart_max_attempts, 0);
    assert_eq!(config.backend.kind, BackendKind::Mock);
    assert_eq!(config.backend.mock.work_iterations, 0);
    assert!(!config.debug.enabled);
    assert_eq!(config.debug.log_level, LogLevel::Info);
    assert!(!config.audio.enabled);
    assert_eq!(
        config.audio.input_device,
        AudioDeviceSelection::Unconfigured
    );
    assert_eq!(
        config.audio.output_device,
        AudioDeviceSelection::Unconfigured
    );
    assert_eq!(config.audio.sample_rate_hz, None);
    assert_eq!(config.audio.buffer_frames, None);
}

#[test]
fn structural_errors_are_rejected_instead_of_defaulted_or_ignored() {
    let cases = [
        (
            "unknown",
            VALID.replace(
                "\"schema_version\": 1,",
                "\"schema_version\": 1, \"extra\": true,",
            ),
        ),
        (
            "missing",
            VALID.replace(
                "\"debug\": { \"enabled\": false, \"log_level\": \"info\" },",
                "",
            ),
        ),
        (
            "duplicate",
            VALID.replace(
                "\"schema_version\": 1,",
                "\"schema_version\": 1, \"schema_version\": 1,",
            ),
        ),
        (
            "nested duplicate",
            VALID.replace(
                "\"max_in_flight\": 16,",
                "\"max_in_flight\": 16, \"max_in_flight\": 16,",
            ),
        ),
        (
            "wrong type",
            VALID.replace(
                "\"stderr_tail_bytes\": 8192",
                "\"stderr_tail_bytes\": \"8192\"",
            ),
        ),
    ];

    for (name, text) in cases {
        let error = parse(&text).expect_err(name);
        assert_eq!(error.kind(), ConfigErrorKind::InvalidDocument, "{name}");
        assert!(!error.to_string().is_empty(), "{name} must be actionable");
    }
}

#[test]
fn schema_backend_debug_and_audio_claims_are_strict() {
    let cases = [
        (
            "schema zero",
            VALID.replace("\"schema_version\": 1", "\"schema_version\": 0"),
            ConfigErrorKind::UnsupportedSchemaVersion,
        ),
        (
            "schema future",
            VALID.replace("\"schema_version\": 1", "\"schema_version\": 2"),
            ConfigErrorKind::UnsupportedSchemaVersion,
        ),
        (
            "backend unavailable",
            VALID.replace("\"kind\": \"mock\"", "\"kind\": \"unavailable\""),
            ConfigErrorKind::InvalidDocument,
        ),
        (
            "backend unknown",
            VALID.replace("\"kind\": \"mock\"", "\"kind\": \"unknown\""),
            ConfigErrorKind::InvalidDocument,
        ),
        (
            "debug level",
            VALID.replace("\"log_level\": \"info\"", "\"log_level\": \"verbose\""),
            ConfigErrorKind::InvalidDocument,
        ),
        (
            "audio enabled",
            VALID.replace(
                "\"enabled\": false,\n    \"input_device\"",
                "\"enabled\": true,\n    \"input_device\"",
            ),
            ConfigErrorKind::Semantic,
        ),
        (
            "input configured",
            VALID.replace(
                "\"input_device\": \"unconfigured\"",
                "\"input_device\": \"default\"",
            ),
            ConfigErrorKind::InvalidDocument,
        ),
        (
            "output configured",
            VALID.replace(
                "\"output_device\": \"unconfigured\"",
                "\"output_device\": \"default\"",
            ),
            ConfigErrorKind::InvalidDocument,
        ),
        (
            "sample rate",
            VALID.replace("\"sample_rate_hz\": null", "\"sample_rate_hz\": 48000"),
            ConfigErrorKind::Semantic,
        ),
        (
            "buffer frames",
            VALID.replace("\"buffer_frames\": null", "\"buffer_frames\": 256"),
            ConfigErrorKind::Semantic,
        ),
    ];

    for (name, text, expected) in cases {
        let error = parse(&text).expect_err(name);
        assert_eq!(error.kind(), expected, "{name}: {error}");
    }
}

#[test]
fn every_numeric_semantic_boundary_is_enforced() {
    let valid_cases = [
        VALID.replace(
            "\"handshake_timeout_ms\": 2000",
            "\"handshake_timeout_ms\": 1",
        ),
        VALID.replace(
            "\"request_timeout_ms\": 2000",
            "\"request_timeout_ms\": 300000",
        ),
        VALID.replace(
            "\"shutdown_timeout_ms\": 2000",
            "\"shutdown_timeout_ms\": 1",
        ),
        VALID.replace("\"max_in_flight\": 16", "\"max_in_flight\": 64"),
        VALID.replace(
            "\"command_queue_capacity\": 32",
            "\"command_queue_capacity\": 1",
        ),
        VALID.replace(
            "\"event_queue_capacity\": 32",
            "\"event_queue_capacity\": 256",
        ),
        VALID.replace("\"stderr_tail_bytes\": 8192", "\"stderr_tail_bytes\": 0"),
        VALID.replace(
            "\"stderr_tail_bytes\": 8192",
            "\"stderr_tail_bytes\": 65536",
        ),
        VALID.replace("\"work_iterations\": 0", "\"work_iterations\": 1000000"),
        VALID.replace(
            "\"restart_max_attempts\": 0",
            "\"restart_max_attempts\": 4294967295",
        ),
    ];
    for text in valid_cases {
        parse(&text).expect("inclusive boundary must load");
    }

    let invalid_cases = [
        VALID.replace(
            "\"handshake_timeout_ms\": 2000",
            "\"handshake_timeout_ms\": 0",
        ),
        VALID.replace(
            "\"request_timeout_ms\": 2000",
            "\"request_timeout_ms\": 300001",
        ),
        VALID.replace(
            "\"shutdown_timeout_ms\": 2000",
            "\"shutdown_timeout_ms\": 0",
        ),
        VALID.replace("\"max_in_flight\": 16", "\"max_in_flight\": 0"),
        VALID.replace("\"max_in_flight\": 16", "\"max_in_flight\": 65"),
        VALID.replace(
            "\"command_queue_capacity\": 32",
            "\"command_queue_capacity\": 0",
        ),
        VALID.replace(
            "\"command_queue_capacity\": 32",
            "\"command_queue_capacity\": 257",
        ),
        VALID.replace(
            "\"event_queue_capacity\": 32",
            "\"event_queue_capacity\": 0",
        ),
        VALID.replace(
            "\"event_queue_capacity\": 32",
            "\"event_queue_capacity\": 257",
        ),
        VALID.replace(
            "\"stderr_tail_bytes\": 8192",
            "\"stderr_tail_bytes\": 65537",
        ),
        VALID.replace("\"work_iterations\": 0", "\"work_iterations\": 1000001"),
        VALID.replace("\"work_iterations\": 0", "\"work_iterations\": -1"),
        VALID.replace(
            "\"restart_max_attempts\": 0",
            "\"restart_max_attempts\": 4294967296",
        ),
    ];
    for text in invalid_cases {
        assert!(parse(&text).is_err(), "accepted invalid boundary: {text}");
    }
}

#[test]
fn path_loading_is_read_only_and_missing_files_are_not_created() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("aivs-config-{}-{unique}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let existing = directory.join("config.json");
    let missing = directory.join("missing.json");
    fs::write(&existing, VALID).unwrap();
    let before = fs::read(&existing).unwrap();

    ProductConfig::load_from_path(&existing).unwrap();
    let missing_error = ProductConfig::load_from_path(&missing).unwrap_err();

    assert_eq!(fs::read(&existing).unwrap(), before);
    assert_eq!(missing_error.kind(), ConfigErrorKind::Io);
    assert!(!missing.exists());
    fs::remove_dir_all(directory).unwrap();
}
