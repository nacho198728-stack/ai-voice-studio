use std::path::PathBuf;
#[cfg(unix)]
use std::process::Stdio;
use std::time::Duration;

use ai_voice_capability::{EngineIdentity, RuntimeBackend};
use ai_voice_config::ProductConfig;
use ai_voice_runtime_host::{
    CapabilityProfileError, RuntimeManager, RuntimeManagerConfig, RuntimeState,
};

fn integration_paths() -> (PathBuf, PathBuf) {
    let runtime = std::env::var_os("AIVS_RUNTIME_PATH")
        .map(PathBuf::from)
        .expect("CTest must set AIVS_RUNTIME_PATH");
    let plugin = std::env::var_os("AIVS_MOCK_PLUGIN_PATH")
        .map(PathBuf::from)
        .expect("CTest must set AIVS_MOCK_PLUGIN_PATH");
    (runtime, plugin)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided native Runtime and Mock plugin paths"]
async fn real_child_start_concurrent_commands_pipeline_and_clean_stop() {
    let (runtime, plugin) = integration_paths();
    let config = RuntimeManagerConfig::new(runtime, plugin)
        .with_handshake_timeout(Duration::from_secs(2))
        .with_request_timeout(Duration::from_secs(2))
        .with_shutdown_timeout(Duration::from_secs(2));
    let manager = RuntimeManager::new(config).expect("valid explicit process configuration");

    let started = manager.start_runtime().await.expect("Runtime must start");
    assert_eq!(started.state, RuntimeState::Connected);
    assert_eq!(started.generation, 1);
    let pid = started.pid.expect("connected Runtime has a PID");
    let hello = started.hello.expect("connected Runtime retains Hello");
    assert_eq!(hello.runtime_version, "0.0.0");
    assert_eq!(hello.protocol_version, 1);
    assert_eq!(hello.generation, 1);

    let ping_bytes = vec![0, 1, 2, 0xff];
    let (ping, capabilities) = tokio::join!(manager.ping(&ping_bytes), manager.get_capabilities());
    assert_eq!(ping.expect("Ping succeeds"), ping_bytes);
    let capabilities = capabilities.expect("capability query succeeds");
    assert_eq!(capabilities.generation(), 1);
    assert_eq!(
        capabilities.capabilities().unwrap().backend(),
        RuntimeBackend::Mock
    );
    assert_eq!(
        capabilities.capabilities().unwrap().engine_identity(),
        Some(EngineIdentity::AivsMockV1)
    );

    let summary = manager
        .run_mock_pipeline()
        .await
        .expect("Mock pipeline succeeds");
    assert_eq!(summary.frames, 128);
    assert_eq!(summary.channels, 2);
    assert_eq!(summary.checksum, 0x3ecd_5190_f6f4_f725);
    assert_eq!(summary.process_call_count, 1);
    assert_eq!(summary.input_frame_count, 128);
    assert_eq!(summary.output_frame_count, 128);
    assert_eq!(summary.process_error_count, 0);

    let stopped = manager.stop_runtime().await.expect("Runtime must stop");
    assert_eq!(stopped.state, RuntimeState::Stopped);
    assert_eq!(stopped.pid, None);
    assert_eq!(stopped.generation, 1);
    assert_eq!(
        manager.stop_runtime().await.unwrap().state,
        RuntimeState::Stopped
    );
    assert_process_reaped(pid).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided native Runtime and Mock plugin paths"]
async fn dropping_the_last_manager_handle_reaps_the_child() {
    let (runtime, plugin) = integration_paths();
    let manager = RuntimeManager::new(RuntimeManagerConfig::new(runtime, plugin)).unwrap();
    let pid = manager.start_runtime().await.unwrap().pid.unwrap();

    drop(manager);

    assert_process_reaped(pid).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided native Runtime and Mock plugin paths"]
async fn idle_child_exit_is_published_as_crashed_and_can_be_started_again() {
    let (runtime, plugin) = integration_paths();
    let manager = RuntimeManager::new(RuntimeManagerConfig::new(runtime, plugin)).unwrap();
    let pid = manager.start_runtime().await.unwrap().pid.unwrap();
    let generation_one = manager.get_capabilities().await.unwrap();
    assert_eq!(generation_one.generation(), 1);

    terminate_process(pid).await;
    let crashed = wait_for_state(&manager, RuntimeState::Crashed).await;
    assert_eq!(crashed.pid, None);
    assert!(crashed.last_exit.is_some());

    let restarted = manager.start_runtime().await.unwrap();
    assert_eq!(restarted.state, RuntimeState::Connected);
    assert_eq!(restarted.generation, 2);
    assert_eq!(restarted.restart_policy.attempts_observed, 1);
    let product =
        ProductConfig::load_from_bytes(include_bytes!("../../../config/config.json")).unwrap();
    let stale = restarted
        .capability_profile(&product, &generation_one)
        .unwrap_err();
    assert_eq!(
        stale,
        CapabilityProfileError::StaleObservation {
            manager_generation: 2,
            observation_generation: 1,
        }
    );
    assert_eq!(manager.get_capabilities().await.unwrap().generation(), 2);
    manager.stop_runtime().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided native Runtime and Mock plugin paths"]
async fn request_timeout_invalidates_the_stream_and_reaps_the_child() {
    let (runtime, plugin) = integration_paths();
    let mut config =
        RuntimeManagerConfig::new(runtime, plugin).with_request_timeout(Duration::from_micros(100));
    config.mock_work_iterations = 1_000_000;
    let manager = RuntimeManager::new(config).unwrap();
    let pid = manager.start_runtime().await.unwrap().pid.unwrap();

    let error = manager.run_mock_pipeline().await.unwrap_err();
    assert_eq!(
        error.code,
        ai_voice_contracts::ErrorCode::RuntimeUnavailable
    );
    assert_eq!(error.kind, ai_voice_runtime_host::ManagerErrorKind::Timeout);
    let failed = wait_for_state(&manager, RuntimeState::Error).await;
    assert_eq!(
        failed.last_error.unwrap().kind,
        ai_voice_runtime_host::ManagerErrorKind::Timeout
    );
    assert_eq!(failed.pid, None);
    assert_process_reaped(pid).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided native Runtime and Mock plugin paths"]
async fn pre_hello_failure_preserves_bounded_stderr_diagnostics() {
    let (runtime, plugin) = integration_paths();
    let missing_plugin = plugin.with_file_name("aivs-intentionally-missing-plugin");
    let mut config = RuntimeManagerConfig::new(runtime, missing_plugin);
    config.stderr_tail_bytes = 64;
    let manager = RuntimeManager::new(config).unwrap();

    let error = manager.start_runtime().await.unwrap_err();
    assert_eq!(error.kind, ai_voice_runtime_host::ManagerErrorKind::Process);
    let status = manager.get_runtime_status().await.unwrap();
    assert_eq!(status.state, RuntimeState::Crashed);
    assert!(!status.stderr_tail.is_empty());
    assert!(status.stderr_tail.len() <= 64);
}

async fn wait_for_state(
    manager: &RuntimeManager,
    expected: RuntimeState,
) -> ai_voice_runtime_host::RuntimeStatus {
    for _ in 0..100 {
        let status = manager.get_runtime_status().await.unwrap();
        if status.state == expected {
            return status;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Runtime did not reach {expected:?}");
}

#[cfg(unix)]
async fn terminate_process(pid: u32) {
    let status = tokio::process::Command::new("/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .await
        .expect("kill probe must run");
    assert!(status.success());
}

#[cfg(windows)]
async fn terminate_process(pid: u32) {
    let status = tokio::process::Command::new("taskkill.exe")
        .args(["/PID", &pid.to_string(), "/F"])
        .status()
        .await
        .expect("taskkill probe must run");
    assert!(status.success());
}

#[cfg(unix)]
async fn assert_process_reaped(pid: u32) {
    for _ in 0..50 {
        let status = tokio::process::Command::new("/bin/kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .expect("kill -0 probe must run");
        if !status.success() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Runtime process {pid} remained alive or unreaped");
}

#[cfg(windows)]
async fn assert_process_reaped(pid: u32) {
    for _ in 0..50 {
        let output = tokio::process::Command::new("tasklist.exe")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .await
            .expect("tasklist probe must run");
        let listing = String::from_utf8_lossy(&output.stdout);
        if !listing.contains(&pid.to_string()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Runtime process {pid} remained alive");
}
