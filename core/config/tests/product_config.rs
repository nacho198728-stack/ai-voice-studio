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
  "debug": {
    "enabled": false,
    "log_level": "info",
    "development_log_directory": "logs/development"
  },
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

    assert_eq!(config.schema_version(), 1);
    assert_eq!(config.runtime().handshake_timeout_ms(), 2_000);
    assert_eq!(config.runtime().request_timeout_ms(), 2_000);
    assert_eq!(config.runtime().shutdown_timeout_ms(), 2_000);
    assert_eq!(config.runtime().max_in_flight(), 16);
    assert_eq!(config.runtime().command_queue_capacity(), 32);
    assert_eq!(config.runtime().event_queue_capacity(), 32);
    assert_eq!(config.runtime().stderr_tail_bytes(), 8_192);
    assert_eq!(config.runtime().restart_max_attempts(), 0);
    assert_eq!(config.backend().kind(), BackendKind::Mock);
    assert_eq!(config.backend().mock().work_iterations(), 0);
    assert!(!config.debug().enabled());
    assert_eq!(config.debug().log_level(), LogLevel::Info);
    assert_eq!(config.debug().effective_log_level(), LogLevel::Info);
    assert_eq!(
        config.debug().development_log_directory().to_str(),
        Some("logs/development")
    );
    assert!(!config.audio().enabled());
    assert_eq!(
        config.audio().input_device(),
        AudioDeviceSelection::Unconfigured
    );
    assert_eq!(
        config.audio().output_device(),
        AudioDeviceSelection::Unconfigured
    );
    assert_eq!(config.audio().sample_rate_hz(), None);
    assert_eq!(config.audio().buffer_frames(), None);
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
                "\"debug\": {\n    \"enabled\": false,\n    \"log_level\": \"info\",\n    \"development_log_directory\": \"logs/development\"\n  },",
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
fn debug_gate_and_portable_development_log_path_are_enforced() {
    for requested in ["trace", "debug"] {
        let text = VALID.replace(
            "\"log_level\": \"info\"",
            &format!("\"log_level\": \"{requested}\""),
        );
        let config = parse(&text).unwrap();
        assert_eq!(config.debug().effective_log_level(), LogLevel::Info);
    }

    let enabled = VALID
        .replace(
            "\"debug\": {\n    \"enabled\": false",
            "\"debug\": {\n    \"enabled\": true",
        )
        .replace("\"log_level\": \"info\"", "\"log_level\": \"trace\"");
    assert_eq!(
        parse(&enabled).unwrap().debug().effective_log_level(),
        LogLevel::Trace
    );

    for invalid in [
        "",
        ".",
        "..",
        "../logs",
        "logs/../escape",
        "/absolute/logs",
        "C:/absolute/logs",
        r"C:\absolute\logs",
        r"logs\windows",
        "logs//empty",
        "logs/./dot",
    ] {
        let text = VALID.replace(
            "\"development_log_directory\": \"logs/development\"",
            &format!("\"development_log_directory\": {invalid:?}"),
        );
        let error = parse(&text).expect_err(invalid);
        assert_eq!(error.kind(), ConfigErrorKind::Semantic, "{invalid}");
    }
    let overlong = "x".repeat(ai_voice_config::MAX_DEVELOPMENT_LOG_DIRECTORY_BYTES + 1);
    let text = VALID.replace("logs/development", &overlong);
    assert_eq!(parse(&text).unwrap_err().kind(), ConfigErrorKind::Semantic);

    let unicode = VALID.replace("logs/development", "日志/开发-🎵");
    let config = parse(&unicode).unwrap();
    let base = std::env::current_dir().unwrap();
    assert_eq!(
        config
            .debug()
            .resolve_development_log_directory(&base)
            .unwrap(),
        base.join("日志").join("开发-🎵")
    );
    assert!(
        config
            .debug()
            .resolve_development_log_directory(PathBuf::from("relative").as_path())
            .is_err()
    );
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
    struct Boundary {
        name: &'static str,
        original: &'static str,
        minimum: &'static str,
        below_minimum: &'static str,
        maximum: &'static str,
        above_maximum: &'static str,
    }
    let boundaries = [
        Boundary {
            name: "handshake timeout",
            original: "\"handshake_timeout_ms\": 2000",
            minimum: "1",
            below_minimum: "0",
            maximum: "300000",
            above_maximum: "300001",
        },
        Boundary {
            name: "request timeout",
            original: "\"request_timeout_ms\": 2000",
            minimum: "1",
            below_minimum: "0",
            maximum: "300000",
            above_maximum: "300001",
        },
        Boundary {
            name: "shutdown timeout",
            original: "\"shutdown_timeout_ms\": 2000",
            minimum: "1",
            below_minimum: "0",
            maximum: "300000",
            above_maximum: "300001",
        },
        Boundary {
            name: "max in flight",
            original: "\"max_in_flight\": 16",
            minimum: "1",
            below_minimum: "0",
            maximum: "64",
            above_maximum: "65",
        },
        Boundary {
            name: "command queue",
            original: "\"command_queue_capacity\": 32",
            minimum: "1",
            below_minimum: "0",
            maximum: "256",
            above_maximum: "257",
        },
        Boundary {
            name: "event queue",
            original: "\"event_queue_capacity\": 32",
            minimum: "1",
            below_minimum: "0",
            maximum: "256",
            above_maximum: "257",
        },
        Boundary {
            name: "stderr retention",
            original: "\"stderr_tail_bytes\": 8192",
            minimum: "0",
            below_minimum: "-1",
            maximum: "65536",
            above_maximum: "65537",
        },
        Boundary {
            name: "Mock work",
            original: "\"work_iterations\": 0",
            minimum: "0",
            below_minimum: "-1",
            maximum: "1000000",
            above_maximum: "1000001",
        },
        Boundary {
            name: "restart attempts",
            original: "\"restart_max_attempts\": 0",
            minimum: "0",
            below_minimum: "-1",
            maximum: "4294967295",
            above_maximum: "4294967296",
        },
    ];

    for boundary in boundaries {
        for (label, value, accepted) in [
            ("minimum", boundary.minimum, true),
            ("below minimum", boundary.below_minimum, false),
            ("maximum", boundary.maximum, true),
            ("above maximum", boundary.above_maximum, false),
        ] {
            let field_name = boundary.original.split(':').next().unwrap();
            let replacement = format!("{field_name}: {value}");
            assert!(
                VALID.contains(boundary.original),
                "{} fixture field",
                boundary.name
            );
            let text = VALID.replacen(boundary.original, &replacement, 1);
            assert_eq!(
                parse(&text).is_ok(),
                accepted,
                "{} {label}={value}",
                boundary.name
            );
        }
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
