use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ai_voice_telemetry::{
    EventFields, Level, TelemetryConfig, TelemetryErrorKind, emit, initialize,
};
use serde_json::Value;

fn unique_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "aivs-telemetry-{name}-{}-{nonce}",
        std::process::id()
    ))
}

fn parse_lines(path: &PathBuf) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("each record must be one JSON object"))
        .collect()
}

#[test]
fn global_initialization_routes_jsonl_flushes_and_rejects_reinitialization() {
    let blocked = unique_directory("blocked");
    fs::write(&blocked, b"not a directory").unwrap();
    let failure = initialize(TelemetryConfig::new(blocked.clone(), Level::Info)).unwrap_err();
    assert_eq!(failure.kind(), TelemetryErrorKind::Directory);
    assert!(failure.to_string().contains("directory"));

    let blocked_file = unique_directory("blocked-file");
    fs::create_dir_all(blocked_file.join("runtime-host.jsonl")).unwrap();
    let file_failure =
        initialize(TelemetryConfig::new(blocked_file.clone(), Level::Info)).unwrap_err();
    assert_eq!(file_failure.kind(), TelemetryErrorKind::File);

    let directory = unique_directory("日志-🎵");
    let guard = initialize(TelemetryConfig::new(directory.clone(), Level::Debug)).unwrap();
    emit(
        Level::Info,
        "runtime-host",
        "quoted \"line\"\n音乐",
        EventFields {
            request_id: Some(42),
            generation: Some(7),
        },
    );
    emit(
        Level::Trace,
        "runtime-host",
        "hidden",
        EventFields::default(),
    );
    emit(
        Level::Warn,
        &"组".repeat(80),
        &"声".repeat(300),
        EventFields::default(),
    );

    let repeated = initialize(TelemetryConfig::new(directory.clone(), Level::Info)).unwrap_err();
    assert_eq!(repeated.kind(), TelemetryErrorKind::AlreadyInitialized);
    drop(guard);

    let records = parse_lines(&directory.join("runtime-host.jsonl"));
    assert_eq!(records.len(), 2);
    let record = records[0].as_object().unwrap();
    assert_eq!(record.len(), 6);
    assert!(record["timestamp"].as_str().unwrap().ends_with('Z'));
    assert!(record["timestamp"].as_str().unwrap().contains('.'));
    assert_eq!(record["component"], "runtime-host");
    assert_eq!(record["level"], "info");
    assert_eq!(record["message"], "quoted \"line\"\n音乐");
    assert_eq!(record["request_id"], 42);
    assert_eq!(record["generation"], 7);
    let bounded = records[1].as_object().unwrap();
    assert_eq!(bounded.len(), 4);
    assert!(bounded["component"].as_str().unwrap().len() <= 64);
    assert!(bounded["message"].as_str().unwrap().len() <= 512);
    assert_eq!(bounded["level"], "warn");

    fs::remove_dir_all(directory).unwrap();
    fs::remove_file(blocked).unwrap();
    fs::remove_dir_all(blocked_file).unwrap();
}
