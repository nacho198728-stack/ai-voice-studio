use std::sync::{Arc, Mutex};

use ai_voice_desktop::{
    CapabilityDto, CommandService, ControlPlane, ControlPlaneFuture, DesktopError,
    MockPipelineSummaryDto, RuntimeStatusDto, create_main_window, with_desktop_commands,
};
use tauri::ipc::InvokeBody;
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::utils::config::WindowConfig;
use tauri::webview::InvokeRequest;

#[derive(Clone)]
struct LifecycleFake {
    state: Arc<Mutex<&'static str>>,
}

impl LifecycleFake {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new("stopped")),
        }
    }
}

impl ControlPlane for LifecycleFake {
    fn start_runtime(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto> {
        let mut state = self.state.lock().unwrap();
        if *state != "stopped" {
            return Box::pin(async {
                Err(DesktopError {
                    code: "invalid_state".to_owned(),
                    message: "Refresh status and retry.".to_owned(),
                })
            });
        }
        *state = "connected";
        Box::pin(async { Ok(status("connected")) })
    }

    fn get_runtime_status(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto> {
        let state = *self.state.lock().unwrap();
        Box::pin(async move { Ok(status(state)) })
    }

    fn get_runtime_capabilities(&self) -> ControlPlaneFuture<'_, CapabilityDto> {
        Box::pin(async {
            Ok(CapabilityDto {
                schema_version: 1,
                platform: "macos".to_owned(),
                architecture: "arm64".to_owned(),
                runtime_version: "0.0.0".to_owned(),
                protocol_version: 1,
                backend: "mock".to_owned(),
                runtime_availability: "available".to_owned(),
                engine_identity: Some("aivs-mock-v1".to_owned()),
                engine_availability: "available".to_owned(),
                generation: 1,
            })
        })
    }

    fn run_mock_pipeline(&self) -> ControlPlaneFuture<'_, MockPipelineSummaryDto> {
        Box::pin(async {
            Ok(MockPipelineSummaryDto {
                input_frames: 128,
                output_frames: 128,
                checksum: "3ecd5190f6f4f725".to_owned(),
                elapsed_microseconds: 12,
                process_call_count: 1,
                process_error_count: 0,
                stream_generation: 1,
            })
        })
    }

    fn stop_runtime(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto> {
        *self.state.lock().unwrap() = "stopped";
        Box::pin(async { Ok(status("stopped")) })
    }
}

#[test]
fn tauri_invoke_handler_exposes_exactly_five_typed_commands_and_serializes_errors() {
    let service = CommandService::new(LifecycleFake::new());
    let checked_in_config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert_eq!(
        checked_in_config["build"]["devUrl"],
        "http://127.0.0.1:1420"
    );
    assert_eq!(checked_in_config["app"]["windows"][0]["create"], false);
    let mut context = mock_context(noop_assets());
    context.config_mut().app.windows.push(WindowConfig {
        label: "main".to_owned(),
        create: false,
        ..Default::default()
    });
    let app = with_desktop_commands(mock_builder(), service)
        .build(context)
        .unwrap();
    assert!(!app.config().app.windows[0].create);
    let webview = create_main_window(&app).unwrap();

    let start = invoke(&webview, "start_runtime").unwrap();
    assert_eq!(start["state"], "connected");
    let repeated = invoke(&webview, "start_runtime").unwrap_err();
    assert_eq!(repeated["code"], "invalid_state");
    assert_eq!(
        invoke(&webview, "get_runtime_status").unwrap()["state"],
        "connected"
    );
    assert_eq!(
        invoke(&webview, "get_runtime_capabilities").unwrap()["engineIdentity"],
        "aivs-mock-v1"
    );
    let summary = invoke(&webview, "run_mock_pipeline").unwrap();
    assert_eq!(summary["checksum"], "3ecd5190f6f4f725");
    assert!(summary.get("pcm").is_none());
    assert!(summary.get("samples").is_none());
    assert_eq!(
        invoke(&webview, "stop_runtime").unwrap()["state"],
        "stopped"
    );

    let unexpected = invoke_body(
        &webview,
        "get_runtime_status",
        serde_json::json!({ "path": "/private/host" }).into(),
    )
    .unwrap_err();
    assert_eq!(unexpected["code"], "invalid_payload");

    assert!(invoke(&webview, "shell_execute").is_err());
    assert!(invoke(&webview, "read_file").is_err());
}

fn invoke(
    webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    command: &str,
) -> Result<serde_json::Value, serde_json::Value> {
    invoke_body(webview, command, InvokeBody::default())
}

fn invoke_body(
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

fn status(state: &str) -> RuntimeStatusDto {
    RuntimeStatusDto {
        product_name: "AI Voice Studio".to_owned(),
        product_version: "0.0.0-dev".to_owned(),
        runtime_version: "0.0.0".to_owned(),
        state: state.to_owned(),
        generation: u64::from(state != "stopped"),
        detail: format!("Runtime is {state}."),
    }
}
