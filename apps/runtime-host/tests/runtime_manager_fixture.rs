use std::path::PathBuf;
#[cfg(unix)]
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ai_voice_config::ProductConfig;
use ai_voice_runtime_host::{
    CapabilityAvailability, CapabilityObservationKind, CapabilityProfileError, ManagerErrorKind,
    RuntimeExitReason, RuntimeManager, RuntimeManagerConfig, RuntimeState,
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

fn product_config() -> ProductConfig {
    ProductConfig::load_from_bytes(include_bytes!("../../../config/config.json")).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn actor_mints_not_evaluated_for_the_current_generation() {
    let manager = manager("shutdown-race");
    let status = manager.start_runtime().await.unwrap();

    let observation = manager.capabilities_not_evaluated().await.unwrap();
    assert_eq!(observation.generation(), status.generation);
    assert_eq!(observation.kind(), CapabilityObservationKind::NotEvaluated);
    assert_eq!(observation.capabilities(), None);
    assert_eq!(observation.error(), None);
    let profile = status
        .capability_profile(&product_config(), &observation)
        .unwrap();
    assert_eq!(
        profile.runtime().availability(),
        CapabilityAvailability::Available
    );
    assert_eq!(
        profile.engine().availability(),
        CapabilityAvailability::NotEvaluated
    );

    manager.stop_runtime().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn failed_capability_query_is_actor_bound_and_stale_after_restart() {
    let manager = manager("capability-error");
    let first = manager.start_runtime().await.unwrap();
    let old_failure = manager.get_capabilities().await.unwrap();
    assert_eq!(old_failure.generation(), first.generation);
    assert_eq!(old_failure.kind(), CapabilityObservationKind::Inconclusive);
    assert_eq!(old_failure.error().unwrap().kind, ManagerErrorKind::Remote);

    manager.stop_runtime().await.unwrap();
    let restarted = manager.start_runtime().await.unwrap();
    assert_eq!(restarted.generation, first.generation + 1);
    let stale = restarted
        .capability_profile(&product_config(), &old_failure)
        .unwrap_err();
    assert_eq!(
        stale,
        CapabilityProfileError::StaleObservation {
            manager_generation: restarted.generation,
            observation_generation: first.generation,
        }
    );

    let fresh_failure = manager.get_capabilities().await.unwrap();
    assert_eq!(fresh_failure.generation(), restarted.generation);
    let profile = restarted
        .capability_profile(&product_config(), &fresh_failure)
        .unwrap();
    assert_eq!(
        profile.runtime().availability(),
        CapabilityAvailability::Available
    );
    assert_eq!(
        profile.engine().availability(),
        CapabilityAvailability::Unknown
    );
    manager.stop_runtime().await.unwrap();
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
async fn concurrent_stops_join_one_shutdown_and_both_complete_after_reap() {
    let manager = manager("shutdown-delay");
    let pid = manager.start_runtime().await.unwrap().pid.unwrap();
    let first_manager = manager.clone();
    let first = tokio::spawn(async move { first_manager.stop_runtime().await });
    wait_for_stderr(&manager, b"shutdown-received").await;

    let second_manager = manager.clone();
    let second = tokio::spawn(async move { second_manager.stop_runtime().await });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(
        !first.is_finished(),
        "first Stop completed before process reap"
    );
    assert!(
        !second.is_finished(),
        "second Stop did not join the in-progress reap"
    );

    let first_status = first.await.unwrap().unwrap();
    let second_status = second.await.unwrap().unwrap();
    assert_eq!(first_status, second_status);
    assert_eq!(first_status.state, RuntimeState::Stopped);
    assert_eq!(first_status.pid, None);
    assert_pid_gone(pid).await;
    let status = manager.get_runtime_status().await.unwrap();
    assert_eq!(occurrences(&status.stderr_tail, b"shutdown-received"), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn concurrent_stop_failure_and_timeout_complete_both_waiters_after_reap() {
    for (mode, expected) in [
        ("shutdown-error", ManagerErrorKind::Remote),
        ("shutdown-hang", ManagerErrorKind::Timeout),
    ] {
        let mut config = config(mode);
        if mode == "shutdown-hang" {
            config.shutdown_timeout = Duration::from_millis(75);
        }
        let manager = RuntimeManager::new(config).unwrap();
        let pid = manager.start_runtime().await.unwrap().pid.unwrap();
        let first_manager = manager.clone();
        let first = tokio::spawn(async move { first_manager.stop_runtime().await });
        wait_for_stderr(&manager, b"shutdown-received").await;
        let second_manager = manager.clone();
        let second = tokio::spawn(async move { second_manager.stop_runtime().await });

        let first_error = first.await.unwrap().unwrap_err();
        let second_error = second.await.unwrap().unwrap_err();
        assert_eq!(first_error, second_error, "mode {mode}");
        assert_eq!(first_error.kind, expected, "mode {mode}");
        assert_pid_gone(pid).await;
        let status = manager.get_runtime_status().await.unwrap();
        assert_eq!(status.pid, None, "mode {mode}");
        assert_eq!(occurrences(&status.stderr_tail, b"shutdown-received"), 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn concurrent_stop_waiters_have_a_command_queue_derived_bound() {
    let mut config = config("shutdown-delay");
    config.command_queue_capacity = 1;
    let manager = RuntimeManager::new(config).unwrap();
    manager.start_runtime().await.unwrap();
    let first_manager = manager.clone();
    let first = tokio::spawn(async move { first_manager.stop_runtime().await });
    wait_for_stderr(&manager, b"shutdown-received").await;

    let second_manager = manager.clone();
    let second = tokio::spawn(async move { second_manager.stop_runtime().await });
    tokio::time::sleep(Duration::from_millis(5)).await;
    let overflow = tokio::time::timeout(Duration::from_millis(50), manager.stop_runtime())
        .await
        .expect("overflow Stop must receive an immediate bounded error")
        .unwrap_err();
    assert_eq!(overflow.kind, ManagerErrorKind::Capacity);

    assert_eq!(
        first.await.unwrap().unwrap(),
        second.await.unwrap().unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn stdout_close_then_hang_cannot_override_short_handshake_deadline() {
    let mut config = config("stdout-close-hang");
    // Keep the handshake deadline well below the 2 s shutdown/reap bound while
    // leaving enough scheduler margin for a freshly provisioned CI runner.
    config.handshake_timeout = Duration::from_millis(500);
    config.shutdown_timeout = Duration::from_secs(2);
    let manager = RuntimeManager::new(config).unwrap();
    let (start, pid) = start_and_capture_pid(&manager).await;
    let began = Instant::now();

    let error = start.await.unwrap().unwrap_err();

    assert!(began.elapsed() < Duration::from_millis(1_000));
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn inherited_continuous_stdout_cannot_starve_absolute_drain_bound() {
    let mut config = config("drain-descendant");
    config.event_queue_capacity = 256;
    let manager = RuntimeManager::new(config).unwrap();
    let pid = manager.start_runtime().await.unwrap().pid.unwrap();
    let began = Instant::now();

    let error = tokio::time::timeout(Duration::from_millis(500), manager.ping(b"drain"))
        .await
        .expect("lifecycle reply exceeded the absolute pipe-drain bound")
        .unwrap_err();

    // The parent exits while descendants retain stdout. The kernel may report
    // the dead parent's stdin first (Process) or the poisoned protocol stream
    // first (Protocol); both must obey the same bounded reap barrier.
    assert!(matches!(
        error.kind,
        ManagerErrorKind::Process | ManagerErrorKind::Protocol
    ));
    let discard_work_cap = if cfg!(windows) {
        Duration::from_millis(450)
    } else {
        Duration::from_millis(90)
    };
    assert!(
        began.elapsed() < discard_work_cap,
        "continuous stdout exceeded the platform discard-work cap {discard_work_cap:?}: {:?}",
        began.elapsed()
    );
    assert_pid_gone(pid).await;
    assert!(
        manager
            .get_runtime_status()
            .await
            .unwrap()
            .stderr_tail
            .ends_with(b"drain-parent-final-stderr")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn blocked_stdin_writer_cannot_mask_earliest_pending_deadline() {
    let mut config = config("stdin-block");
    config.max_in_flight = 64;
    config.command_queue_capacity = 256;
    config.request_timeout = Duration::from_millis(150);
    let manager = RuntimeManager::new(config).unwrap();
    let pid = manager.start_runtime().await.unwrap().pid.unwrap();
    let first_manager = manager.clone();
    let began = Instant::now();
    let first = tokio::spawn(async move { first_manager.ping(&[b'a'; 256]).await });
    wait_for_stderr(&manager, b"first-request-held").await;

    let mut flood = Vec::new();
    for byte in 0_u8..63_u8 {
        let flood_manager = manager.clone();
        flood.push(tokio::spawn(async move {
            flood_manager.ping(&[byte; 256]).await
        }));
    }

    let error = tokio::time::timeout(Duration::from_secs(1), first)
        .await
        .expect("earliest pending deadline was masked by a blocked stdin write")
        .unwrap()
        .unwrap_err();

    assert_eq!(error.kind, ManagerErrorKind::Timeout);
    assert!(began.elapsed() < Duration::from_millis(500));
    assert_pid_gone(pid).await;
    for request in flood {
        let _ = request.await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn command_flood_cannot_starve_owned_child_exit_observation() {
    let mut config = config("exit-descendant");
    config.command_queue_capacity = 256;
    let manager = RuntimeManager::new(config).unwrap();
    let pid = manager.start_runtime().await.unwrap().pid.unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let mut flood = Vec::new();
    for _ in 0..256 {
        let flood_manager = manager.clone();
        let flood_running = Arc::clone(&running);
        flood.push(tokio::spawn(async move {
            while flood_running.load(Ordering::Relaxed) {
                let _ = flood_manager.get_runtime_status().await;
            }
        }));
    }

    let status = tokio::time::timeout(Duration::from_millis(750), async {
        loop {
            let status = manager.get_runtime_status().await.unwrap();
            if status.state != RuntimeState::Connected {
                return status;
            }
        }
    })
    .await
    .expect("command traffic starved owned-child exit observation");

    running.store(false, Ordering::Relaxed);
    for task in flood {
        task.await.unwrap();
    }
    assert_eq!(status.state, RuntimeState::Crashed);
    assert_eq!(status.pid, None);
    assert_pid_gone(pid).await;
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

fn occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
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
