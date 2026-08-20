use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ai_voice_config::ProductConfig;
use ai_voice_contracts::ErrorCode;
use ai_voice_desktop::{
    ArtifactLayout, CapabilityDto, CommandService, ControlPlane, ControlPlaneFuture, DesktopError,
    MockPipelineSummaryDto, NativeArtifactPaths, NavigationPolicy, RuntimeStatusDto,
    build_manager_config,
};
use ai_voice_runtime_host::{
    ManagerError, ManagerErrorKind, RestartPolicyStatus, RuntimeState, RuntimeStatus,
};

#[derive(Clone, Default)]
struct FakeControlPlane {
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl ControlPlane for FakeControlPlane {
    fn start_runtime(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto> {
        self.record("start_runtime");
        Box::pin(async { Ok(status("connected", 1)) })
    }

    fn get_runtime_status(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto> {
        self.record("get_runtime_status");
        Box::pin(async { Ok(status("stopped", 0)) })
    }

    fn get_runtime_capabilities(&self) -> ControlPlaneFuture<'_, CapabilityDto> {
        self.record("get_runtime_capabilities");
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
        self.record("run_mock_pipeline");
        Box::pin(async {
            Ok(MockPipelineSummaryDto {
                input_frames: 128,
                output_frames: 128,
                checksum: "3ecd5190f6f4f725".to_owned(),
                elapsed_microseconds: 17,
                process_call_count: 1,
                process_error_count: 0,
                stream_generation: 1,
            })
        })
    }

    fn stop_runtime(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto> {
        self.record("stop_runtime");
        Box::pin(async { Ok(status("stopped", 1)) })
    }
}

impl FakeControlPlane {
    fn record(&self, name: &'static str) {
        self.calls.lock().unwrap().push(name);
    }
}

#[tokio::test]
async fn command_service_exposes_only_five_bounded_control_operations() {
    let fake = FakeControlPlane::default();
    let service = CommandService::new(fake.clone());

    assert_eq!(service.start_runtime().await.unwrap().state, "connected");
    assert_eq!(service.get_runtime_status().await.unwrap().state, "stopped");
    let capability = service.get_runtime_capabilities().await.unwrap();
    assert_eq!(capability.backend, "mock");
    assert_eq!(capability.engine_identity.as_deref(), Some("aivs-mock-v1"));
    let summary = service.run_mock_pipeline().await.unwrap();
    assert_eq!(summary.input_frames, 128);
    assert_eq!(summary.output_frames, 128);
    assert_eq!(summary.checksum, "3ecd5190f6f4f725");
    assert_eq!(service.stop_runtime().await.unwrap().state, "stopped");
    assert_eq!(
        *fake.calls.lock().unwrap(),
        [
            "start_runtime",
            "get_runtime_status",
            "get_runtime_capabilities",
            "run_mock_pipeline",
            "stop_runtime",
        ]
    );

    let json = serde_json::to_value(summary).unwrap();
    let keys: Vec<_> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "checksum",
            "elapsedMicroseconds",
            "inputFrames",
            "outputFrames",
            "processCallCount",
            "processErrorCount",
            "streamGeneration",
        ]
    );
    assert!(!json.to_string().to_ascii_lowercase().contains("pcm"));
    assert!(!json.to_string().to_ascii_lowercase().contains("sample"));
}

#[test]
fn status_and_errors_are_stable_bounded_and_redacted() {
    let native = RuntimeStatus {
        state: RuntimeState::Crashed,
        generation: 4,
        pid: None,
        hello: None,
        last_exit: None,
        last_error: None,
        stderr_tail: b"private child stderr".to_vec(),
        restart_policy: RestartPolicyStatus {
            automatic_restart: false,
            max_attempts: 0,
            attempts_observed: 0,
        },
    };
    let mapped = RuntimeStatusDto::from_runtime_status(&native);
    assert_eq!(mapped.state, "crashed");
    assert_eq!(mapped.product_name, "AI Voice Studio");
    assert!(!serde_json::to_string(&mapped).unwrap().contains("stderr"));
    assert!(!serde_json::to_string(&mapped).unwrap().contains("pid"));

    let internal = ManagerError {
        code: ErrorCode::RuntimeUnavailable,
        kind: ManagerErrorKind::Process,
        message: format!(
            "spawn failed at /Users/tester/secret/runtime: {}",
            "x".repeat(2_000)
        ),
    };
    let error = DesktopError::from_manager(&internal);
    assert_eq!(error.code, "runtime_unavailable");
    assert!(error.message.len() <= 240);
    assert!(!error.message.contains("/Users"));
    assert!(!error.message.contains("secret"));
}

fn status(state: &str, generation: u64) -> RuntimeStatusDto {
    RuntimeStatusDto {
        product_name: "AI Voice Studio".to_owned(),
        product_version: "0.0.0-dev".to_owned(),
        runtime_version: "0.0.0".to_owned(),
        state: state.to_owned(),
        generation,
        detail: format!("Runtime is {state}."),
    }
}

#[test]
fn artifact_layout_is_deterministic_for_unicode_macos_and_windows_paths() {
    let mac = ArtifactLayout::new("aarch64-apple-darwin").unwrap();
    let development = mac.development_paths(Path::new("/tmp/AI 声音 🎵/src-tauri"));
    assert_eq!(
        development.runtime,
        PathBuf::from("/tmp/AI 声音 🎵/src-tauri/binaries/voice-runtime-aarch64-apple-darwin")
    );
    assert_eq!(
        development.plugin,
        PathBuf::from(
            "/tmp/AI 声音 🎵/src-tauri/resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib"
        )
    );

    let windows = ArtifactLayout::new("x86_64-pc-windows-msvc").unwrap();
    let bundled = windows.bundled_paths(
        Path::new(r"C:\Program Files\AI 声音\ai-voice-studio.exe"),
        Path::new(r"C:\Program Files\AI 声音\resources"),
    );
    assert_eq!(
        bundled,
        NativeArtifactPaths {
            runtime: PathBuf::from(r"C:\Program Files\AI 声音\voice-runtime.exe"),
            plugin: PathBuf::from(
                r"C:\Program Files\AI 声音\resources\native\aivs_mock_voice_engine-x86_64-pc-windows-msvc.dll"
            ),
            config: PathBuf::from(r"C:\Program Files\AI 声音\resources\config\config.json"),
        }
    );
}

#[test]
fn navigation_policy_allows_only_the_exact_packaged_or_configured_development_origin() {
    let macos = NavigationPolicy::production(false);
    assert!(macos.allows(&"tauri://localhost/".parse().unwrap()));
    assert!(macos.allows(&"tauri://localhost/status?fresh=1#runtime".parse().unwrap()));
    assert!(!macos.allows(&"http://tauri.localhost/".parse().unwrap()));

    let windows = NavigationPolicy::production(true);
    assert!(windows.allows(&"http://tauri.localhost/".parse().unwrap()));
    assert!(!windows.allows(&"https://tauri.localhost/".parse().unwrap()));
    assert!(!windows.allows(&"http://tauri.localhost.evil.example/".parse().unwrap()));

    let development = NavigationPolicy::development();
    assert!(development.allows(&"http://127.0.0.1:1420/".parse().unwrap()));
    assert!(development.allows(&"http://127.0.0.1:1420/src/main.ts".parse().unwrap()));
    for denied in [
        "http://localhost:1420/",
        "http://127.0.0.1:1421/",
        "http://127.0.0.1.evil.example:1420/",
        "http://user@127.0.0.1:1420/",
        "https://example.com/",
        "file:///tmp/index.html",
        "data:text/html,hostile",
        "javascript:alert(1)",
    ] {
        assert!(!development.allows(&denied.parse().unwrap()), "{denied}");
    }

    assert!(
        NavigationPolicy::for_mode(true, false).allows(&"http://127.0.0.1:1420/".parse().unwrap())
    );
    assert!(
        NavigationPolicy::for_mode(false, false).allows(&"tauri://localhost/".parse().unwrap())
    );
    assert!(
        NavigationPolicy::for_mode(false, true).allows(&"http://tauri.localhost/".parse().unwrap())
    );
}

#[test]
fn invalid_target_triples_and_missing_artifacts_return_actionable_public_errors() {
    assert!(ArtifactLayout::new("../../host").is_err());
    let missing = NativeArtifactPaths {
        runtime: PathBuf::from("/missing/voice-runtime"),
        plugin: PathBuf::from("/missing/mock-plugin"),
        config: PathBuf::from("/missing/config.json"),
    };
    let error = missing.validate_development().unwrap_err();
    assert_eq!(error.code, "native_artifacts_missing");
    assert!(error.message.contains("pnpm desktop:native:prepare"));
    assert!(!error.message.contains("/missing"));

    let bundled = missing.validate_bundled().unwrap_err();
    assert_eq!(bundled.code, "native_artifacts_missing");
    assert!(bundled.message.contains("Reinstall"));
    assert!(!bundled.message.contains("pnpm"));
    assert!(!bundled.message.contains("/missing"));
}

#[test]
fn manager_config_is_derived_from_validated_product_and_resolved_artifacts() {
    let product =
        ProductConfig::load_from_bytes(include_bytes!("../../../../config/config.json")).unwrap();
    let root = std::env::temp_dir().join("AI Voice Studio 配置测试");
    let paths = NativeArtifactPaths {
        runtime: root.join("runtime/voice-runtime"),
        plugin: root.join("native/aivs_mock_voice_engine"),
        config: root.join("config/config.json"),
    };
    let app_data = root.join("app-data");

    assert!(paths.runtime.is_absolute());
    assert!(app_data.is_absolute());

    let config = build_manager_config(&product, &paths, &app_data).unwrap();

    assert_eq!(config.runtime_path, paths.runtime);
    assert_eq!(config.plugin_path, paths.plugin);
    assert_eq!(config.log_directory, app_data.join("logs/development"));
    assert_eq!(config.request_timeout, std::time::Duration::from_secs(2));
    assert_eq!(config.max_in_flight, 16);
}
