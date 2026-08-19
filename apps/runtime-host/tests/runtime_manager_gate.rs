use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ai_voice_runtime_host::{
    CapabilityObservationKind, ManagerErrorKind, RuntimeExitReason, RuntimeManager,
    RuntimeManagerConfig, RuntimeState,
};

const CASE_DEADLINE: Duration = Duration::from_secs(10);
const POLL_DEADLINE: Duration = Duration::from_secs(2);

fn fixture_path() -> PathBuf {
    std::env::var_os("AIVS_RUNTIME_FIXTURE_PATH")
        .map(PathBuf::from)
        .expect("CTest must set AIVS_RUNTIME_FIXTURE_PATH")
}

fn real_paths() -> (PathBuf, PathBuf) {
    let runtime = std::env::var_os("AIVS_RUNTIME_PATH")
        .map(PathBuf::from)
        .expect("CTest must set AIVS_RUNTIME_PATH");
    let plugin = std::env::var_os("AIVS_MOCK_PLUGIN_PATH")
        .map(PathBuf::from)
        .expect("CTest must set AIVS_MOCK_PLUGIN_PATH");
    (runtime, plugin)
}

fn fixture_config(mode: &str) -> RuntimeManagerConfig {
    let mut config = RuntimeManagerConfig::new(fixture_path(), std::env::temp_dir().join(mode));
    config.handshake_timeout = Duration::from_millis(400);
    config.request_timeout = Duration::from_millis(400);
    config.shutdown_timeout = Duration::from_millis(400);
    config.stderr_tail_bytes = 127;
    config
}

async fn bounded<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(CASE_DEADLINE, future)
        .await
        .expect("process integration case exceeded its 10 second deadline")
}

struct ProcessGuard {
    pid: Option<u32>,
}

impl ProcessGuard {
    const fn new(pid: u32) -> Self {
        Self { pid: Some(pid) }
    }

    fn disarm(&mut self) {
        self.pid = None;
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.pid.take() {
            force_terminate(pid);
        }
    }
}

struct DirectoryGuard(PathBuf);

impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided native Runtime, Mock plugin, and controlled fixture"]
async fn real_runtime_uses_unicode_process_arguments_and_reaps_after_ordered_shutdown() {
    bounded(async {
        let (runtime, plugin) = real_paths();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "aivs-runtime-gate-声音-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let _directory_guard = DirectoryGuard(directory.clone());
        let runtime_copy = directory.join(unicode_file_name("voice-runtime-进程", &runtime));
        let plugin_copy = directory.join(unicode_file_name("mock-engine-引擎", &plugin));
        fs::copy(&runtime, &runtime_copy).unwrap();
        fs::copy(&plugin, &plugin_copy).unwrap();

        let mut config = RuntimeManagerConfig::new(runtime_copy, plugin_copy);
        config.log_directory = directory.join("日志-目录");
        let manager = RuntimeManager::new(config).unwrap();
        let started = manager.start_runtime().await.unwrap();
        let pid = started.pid.unwrap();
        let mut process_guard = ProcessGuard::new(pid);
        assert_eq!(started.state, RuntimeState::Connected);
        assert_eq!(started.hello.as_ref().unwrap().generation, 1);

        let capabilities = manager.get_capabilities().await.unwrap();
        assert_eq!(capabilities.generation(), started.generation);
        assert_eq!(capabilities.kind(), CapabilityObservationKind::Observed);
        assert_eq!(
            manager.ping("关联-✓".as_bytes()).await.unwrap(),
            "关联-✓".as_bytes()
        );

        let stopped = manager.stop_runtime().await.unwrap();
        assert_eq!(stopped.state, RuntimeState::Stopped);
        assert_eq!(stopped.pid, None);
        assert_eq!(
            stopped.last_exit.as_ref().unwrap().reason,
            RuntimeExitReason::RequestedShutdown
        );
        assert!(stopped.last_exit.as_ref().unwrap().success);
        assert_pid_gone(pid).await;
        process_guard.disarm();
        assert!(directory.join("日志-目录/voice-runtime.jsonl").is_file());
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn hostile_pre_hello_frames_fail_closed_only_after_reap() {
    bounded(async {
        for (mode, expected_kind) in [
            ("prehello-malformed-frame", ManagerErrorKind::Protocol),
            ("prehello-oversized-frame", ManagerErrorKind::Protocol),
            ("prehello-wrong-version", ManagerErrorKind::Protocol),
            ("prehello-malformed-payload", ManagerErrorKind::Payload),
        ] {
            let manager = RuntimeManager::new(fixture_config(mode)).unwrap();
            let (result, mut guard) = start_with_pid(&manager).await;
            let error = result.unwrap_err();
            assert_eq!(error.kind, expected_kind, "mode {mode}");
            assert_pid_gone(guard.pid.unwrap()).await;
            let status = manager.get_runtime_status().await.unwrap();
            assert_eq!(status.state, RuntimeState::Error, "mode {mode}");
            assert_eq!(status.pid, None, "mode {mode}");
            assert_eq!(
                status.last_exit.unwrap().reason,
                RuntimeExitReason::ProtocolFailure,
                "mode {mode}"
            );
            guard.disarm();
        }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn hostile_connected_frames_and_writer_failure_invalidate_after_reap() {
    bounded(async {
        for mode in [
            "response-unknown-id",
            "posthello-request",
            "posthello-hello",
            "stdin-close-hang",
        ] {
            let manager = RuntimeManager::new(fixture_config(mode)).unwrap();
            let started = manager.start_runtime().await.unwrap();
            let pid = started.pid.unwrap();
            let mut guard = ProcessGuard::new(pid);
            if mode == "stdin-close-hang" {
                wait_for_stderr(&manager, b"stdin-closed").await;
            }

            let error = manager.ping(b"correlate").await.unwrap_err();
            let expected = if mode == "stdin-close-hang" {
                ManagerErrorKind::Process
            } else {
                ManagerErrorKind::Protocol
            };
            assert_eq!(error.kind, expected, "mode {mode}");
            assert_pid_gone(pid).await;
            assert_eq!(manager.get_runtime_status().await.unwrap().pid, None);
            guard.disarm();
        }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn duplicate_response_and_malformed_payload_poison_the_stream_after_first_observation() {
    bounded(async {
        let duplicate = RuntimeManager::new(fixture_config("response-duplicate")).unwrap();
        let pid = duplicate.start_runtime().await.unwrap().pid.unwrap();
        let mut duplicate_guard = ProcessGuard::new(pid);
        assert_eq!(duplicate.ping(b"first").await.unwrap(), b"first");
        let failed = wait_for_non_connected(&duplicate).await;
        assert_eq!(failed.state, RuntimeState::Error);
        assert_eq!(
            failed.last_exit.unwrap().reason,
            RuntimeExitReason::ProtocolFailure
        );
        assert_pid_gone(pid).await;
        duplicate_guard.disarm();

        let malformed = RuntimeManager::new(fixture_config("malformed-capabilities")).unwrap();
        let pid = malformed.start_runtime().await.unwrap().pid.unwrap();
        let mut malformed_guard = ProcessGuard::new(pid);
        let observation = malformed.get_capabilities().await.unwrap();
        assert_eq!(observation.kind(), CapabilityObservationKind::Inconclusive);
        assert_eq!(observation.error().unwrap().kind, ManagerErrorKind::Payload);
        assert_pid_gone(pid).await;
        assert_eq!(malformed.get_runtime_status().await.unwrap().pid, None);
        malformed_guard.disarm();
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn stderr_flood_is_drained_without_blocking_and_retains_only_the_bounded_tail() {
    bounded(async {
        let manager = RuntimeManager::new(fixture_config("stderr-flood")).unwrap();
        let started = manager.start_runtime().await.unwrap();
        let pid = started.pid.unwrap();
        let mut guard = ProcessGuard::new(pid);
        wait_for_stderr(&manager, b"stderr-flood-complete").await;
        assert_eq!(manager.ping(b"alive").await.unwrap(), b"alive");
        let status = manager.get_runtime_status().await.unwrap();
        assert_eq!(status.stderr_tail.len(), 127);
        assert!(status.stderr_tail.ends_with(b"stderr-flood-complete\n"));
        manager.stop_runtime().await.unwrap();
        assert_pid_gone(pid).await;
        guard.disarm();
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn native_nonzero_exit_and_stale_generation_events_are_mapped_without_crossing_restart() {
    bounded(async {
        let exiting = RuntimeManager::new(fixture_config("exit-nonzero")).unwrap();
        let started = exiting.start_runtime().await.unwrap();
        let pid = started.pid.unwrap();
        let mut exit_guard = ProcessGuard::new(pid);
        let crashed = wait_for_non_connected(&exiting).await;
        assert_eq!(crashed.state, RuntimeState::Crashed);
        assert_eq!(crashed.last_exit.as_ref().unwrap().code, Some(7));
        assert!(!crashed.last_exit.as_ref().unwrap().success);
        assert_pid_gone(pid).await;
        exit_guard.disarm();

        let stale = RuntimeManager::new(fixture_config("stale-responses")).unwrap();
        let first = stale.start_runtime().await.unwrap();
        let pid = first.pid.unwrap();
        let mut first_guard = ProcessGuard::new(pid);
        let error = stale.ping(b"generation-one").await.unwrap_err();
        assert_eq!(error.kind, ManagerErrorKind::Protocol);
        assert_pid_gone(pid).await;
        first_guard.disarm();

        let second = stale.start_runtime().await.unwrap();
        assert_eq!(second.generation, first.generation + 1);
        let pid = second.pid.unwrap();
        let mut second_guard = ProcessGuard::new(pid);
        assert_eq!(
            stale.ping(b"generation-two").await.unwrap(),
            b"generation-two"
        );
        assert_eq!(
            stale.get_capabilities().await.unwrap().generation(),
            second.generation
        );
        stale.stop_runtime().await.unwrap();
        assert_pid_gone(pid).await;
        second_guard.disarm();
    })
    .await;
}

#[test]
#[ignore = "requires CTest-provided controlled child fixture"]
fn aborting_the_owning_tokio_runtime_reaps_the_child() {
    let fixture = fixture_path();
    let (pid_sender, pid_receiver) = std_mpsc::sync_channel(1);
    let (continue_sender, continue_receiver) = std_mpsc::sync_channel(1);
    let thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let manager = runtime.block_on(async {
            let manager = RuntimeManager::new(RuntimeManagerConfig::new(
                fixture,
                std::env::temp_dir().join("shutdown-hang"),
            ))
            .unwrap();
            let pid = manager.start_runtime().await.unwrap().pid.unwrap();
            pid_sender.send(pid).unwrap();
            manager
        });
        continue_receiver
            .recv_timeout(CASE_DEADLINE)
            .expect("test owner did not install the cleanup guard");
        drop(runtime);
        drop(manager);
    });
    let pid = pid_receiver
        .recv_timeout(CASE_DEADLINE)
        .expect("fixture did not publish its PID before the deadline");
    let mut guard = ProcessGuard::new(pid);
    continue_sender.send(()).unwrap();
    thread.join().unwrap();
    assert_pid_gone_blocking(pid);
    guard.disarm();
}

async fn start_with_pid(
    manager: &RuntimeManager,
) -> (
    Result<ai_voice_runtime_host::RuntimeStatus, ai_voice_runtime_host::ManagerError>,
    ProcessGuard,
) {
    let mut start = Box::pin(manager.start_runtime());
    let deadline = tokio::time::Instant::now() + POLL_DEADLINE;
    loop {
        tokio::select! {
            result = &mut start => panic!("start completed before publishing a PID: {result:?}"),
            _ = tokio::time::sleep(Duration::from_millis(1)) => {
                let status = manager.get_runtime_status().await.unwrap();
                if let Some(pid) = status.pid {
                    let guard = ProcessGuard::new(pid);
                    return (start.await, guard);
                }
            }
            _ = tokio::time::sleep_until(deadline) => panic!("start did not publish a PID"),
        }
    }
}

async fn wait_for_non_connected(manager: &RuntimeManager) -> ai_voice_runtime_host::RuntimeStatus {
    let deadline = tokio::time::Instant::now() + POLL_DEADLINE;
    loop {
        let status = manager.get_runtime_status().await.unwrap();
        if !matches!(
            status.state,
            RuntimeState::Starting | RuntimeState::Connected
        ) {
            return status;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "manager stayed connected"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

async fn wait_for_stderr(manager: &RuntimeManager, expected: &[u8]) {
    let deadline = tokio::time::Instant::now() + POLL_DEADLINE;
    loop {
        let tail = manager.get_runtime_status().await.unwrap().stderr_tail;
        if tail
            .windows(expected.len())
            .any(|window| window == expected)
        {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "stderr did not contain {:?}; tail was {:?}",
            String::from_utf8_lossy(expected),
            String::from_utf8_lossy(&tail)
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

fn unicode_file_name(stem: &str, source: &Path) -> String {
    let extension = source.extension().and_then(|value| value.to_str());
    match extension {
        Some(extension) => format!("{stem}.{extension}"),
        None => stem.to_owned(),
    }
}

async fn assert_pid_gone(pid: u32) {
    let deadline = tokio::time::Instant::now() + POLL_DEADLINE;
    while process_exists(pid).await {
        assert!(
            tokio::time::Instant::now() < deadline,
            "PID {pid} remained alive"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn assert_pid_gone_blocking(pid: u32) {
    let deadline = std::time::Instant::now() + POLL_DEADLINE;
    while process_exists_blocking(pid) {
        assert!(
            std::time::Instant::now() < deadline,
            "PID {pid} remained alive"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
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
        .unwrap()
        .success()
}

#[cfg(windows)]
async fn process_exists(pid: u32) -> bool {
    let output = tokio::process::Command::new("tasklist.exe")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .await
        .unwrap();
    String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
}

#[cfg(unix)]
fn process_exists_blocking(pid: u32) -> bool {
    std::process::Command::new("/bin/kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(windows)]
fn process_exists_blocking(pid: u32) -> bool {
    std::process::Command::new("tasklist.exe")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
}

#[cfg(unix)]
fn force_terminate(pid: u32) {
    let _ = std::process::Command::new("/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let deadline = std::time::Instant::now() + POLL_DEADLINE;
    while process_exists_blocking(pid) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(windows)]
fn force_terminate(pid: u32) {
    let _ = std::process::Command::new("taskkill.exe")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let deadline = std::time::Instant::now() + POLL_DEADLINE;
    while process_exists_blocking(pid) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
}
