use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ai_voice_desktop::{
    ArtifactLayout, DesktopApplication, create_main_window, current_target_triple,
    with_desktop_commands,
};
use tauri::ipc::InvokeBody;
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::utils::config::WindowConfig;
use tauri::webview::InvokeRequest;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires tools/scripts/stage-desktop-native.mjs output"]
async fn staged_tauri_invoke_runs_the_real_runtime_c_abi_mock_chain_and_reaps() {
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
    let mut context = mock_context(noop_assets());
    context.config_mut().app.windows.push(WindowConfig {
        label: "main".to_owned(),
        create: false,
        ..Default::default()
    });
    let app = with_desktop_commands(mock_builder(), desktop.service())
        .build(context)
        .unwrap();
    let webview = create_main_window(&app).unwrap();

    let initial = invoke(&webview, "get_runtime_status", InvokeBody::default()).unwrap();
    assert_eq!(initial["state"], "stopped");
    assert!(initial.get("pid").is_none());
    assert!(initial.get("stderr").is_none());
    let invalid = invoke(
        &webview,
        "get_runtime_status",
        serde_json::json!({ "unexpected": true }).into(),
    )
    .unwrap_err();
    assert_eq!(invalid["code"], "invalid_payload");

    let connected = invoke(&webview, "start_runtime", InvokeBody::default()).unwrap();
    assert_eq!(connected["state"], "connected");
    assert_eq!(connected["generation"], 1);
    assert_eq!(
        invoke(&webview, "get_runtime_status", InvokeBody::default()).unwrap()["state"],
        "connected"
    );

    let capabilities = invoke(&webview, "get_runtime_capabilities", InvokeBody::default()).unwrap();
    assert_eq!(capabilities["backend"], "mock");
    assert_eq!(capabilities["engineIdentity"], "aivs-mock-v1");
    let summary = invoke(&webview, "run_mock_pipeline", InvokeBody::default()).unwrap();
    assert_eq!(summary["inputFrames"], 128);
    assert_eq!(summary["outputFrames"], 128);
    assert_eq!(summary["checksum"], "3ecd5190f6f4f725");
    assert_eq!(summary["processCallCount"], 1);
    assert_eq!(summary["processErrorCount"], 0);
    assert!(summary.get("pcm").is_none());
    assert!(summary.get("samples").is_none());
    assert!(serde_json::to_vec(&summary).unwrap().len() < 1_024);

    assert_eq!(
        invoke(&webview, "stop_runtime", InvokeBody::default()).unwrap()["state"],
        "stopped"
    );
    assert_eq!(
        invoke(&webview, "get_runtime_status", InvokeBody::default()).unwrap()["state"],
        "stopped"
    );
    drop(webview);
    drop(app);
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

fn invoke(
    webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    command: &str,
    body: InvokeBody,
) -> Result<serde_json::Value, serde_json::Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: command.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body,
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_owned(),
        },
    )
    .map(|body| body.deserialize::<serde_json::Value>().unwrap())
}
