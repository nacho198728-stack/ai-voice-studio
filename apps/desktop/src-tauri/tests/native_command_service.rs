use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ai_voice_desktop::{ArtifactLayout, DesktopApplication, current_target_triple};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires tools/scripts/stage-desktop-native.mjs output"]
async fn staged_command_service_runs_the_real_runtime_c_abi_mock_chain_and_reaps() {
    let tauri_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let paths = ArtifactLayout::new(current_target_triple())
        .unwrap()
        .development_paths(&tauri_root);
    paths
        .validate_development()
        .expect("run pnpm desktop:native:prepare before this test");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let app_data = std::env::temp_dir().join(format!(
        "aivs-desktop-native-声音-{}-{nonce}",
        std::process::id()
    ));
    let desktop = DesktopApplication::initialize(paths, app_data.clone())
        .await
        .unwrap();
    let service = desktop.service();

    assert_eq!(service.get_runtime_status().await.unwrap().state, "stopped");
    let connected = service.start_runtime().await.unwrap();
    assert_eq!(connected.state, "connected");
    assert_eq!(connected.generation, 1);

    let capabilities = service.get_runtime_capabilities().await.unwrap();
    assert_eq!(capabilities.backend, "mock");
    assert_eq!(
        capabilities.engine_identity.as_deref(),
        Some("aivs-mock-v1")
    );
    let summary = service.run_mock_pipeline().await.unwrap();
    assert_eq!(summary.input_frames, 128);
    assert_eq!(summary.output_frames, 128);
    assert_eq!(summary.checksum, "3ecd5190f6f4f725");
    assert_eq!(summary.process_call_count, 1);
    assert_eq!(summary.process_error_count, 0);

    let stopped = service.stop_runtime().await.unwrap();
    assert_eq!(stopped.state, "stopped");
    assert_eq!(service.get_runtime_status().await.unwrap().state, "stopped");
    desktop.shutdown().await.unwrap();
    drop(desktop);

    let native_log = fs::read_to_string(app_data.join("logs/development/voice-runtime.jsonl"))
        .expect("real C++ Runtime must create its owned structured log");
    assert!(native_log.contains("Runtime process initializing"));
    assert!(native_log.contains("Runtime shutdown completed"));
    let rust_log = fs::read_to_string(app_data.join("logs/development/runtime-host.jsonl"))
        .expect("desktop composition root must retain and flush Rust telemetry");
    assert!(rust_log.contains("Runtime start requested"));
    assert!(rust_log.contains("Runtime shutdown completed"));
    fs::remove_dir_all(app_data).unwrap();
}
