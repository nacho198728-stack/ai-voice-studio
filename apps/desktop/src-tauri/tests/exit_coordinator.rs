use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use ai_voice_desktop::ExitCoordinator;

#[tokio::test]
async fn exit_request_returns_immediately_runs_one_slow_cleanup_and_then_allows_final_exit() {
    let coordinator = ExitCoordinator::new();
    let starts = Arc::new(AtomicUsize::new(0));
    let exits = Arc::new(AtomicUsize::new(0));
    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();

    let started_at = Instant::now();
    let first = coordinator.request_exit(
        Duration::from_secs(1),
        {
            let starts = Arc::clone(&starts);
            move || async move {
                starts.fetch_add(1, Ordering::SeqCst);
                let _ = release_rx.await;
            }
        },
        {
            let exits = Arc::clone(&exits);
            move || {
                exits.fetch_add(1, Ordering::SeqCst);
            }
        },
    );
    assert!(first.should_prevent());
    assert!(started_at.elapsed() < Duration::from_millis(50));
    let cleanup = first.into_cleanup().expect("first request starts cleanup");
    let cleanup_task = tokio::spawn(cleanup);
    tokio::task::yield_now().await;
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(exits.load(Ordering::SeqCst), 0);

    let repeated = coordinator.request_exit(
        Duration::from_secs(1),
        || async { panic!("duplicate request must not start cleanup") },
        || panic!("duplicate request must not exit early"),
    );
    assert!(repeated.should_prevent());
    assert!(repeated.into_cleanup().is_none());

    release_tx.send(()).unwrap();
    cleanup_task.await.unwrap();
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(exits.load(Ordering::SeqCst), 1);

    let final_request = coordinator.request_exit(
        Duration::from_secs(1),
        || async { panic!("final request must not clean up again") },
        || panic!("final request is already authorized"),
    );
    assert!(!final_request.should_prevent());
    assert!(final_request.into_cleanup().is_none());
}

#[tokio::test]
async fn cleanup_timeout_still_authorizes_and_requests_final_exit() {
    let coordinator = ExitCoordinator::new();
    let exits = Arc::new(AtomicUsize::new(0));
    let disposition =
        coordinator.request_exit(Duration::from_millis(10), std::future::pending::<()>, {
            let exits = Arc::clone(&exits);
            move || {
                exits.fetch_add(1, Ordering::SeqCst);
            }
        });

    disposition.into_cleanup().unwrap().await;
    assert_eq!(exits.load(Ordering::SeqCst), 1);
    assert!(
        !coordinator
            .request_exit(Duration::from_secs(1), || async {}, || {})
            .should_prevent()
    );
}

#[tokio::test]
async fn cleanup_failure_still_authorizes_and_requests_final_exit() {
    let coordinator = ExitCoordinator::new();
    let exits = Arc::new(AtomicUsize::new(0));
    let disposition = coordinator.request_exit(
        Duration::from_secs(1),
        || async { Err::<(), _>("bounded cleanup failed") },
        {
            let exits = Arc::clone(&exits);
            move || {
                exits.fetch_add(1, Ordering::SeqCst);
            }
        },
    );

    disposition.into_cleanup().unwrap().await;
    assert_eq!(exits.load(Ordering::SeqCst), 1);
    assert!(
        !coordinator
            .request_exit(Duration::from_secs(1), || async {}, || {})
            .should_prevent()
    );
}
