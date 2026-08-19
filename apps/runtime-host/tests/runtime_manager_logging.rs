use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use ai_voice_runtime_host::{ManagerErrorKind, RuntimeManager, RuntimeManagerConfig};
use ai_voice_telemetry::{Level, TelemetryConfig, initialize};
use serde_json::Value;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lifecycle_failures_emit_bounded_structured_events_without_resource_paths() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "aivs-runtime-manager-logging-{}-{nonce}",
        std::process::id()
    ));
    let guard = initialize(TelemetryConfig::new(directory.clone(), Level::Debug)).unwrap();
    let missing_runtime = directory.join("missing-runtime-密钥");
    let plugin = directory.join("mock-plugin-路径");
    let manager = RuntimeManager::new(RuntimeManagerConfig::new(
        missing_runtime.clone(),
        plugin.clone(),
    ))
    .unwrap();

    let error = manager.start_runtime().await.unwrap_err();
    assert_eq!(error.kind, ManagerErrorKind::Process);
    drop(manager);
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    drop(guard);

    let text = fs::read_to_string(directory.join("runtime-host.jsonl")).unwrap();
    let records: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(records.iter().any(|record| {
        record["message"] == "Runtime start requested" && record["generation"] == 1
    }));
    assert!(records.iter().any(|record| {
        record["message"] == "Runtime process spawn failed"
            && record["level"] == "error"
            && record["generation"] == 1
    }));
    assert!(!text.contains(missing_runtime.to_string_lossy().as_ref()));
    assert!(!text.contains(plugin.to_string_lossy().as_ref()));

    fs::remove_dir_all(directory).unwrap();
}
