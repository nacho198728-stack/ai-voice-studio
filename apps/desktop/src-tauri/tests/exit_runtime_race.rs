use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ai_voice_config::ProductConfig;
use ai_voice_desktop::{CommandService, ExitCoordinator, RuntimeControlPlane};
use ai_voice_runtime_host::{RuntimeManager, RuntimeManagerConfig, RuntimeState};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CTest-provided controlled child fixture"]
async fn exit_cleanup_joins_a_pending_desktop_stop_until_the_same_reap_completes() {
    let fixture = PathBuf::from(
        std::env::var_os("AIVS_RUNTIME_FIXTURE_PATH")
            .expect("CTest must set AIVS_RUNTIME_FIXTURE_PATH"),
    );
    let mut config =
        RuntimeManagerConfig::new(fixture, std::env::temp_dir().join("shutdown-delay"));
    config.handshake_timeout = Duration::from_millis(500);
    config.request_timeout = Duration::from_millis(500);
    config.shutdown_timeout = Duration::from_millis(500);
    let manager = RuntimeManager::new(config).unwrap();
    let product =
        ProductConfig::load_from_bytes(include_bytes!("../../../../config/config.json")).unwrap();
    let service = CommandService::new(RuntimeControlPlane::new(manager.clone(), product));
    let pid = manager.start_runtime().await.unwrap().pid.unwrap();

    let stop_service = service.clone();
    let stop = tokio::spawn(async move { stop_service.stop_runtime().await });
    wait_until_stopping_with_shutdown(&manager).await;

    let exits = Arc::new(AtomicUsize::new(0));
    let coordinator = ExitCoordinator::new();
    let cleanup_service = service.clone();
    let disposition = coordinator.request_exit(
        Duration::from_secs(3),
        move || async move { cleanup_service.stop_runtime().await },
        {
            let exits = Arc::clone(&exits);
            move || {
                exits.fetch_add(1, Ordering::SeqCst);
            }
        },
    );
    let cleanup = tokio::spawn(disposition.into_cleanup().unwrap());
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!stop.is_finished(), "UI Stop completed before process reap");
    assert!(
        !cleanup.is_finished(),
        "exit cleanup did not join the pending Stop"
    );
    assert_eq!(exits.load(Ordering::SeqCst), 0);

    assert_eq!(stop.await.unwrap().unwrap().state, "stopped");
    cleanup.await.unwrap();
    assert_eq!(exits.load(Ordering::SeqCst), 1);
    assert!(!process_exists(pid).await);
    let status = manager.get_runtime_status().await.unwrap();
    assert_eq!(status.state, RuntimeState::Stopped);
    assert_eq!(status.pid, None);
    assert_eq!(occurrences(&status.stderr_tail, b"shutdown-received"), 1);
}

async fn wait_until_stopping_with_shutdown(manager: &RuntimeManager) {
    for _ in 0..200 {
        let status = manager.get_runtime_status().await.unwrap();
        if status.state == RuntimeState::Stopping
            && status
                .stderr_tail
                .windows(b"shutdown-received".len())
                .any(|window| window == b"shutdown-received")
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    panic!("fixture did not enter Stopping after receiving Shutdown");
}

fn occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
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
