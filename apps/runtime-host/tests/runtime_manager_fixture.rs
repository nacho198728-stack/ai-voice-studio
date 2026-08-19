use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use ai_voice_runtime_host::{
    ManagerErrorKind, RuntimeExitReason, RuntimeManager, RuntimeManagerConfig, RuntimeState,
};

fn fixture_path() -> PathBuf {
    std::env::var_os("AIVS_RUNTIME_FIXTURE_PATH")
        .map(PathBuf::from)
        .expect("CTest must set AIVS_RUNTIME_FIXTURE_PATH")
}

fn manager(mode: &str) -> RuntimeManager {
    RuntimeManager::new(config(mode)).expect("fixture configuration is valid")
}

fn config(mode: &str) -> RuntimeManagerConfig {
    let plugin_mode = std::env::temp_dir().join(mode);
    let mut config = RuntimeManagerConfig::new(fixture_path(), plugin_mode);
    config.handshake_timeout = Duration::from_millis(500);
    config.request_timeout = Duration::from_millis(500);
    config.shutdown_timeout = Duration::from_millis(500);
    config
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn immediate_shutdown_response_and_exit_is_ordered_reliably() {
    for _ in 0..50 {
        let manager = manager("shutdown-race");
        manager.start_runtime().await.unwrap();
        let stopped = manager.stop_runtime().await.unwrap();
        assert_eq!(stopped.state, RuntimeState::Stopped);
        assert_eq!(stopped.pid, None);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn stdout_close_then_hang_cannot_override_short_handshake_deadline() {
    let mut config = config("stdout-close-hang");
    config.handshake_timeout = Duration::from_millis(100);
    config.shutdown_timeout = Duration::from_secs(2);
    let manager = RuntimeManager::new(config).unwrap();
    let (start, pid) = start_and_capture_pid(&manager).await;
    let began = Instant::now();

    let error = start.await.unwrap().unwrap_err();

    assert!(began.elapsed() < Duration::from_millis(500));
    assert_eq!(error.kind, ManagerErrorKind::Protocol);
    assert_pid_gone(pid).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn handshake_timeout_wrong_frame_and_truncation_are_reap_barriers() {
    for (mode, expected) in [
        ("handshake-hang", ManagerErrorKind::Timeout),
        ("prehello-response", ManagerErrorKind::Protocol),
        ("prehello-truncated", ManagerErrorKind::Protocol),
    ] {
        let mut config = config(mode);
        if mode == "handshake-hang" {
            config.handshake_timeout = Duration::from_millis(50);
        }
        let manager = RuntimeManager::new(config).unwrap();
        let (start, pid) = start_and_capture_pid(&manager).await;
        let error = start.await.unwrap().unwrap_err();
        assert_eq!(error.kind, expected, "mode {mode}");
        assert_pid_gone(pid).await;
        assert_eq!(manager.get_runtime_status().await.unwrap().pid, None);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn shutdown_timeout_returns_only_after_reap() {
    let mut config = config("shutdown-hang");
    config.shutdown_timeout = Duration::from_millis(50);
    let manager = RuntimeManager::new(config).unwrap();
    let pid = manager.start_runtime().await.unwrap().pid.unwrap();

    let error = manager.stop_runtime().await.unwrap_err();

    assert_eq!(error.kind, ManagerErrorKind::Timeout);
    assert_pid_gone(pid).await;
    let status = manager.get_runtime_status().await.unwrap();
    assert_eq!(status.state, RuntimeState::Error);
    assert_eq!(status.pid, None);
    assert_eq!(status.last_exit.unwrap().reason, RuntimeExitReason::Timeout);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn request_timeout_returns_only_after_reap() {
    let mut config = config("request-timeout");
    config.request_timeout = Duration::from_millis(50);
    let manager = RuntimeManager::new(config).unwrap();
    let pid = manager.start_runtime().await.unwrap().pid.unwrap();

    let error = manager.ping(b"timeout").await.unwrap_err();

    assert_eq!(error.kind, ManagerErrorKind::Timeout);
    assert_pid_gone(pid).await;
    let status = manager.get_runtime_status().await.unwrap();
    assert_eq!(status.state, RuntimeState::Error);
    assert_eq!(status.last_exit.unwrap().reason, RuntimeExitReason::Timeout);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn stop_is_reserved_when_normal_pending_is_full_or_cancelled() {
    for mode in ["full-pending", "cancelled-pending"] {
        let mut config = config(mode);
        config.max_in_flight = 1;
        config.request_timeout = Duration::from_secs(2);
        let manager = RuntimeManager::new(config).unwrap();
        manager.start_runtime().await.unwrap();
        let ping_manager = manager.clone();
        let mut ping = Some(tokio::spawn(
            async move { ping_manager.ping(b"held").await },
        ));
        wait_for_stderr(&manager, b"ping-held").await;
        if mode == "cancelled-pending" {
            let cancelled = ping.take().unwrap();
            cancelled.abort();
            let _ = cancelled.await;
        }

        let stopped = manager.stop_runtime().await.unwrap();

        assert_eq!(stopped.state, RuntimeState::Stopped);
        if mode == "full-pending" {
            assert_eq!(ping.take().unwrap().await.unwrap().unwrap(), b"held");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn request_eof_and_fatal_correlation_reap_before_reply() {
    for (mode, expected) in [
        ("request-close-hang", ManagerErrorKind::Protocol),
        ("correlation-mismatch", ManagerErrorKind::Protocol),
    ] {
        let manager = manager(mode);
        let pid = manager.start_runtime().await.unwrap().pid.unwrap();
        let error = manager.ping(b"request").await.unwrap_err();
        assert_eq!(error.kind, expected, "mode {mode}");
        assert_pid_gone(pid).await;
        let status = manager.get_runtime_status().await.unwrap();
        assert_eq!(status.pid, None);
        assert_eq!(
            status.last_exit.unwrap().reason,
            RuntimeExitReason::ProtocolFailure
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn child_receives_no_inherited_parent_environment() {
    assert_eq!(
        std::env::var("AIVS_TEST_SECRET").unwrap(),
        "must-not-inherit"
    );
    let manager = manager("environment");
    manager.start_runtime().await.unwrap();

    assert_eq!(manager.ping(b"environment").await.unwrap(), b"clean");
    manager.stop_runtime().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn final_stderr_is_drained_after_child_exit() {
    let manager = manager("stderr-exit");

    manager.start_runtime().await.unwrap_err();

    let status = manager.get_runtime_status().await.unwrap();
    assert!(
        status.stderr_tail.ends_with(b"final-stderr-diagnostic"),
        "stderr tail was {:?}",
        String::from_utf8_lossy(&status.stderr_tail)
    );
}

async fn start_and_capture_pid(
    manager: &RuntimeManager,
) -> (
    tokio::task::JoinHandle<
        Result<ai_voice_runtime_host::RuntimeStatus, ai_voice_runtime_host::ManagerError>,
    >,
    u32,
) {
    let start_manager = manager.clone();
    let start = tokio::spawn(async move { start_manager.start_runtime().await });
    for _ in 0..100 {
        let status = manager.get_runtime_status().await.unwrap();
        if status.state == RuntimeState::Starting
            && let Some(pid) = status.pid
        {
            return (start, pid);
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    panic!("fixture did not publish a starting PID");
}

async fn wait_for_stderr(manager: &RuntimeManager, expected: &[u8]) {
    for _ in 0..100 {
        if manager
            .get_runtime_status()
            .await
            .unwrap()
            .stderr_tail
            .windows(expected.len())
            .any(|window| window == expected)
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    panic!("fixture did not confirm held Ping");
}

async fn assert_pid_gone(pid: u32) {
    assert!(
        !process_exists(pid).await,
        "PID {pid} was live after API reply"
    );
}

#[cfg(unix)]
async fn process_exists(pid: u32) -> bool {
    tokio::process::Command::new("/bin/kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .expect("kill -0 probe must run")
        .success()
}

#[cfg(windows)]
async fn process_exists(pid: u32) -> bool {
    let output = tokio::process::Command::new("tasklist.exe")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .await
        .expect("tasklist probe must run");
    String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
}
