use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use ai_voice_capability::{
    Architecture, CapabilityAvailability, EngineIdentity, Platform, RuntimeBackend,
};
use ai_voice_config::ProductConfig;
use ai_voice_contracts::{ErrorCode, RUNTIME_VERSION};
use ai_voice_runtime_host::{
    CapabilityProfile, ManagerError, MockPipelineSummary, RuntimeManager, RuntimeManagerConfig,
    RuntimeState, RuntimeStatus,
};
use ai_voice_telemetry::{TelemetryConfig, TelemetryGuard, initialize as initialize_telemetry};
use serde::Serialize;
use tauri::Manager;
use tauri::ipc::{InvokeBody, Request};

pub const PRODUCT_NAME: &str = "AI Voice Studio";
pub const PRODUCT_VERSION: &str = "0.0.0-dev";
const MAX_PUBLIC_ERROR_BYTES: usize = 240;
const PREPARE_COMMAND: &str = "pnpm desktop:native:prepare";
const EXIT_CLEANUP_TIMEOUT: Duration = Duration::from_secs(3);
const EXIT_READY: u8 = 0;
const EXIT_CLEANING: u8 = 1;
const EXIT_FINAL: u8 = 2;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatusDto {
    pub product_name: String,
    pub product_version: String,
    pub runtime_version: String,
    pub state: String,
    pub generation: u64,
    pub detail: String,
}

impl RuntimeStatusDto {
    pub fn from_runtime_status(status: &RuntimeStatus) -> Self {
        let (state, detail) = match status.state {
            RuntimeState::Stopped => ("stopped", "Runtime is stopped."),
            RuntimeState::Starting => ("starting", "Runtime is starting."),
            RuntimeState::Connected => ("connected", "Runtime is connected."),
            RuntimeState::Stopping => ("stopping", "Runtime is stopping."),
            RuntimeState::Crashed => (
                "crashed",
                "Runtime exited unexpectedly. Start it again after checking local logs.",
            ),
            RuntimeState::Error => (
                "error",
                "Runtime encountered an error. Retry the action or check local logs.",
            ),
        };
        Self {
            product_name: PRODUCT_NAME.to_owned(),
            product_version: PRODUCT_VERSION.to_owned(),
            runtime_version: RUNTIME_VERSION.to_owned(),
            state: state.to_owned(),
            generation: status.generation,
            detail: detail.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDto {
    pub schema_version: u32,
    pub platform: String,
    pub architecture: String,
    pub runtime_version: String,
    pub protocol_version: u32,
    pub backend: String,
    pub runtime_availability: String,
    pub engine_identity: Option<String>,
    pub engine_availability: String,
    pub generation: u64,
}

impl CapabilityDto {
    fn from_profile(profile: &CapabilityProfile) -> Self {
        Self {
            schema_version: profile.schema_version(),
            platform: platform_name(profile.platform()).to_owned(),
            architecture: architecture_name(profile.architecture()).to_owned(),
            runtime_version: profile.runtime().version().to_owned(),
            protocol_version: profile.runtime().protocol_version(),
            backend: backend_name(profile.runtime().backend()).to_owned(),
            runtime_availability: availability_name(profile.runtime().availability()).to_owned(),
            engine_identity: profile
                .engine()
                .identity()
                .map(engine_name)
                .map(str::to_owned),
            engine_availability: availability_name(profile.engine().availability()).to_owned(),
            generation: profile.manager().generation(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockPipelineSummaryDto {
    pub input_frames: u64,
    pub output_frames: u64,
    pub checksum: String,
    pub elapsed_microseconds: u64,
    pub process_call_count: u64,
    pub process_error_count: u64,
    pub stream_generation: u64,
}

impl From<MockPipelineSummary> for MockPipelineSummaryDto {
    fn from(summary: MockPipelineSummary) -> Self {
        Self {
            input_frames: summary.input_frame_count,
            output_frames: summary.output_frame_count,
            checksum: format!("{:016x}", summary.checksum),
            elapsed_microseconds: summary.elapsed_microseconds,
            process_call_count: summary.process_call_count,
            process_error_count: summary.process_error_count,
            stream_generation: summary.stream_generation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DesktopError {
    pub code: String,
    pub message: String,
}

impl DesktopError {
    pub fn from_manager(error: &ManagerError) -> Self {
        let code = error_code_name(error.code);
        let message = match error.code {
            ErrorCode::InvalidState | ErrorCode::RuntimeShuttingDown => {
                "This action is unavailable in the current Runtime state. Refresh status and retry."
            }
            ErrorCode::RuntimeUnavailable => {
                "Runtime is unavailable. Build the native artifacts if needed, then retry."
            }
            ErrorCode::EngineUnavailable => {
                "The Mock Engine is unavailable. Prepare the native artifacts and retry."
            }
            ErrorCode::UnsupportedProtocolVersion | ErrorCode::UnsupportedVoiceEngineAbi => {
                "Native artifact versions are incompatible. Rebuild all native artifacts."
            }
            ErrorCode::MalformedFrame | ErrorCode::FrameTooLarge => {
                "The Runtime returned an invalid response. Restart it and retry."
            }
            ErrorCode::InvalidArgument | ErrorCode::BufferTooSmall => {
                "The desktop request was rejected by the bounded native contract."
            }
            ErrorCode::InternalError | ErrorCode::Success => {
                "The desktop control plane encountered an internal error."
            }
        };
        Self::public(code, message)
    }

    fn public(code: &str, message: &str) -> Self {
        Self {
            code: code.to_owned(),
            message: truncate_utf8(message, MAX_PUBLIC_ERROR_BYTES).to_owned(),
        }
    }
}

impl std::fmt::Display for DesktopError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for DesktopError {}

pub type ControlPlaneFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, DesktopError>> + Send + 'a>>;

pub trait ControlPlane: Send + Sync + 'static {
    fn start_runtime(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto>;
    fn get_runtime_status(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto>;
    fn get_runtime_capabilities(&self) -> ControlPlaneFuture<'_, CapabilityDto>;
    fn run_mock_pipeline(&self) -> ControlPlaneFuture<'_, MockPipelineSummaryDto>;
    fn stop_runtime(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto>;
}

#[derive(Clone)]
pub struct CommandService {
    control: Arc<dyn ControlPlane>,
}

impl CommandService {
    pub fn new<C>(control: C) -> Self
    where
        C: ControlPlane,
    {
        Self {
            control: Arc::new(control),
        }
    }

    pub async fn start_runtime(&self) -> Result<RuntimeStatusDto, DesktopError> {
        self.control.start_runtime().await
    }

    pub async fn get_runtime_status(&self) -> Result<RuntimeStatusDto, DesktopError> {
        self.control.get_runtime_status().await
    }

    pub async fn get_runtime_capabilities(&self) -> Result<CapabilityDto, DesktopError> {
        self.control.get_runtime_capabilities().await
    }

    pub async fn run_mock_pipeline(&self) -> Result<MockPipelineSummaryDto, DesktopError> {
        self.control.run_mock_pipeline().await
    }

    pub async fn stop_runtime(&self) -> Result<RuntimeStatusDto, DesktopError> {
        self.control.stop_runtime().await
    }
}

#[tauri::command]
async fn start_runtime(
    request: Request<'_>,
    service: tauri::State<'_, CommandService>,
) -> Result<RuntimeStatusDto, DesktopError> {
    validate_empty_request(&request)?;
    service.start_runtime().await
}

#[tauri::command]
async fn get_runtime_status(
    request: Request<'_>,
    service: tauri::State<'_, CommandService>,
) -> Result<RuntimeStatusDto, DesktopError> {
    validate_empty_request(&request)?;
    service.get_runtime_status().await
}

#[tauri::command]
async fn get_runtime_capabilities(
    request: Request<'_>,
    service: tauri::State<'_, CommandService>,
) -> Result<CapabilityDto, DesktopError> {
    validate_empty_request(&request)?;
    service.get_runtime_capabilities().await
}

#[tauri::command]
async fn run_mock_pipeline(
    request: Request<'_>,
    service: tauri::State<'_, CommandService>,
) -> Result<MockPipelineSummaryDto, DesktopError> {
    validate_empty_request(&request)?;
    service.run_mock_pipeline().await
}

#[tauri::command]
async fn stop_runtime(
    request: Request<'_>,
    service: tauri::State<'_, CommandService>,
) -> Result<RuntimeStatusDto, DesktopError> {
    validate_empty_request(&request)?;
    service.stop_runtime().await
}

fn validate_empty_request(request: &Request<'_>) -> Result<(), DesktopError> {
    match request.body() {
        InvokeBody::Json(serde_json::Value::Null) => Ok(()),
        InvokeBody::Json(serde_json::Value::Object(values)) if values.is_empty() => Ok(()),
        InvokeBody::Json(_) | InvokeBody::Raw(_) => Err(DesktopError::public(
            "invalid_payload",
            "This desktop command does not accept a payload.",
        )),
    }
}

pub fn with_desktop_commands<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    service: CommandService,
) -> tauri::Builder<R> {
    with_desktop_command_handler(builder).manage(service)
}

fn with_desktop_command_handler<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        start_runtime,
        get_runtime_status,
        get_runtime_capabilities,
        run_mock_pipeline,
        stop_runtime
    ])
}

#[derive(Clone, Debug)]
pub struct NavigationPolicy {
    allowed_origin: tauri::Url,
}

impl NavigationPolicy {
    pub fn production(windows: bool) -> Self {
        let origin = if windows {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        };
        Self {
            allowed_origin: origin.parse().expect("fixed packaged origin must be valid"),
        }
    }

    pub fn development() -> Self {
        Self {
            allowed_origin: "http://127.0.0.1:1420"
                .parse()
                .expect("fixed development origin must be valid"),
        }
    }

    pub fn for_mode(development: bool, windows: bool) -> Self {
        if development {
            Self::development()
        } else {
            Self::production(windows)
        }
    }

    pub fn allows(&self, candidate: &tauri::Url) -> bool {
        candidate.username().is_empty()
            && candidate.password().is_none()
            && candidate.scheme() == self.allowed_origin.scheme()
            && candidate.host_str() == self.allowed_origin.host_str()
            && candidate.port_or_known_default() == self.allowed_origin.port_or_known_default()
    }

    fn current() -> Self {
        Self::for_mode(tauri::is_dev(), cfg!(windows))
    }
}

pub fn create_main_window<R: tauri::Runtime>(
    app: &tauri::App<R>,
) -> tauri::Result<tauri::WebviewWindow<R>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == "main")
        .ok_or_else(|| tauri::Error::WindowNotFound)?
        .clone();
    let policy = NavigationPolicy::current();
    tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?
        .on_navigation(move |url| policy.allows(url))
        .build()
}

type ExitCleanup = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub struct ExitDisposition {
    prevent: bool,
    cleanup: Option<ExitCleanup>,
}

impl ExitDisposition {
    pub fn should_prevent(&self) -> bool {
        self.prevent
    }

    pub fn into_cleanup(self) -> Option<ExitCleanup> {
        self.cleanup
    }
}

#[derive(Clone, Debug)]
pub struct ExitCoordinator {
    phase: Arc<AtomicU8>,
}

impl ExitCoordinator {
    pub fn new() -> Self {
        Self {
            phase: Arc::new(AtomicU8::new(EXIT_READY)),
        }
    }

    pub fn request_exit<C, F, O, E>(
        &self,
        timeout: Duration,
        cleanup: C,
        final_exit: E,
    ) -> ExitDisposition
    where
        C: FnOnce() -> F + Send + 'static,
        F: Future<Output = O> + Send + 'static,
        O: Send + 'static,
        E: FnOnce() + Send + 'static,
    {
        match self.phase.compare_exchange(
            EXIT_READY,
            EXIT_CLEANING,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                let coordinator = self.clone();
                ExitDisposition {
                    prevent: true,
                    cleanup: Some(Box::pin(async move {
                        let _ = tokio::time::timeout(timeout, cleanup()).await;
                        coordinator.phase.store(EXIT_FINAL, Ordering::Release);
                        final_exit();
                    })),
                }
            }
            Err(EXIT_FINAL) => ExitDisposition {
                prevent: false,
                cleanup: None,
            },
            Err(_) => ExitDisposition {
                prevent: true,
                cleanup: None,
            },
        }
    }
}

impl Default for ExitCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct RuntimeControlPlane {
    manager: RuntimeManager,
    product: Arc<ProductConfig>,
}

impl RuntimeControlPlane {
    pub fn new(manager: RuntimeManager, product: ProductConfig) -> Self {
        Self {
            manager,
            product: Arc::new(product),
        }
    }
}

pub struct DesktopRuntime {
    manager: RuntimeManager,
    service: CommandService,
}

impl DesktopRuntime {
    pub async fn from_paths(
        paths: NativeArtifactPaths,
        app_data_directory: PathBuf,
    ) -> Result<Self, DesktopError> {
        paths.validate_development()?;
        let product = ProductConfig::load_from_path(&paths.config).map_err(|_| {
            DesktopError::public(
                "invalid_configuration",
                "The packaged product configuration is missing or invalid. Rebuild the desktop app.",
            )
        })?;
        let manager_config = build_manager_config(&product, &paths, &app_data_directory)?;
        Self::from_product(product, manager_config)
    }

    fn from_product(
        product: ProductConfig,
        manager_config: RuntimeManagerConfig,
    ) -> Result<Self, DesktopError> {
        let manager = RuntimeManager::new(manager_config)
            .map_err(|error| DesktopError::from_manager(&error))?;
        let service = CommandService::new(RuntimeControlPlane::new(manager.clone(), product));
        Ok(Self { manager, service })
    }

    pub fn service(&self) -> CommandService {
        self.service.clone()
    }

    pub async fn shutdown(&self) -> Result<RuntimeStatusDto, DesktopError> {
        self.manager
            .stop_runtime()
            .await
            .map(|status| RuntimeStatusDto::from_runtime_status(&status))
            .map_err(|error| DesktopError::from_manager(&error))
    }
}

pub struct DesktopApplication {
    runtime: DesktopRuntime,
    _telemetry: TelemetryGuard,
}

impl DesktopApplication {
    pub async fn initialize(
        paths: NativeArtifactPaths,
        app_data_directory: PathBuf,
    ) -> Result<Self, DesktopError> {
        paths.validate_development()?;
        Self::initialize_validated(paths, app_data_directory)
    }

    pub async fn initialize_bundled(
        paths: NativeArtifactPaths,
        app_data_directory: PathBuf,
    ) -> Result<Self, DesktopError> {
        paths.validate_bundled()?;
        Self::initialize_validated(paths, app_data_directory)
    }

    fn initialize_validated(
        paths: NativeArtifactPaths,
        app_data_directory: PathBuf,
    ) -> Result<Self, DesktopError> {
        let product = ProductConfig::load_from_path(&paths.config).map_err(|_| {
            DesktopError::public(
                "invalid_configuration",
                "The packaged product configuration is missing or invalid. Rebuild the desktop app.",
            )
        })?;
        let manager_config = build_manager_config(&product, &paths, &app_data_directory)?;
        let telemetry = initialize_telemetry(TelemetryConfig::new(
            manager_config.log_directory.clone(),
            manager_config.logging_policy,
        ))
        .map_err(|_| {
            DesktopError::public(
                "telemetry_unavailable",
                "Local telemetry could not be initialized. Check application data permissions.",
            )
        })?;
        let runtime = DesktopRuntime::from_product(product, manager_config)?;
        Ok(Self {
            runtime,
            _telemetry: telemetry,
        })
    }

    pub fn service(&self) -> CommandService {
        self.runtime.service()
    }

    pub async fn shutdown(&self) -> Result<RuntimeStatusDto, DesktopError> {
        self.runtime.shutdown().await
    }
}

pub fn run() -> Result<(), DesktopError> {
    let owner = Arc::new(OnceLock::<DesktopApplication>::new());
    let setup_owner = Arc::clone(&owner);
    let builder = with_desktop_command_handler(tauri::Builder::default()).setup(move |app| {
        let layout = ArtifactLayout::new(current_target_triple())?;
        let app_data_directory = app.path().app_local_data_dir().map_err(|_| {
            DesktopError::public(
                "application_data_unavailable",
                "The application data directory is unavailable.",
            )
        })?;
        #[cfg(debug_assertions)]
        let application = tauri::async_runtime::block_on(DesktopApplication::initialize(
            layout.development_paths(Path::new(env!("CARGO_MANIFEST_DIR"))),
            app_data_directory,
        ))?;
        #[cfg(not(debug_assertions))]
        let application = {
            let executable = std::env::current_exe().map_err(|_| {
                DesktopError::public(
                    "installation_unavailable",
                    "The packaged application location is unavailable.",
                )
            })?;
            let resource_directory = app.path().resource_dir().map_err(|_| {
                DesktopError::public(
                    "installation_unavailable",
                    "The packaged resource location is unavailable.",
                )
            })?;
            tauri::async_runtime::block_on(DesktopApplication::initialize_bundled(
                layout.bundled_paths(&executable, &resource_directory),
                app_data_directory,
            ))?
        };
        app.manage(application.service());
        setup_owner.set(application).map_err(|_| {
            DesktopError::public(
                "desktop_already_initialized",
                "The desktop control plane was initialized more than once.",
            )
        })?;
        create_main_window(app)?;
        Ok(())
    });
    let app = builder.build(tauri::generate_context!()).map_err(|_| {
        DesktopError::public(
            "desktop_startup_failed",
            "The desktop shell could not start.",
        )
    })?;
    let exit = ExitCoordinator::new();
    app.run(move |app_handle, event| {
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
            let cleanup_owner = Arc::clone(&owner);
            let final_handle = app_handle.clone();
            let disposition = exit.request_exit(
                EXIT_CLEANUP_TIMEOUT,
                move || async move {
                    if let Some(application) = cleanup_owner.get() {
                        let _ = application.shutdown().await;
                    }
                },
                move || final_handle.exit(code.unwrap_or(0)),
            );
            if disposition.should_prevent() {
                api.prevent_exit();
            }
            if let Some(cleanup) = disposition.into_cleanup() {
                tauri::async_runtime::spawn(cleanup);
            }
        }
    });
    Ok(())
}

impl ControlPlane for RuntimeControlPlane {
    fn start_runtime(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto> {
        Box::pin(async {
            self.manager
                .start_runtime()
                .await
                .map(|status| RuntimeStatusDto::from_runtime_status(&status))
                .map_err(|error| DesktopError::from_manager(&error))
        })
    }

    fn get_runtime_status(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto> {
        Box::pin(async {
            self.manager
                .get_runtime_status()
                .await
                .map(|status| RuntimeStatusDto::from_runtime_status(&status))
                .map_err(|error| DesktopError::from_manager(&error))
        })
    }

    fn get_runtime_capabilities(&self) -> ControlPlaneFuture<'_, CapabilityDto> {
        Box::pin(async {
            let observation = self
                .manager
                .get_capabilities()
                .await
                .map_err(|error| DesktopError::from_manager(&error))?;
            let status = self
                .manager
                .get_runtime_status()
                .await
                .map_err(|error| DesktopError::from_manager(&error))?;
            let profile = status
                .capability_profile(&self.product, &observation)
                .map_err(|_| {
                    DesktopError::public(
                        "capability_unavailable",
                        "Capabilities changed while being queried. Refresh status and retry.",
                    )
                })?;
            Ok(CapabilityDto::from_profile(&profile))
        })
    }

    fn run_mock_pipeline(&self) -> ControlPlaneFuture<'_, MockPipelineSummaryDto> {
        Box::pin(async {
            self.manager
                .run_mock_pipeline()
                .await
                .map(Into::into)
                .map_err(|error| DesktopError::from_manager(&error))
        })
    }

    fn stop_runtime(&self) -> ControlPlaneFuture<'_, RuntimeStatusDto> {
        Box::pin(async {
            self.manager
                .stop_runtime()
                .await
                .map(|status| RuntimeStatusDto::from_runtime_status(&status))
                .map_err(|error| DesktopError::from_manager(&error))
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeArtifactPaths {
    pub runtime: PathBuf,
    pub plugin: PathBuf,
    pub config: PathBuf,
}

impl NativeArtifactPaths {
    pub fn validate_development(&self) -> Result<(), DesktopError> {
        if self.all_files_exist() {
            return Ok(());
        }
        Err(DesktopError::public(
            "native_artifacts_missing",
            &format!("Native artifacts are missing. Run `{PREPARE_COMMAND}`, then retry."),
        ))
    }

    pub fn validate_bundled(&self) -> Result<(), DesktopError> {
        if self.all_files_exist() {
            return Ok(());
        }
        Err(DesktopError::public(
            "native_artifacts_missing",
            "The application installation is incomplete. Reinstall AI Voice Studio.",
        ))
    }

    fn all_files_exist(&self) -> bool {
        self.runtime.is_file() && self.plugin.is_file() && self.config.is_file()
    }
}

pub fn build_manager_config(
    product: &ProductConfig,
    paths: &NativeArtifactPaths,
    app_data_directory: &Path,
) -> Result<RuntimeManagerConfig, DesktopError> {
    RuntimeManagerConfig::from_product_config(
        product,
        paths.runtime.clone(),
        paths.plugin.clone(),
        app_data_directory.to_path_buf(),
    )
    .map_err(|_| {
        DesktopError::public(
            "invalid_configuration",
            "The validated desktop configuration could not be applied to the Runtime.",
        )
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactLayout {
    target: String,
    windows: bool,
    library_extension: &'static str,
}

impl ArtifactLayout {
    pub fn new(target: &str) -> Result<Self, DesktopError> {
        if target.is_empty()
            || target.len() > 128
            || !target
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(DesktopError::public(
                "invalid_target",
                "The build target triple is invalid.",
            ));
        }
        let windows = target.contains("windows");
        let library_extension = if windows {
            "dll"
        } else if target.contains("apple-darwin") {
            "dylib"
        } else {
            "so"
        };
        Ok(Self {
            target: target.to_owned(),
            windows,
            library_extension,
        })
    }

    pub fn development_paths(&self, tauri_root: &Path) -> NativeArtifactPaths {
        NativeArtifactPaths {
            runtime: join_portable(
                tauri_root,
                &format!(
                    "binaries/voice-runtime-{}{}",
                    self.target,
                    if self.windows { ".exe" } else { "" }
                ),
                self.windows,
            ),
            plugin: join_portable(
                tauri_root,
                &format!(
                    "resources/native/aivs_mock_voice_engine-{}.{}",
                    self.target, self.library_extension
                ),
                self.windows,
            ),
            config: join_portable(tauri_root, "resources/config/config.json", self.windows),
        }
    }

    pub fn bundled_paths(&self, executable: &Path, resource_dir: &Path) -> NativeArtifactPaths {
        let executable_dir = parent_portable(executable, self.windows);
        NativeArtifactPaths {
            runtime: join_portable(
                &executable_dir,
                if self.windows {
                    "voice-runtime.exe"
                } else {
                    "voice-runtime"
                },
                self.windows,
            ),
            plugin: join_portable(
                resource_dir,
                &format!(
                    "native/aivs_mock_voice_engine-{}.{}",
                    self.target, self.library_extension
                ),
                self.windows,
            ),
            config: join_portable(resource_dir, "config/config.json", self.windows),
        }
    }
}

fn parent_portable(path: &Path, windows: bool) -> PathBuf {
    if !windows {
        return path.parent().unwrap_or(path).to_path_buf();
    }
    let value = path.to_string_lossy();
    value
        .rsplit_once(['\\', '/'])
        .map_or_else(|| path.to_path_buf(), |(parent, _)| PathBuf::from(parent))
}

fn join_portable(base: &Path, child: &str, windows: bool) -> PathBuf {
    if !windows {
        return base.join(child);
    }
    let base = base.to_string_lossy();
    let child = child.replace('/', "\\");
    PathBuf::from(format!("{}\\{}", base.trim_end_matches(['\\', '/']), child))
}

fn platform_name(value: Platform) -> &'static str {
    match value {
        Platform::Macos => "macos",
        Platform::Windows => "windows",
        Platform::Linux => "linux",
        Platform::Unknown => "unknown",
    }
}

fn architecture_name(value: Architecture) -> &'static str {
    match value {
        Architecture::Arm64 => "arm64",
        Architecture::X86_64 => "x86_64",
        Architecture::Unknown => "unknown",
    }
}

fn backend_name(value: RuntimeBackend) -> &'static str {
    match value {
        RuntimeBackend::Mock => "mock",
        RuntimeBackend::Unavailable => "unavailable",
    }
}

fn availability_name(value: CapabilityAvailability) -> &'static str {
    match value {
        CapabilityAvailability::Available => "available",
        CapabilityAvailability::Unavailable => "unavailable",
        CapabilityAvailability::Unknown => "unknown",
        CapabilityAvailability::NotEvaluated => "not_evaluated",
    }
}

fn engine_name(value: EngineIdentity) -> &'static str {
    match value {
        EngineIdentity::AivsMockV1 => "aivs-mock-v1",
    }
}

fn error_code_name(value: ErrorCode) -> &'static str {
    match value {
        ErrorCode::Success => "internal_error",
        ErrorCode::UnsupportedProtocolVersion => "unsupported_protocol_version",
        ErrorCode::UnsupportedVoiceEngineAbi => "unsupported_voice_engine_abi",
        ErrorCode::MalformedFrame => "malformed_frame",
        ErrorCode::FrameTooLarge => "frame_too_large",
        ErrorCode::RuntimeUnavailable => "runtime_unavailable",
        ErrorCode::RuntimeShuttingDown => "runtime_shutting_down",
        ErrorCode::EngineUnavailable => "engine_unavailable",
        ErrorCode::InvalidArgument => "invalid_argument",
        ErrorCode::InvalidState => "invalid_state",
        ErrorCode::BufferTooSmall => "buffer_too_small",
        ErrorCode::InternalError => "internal_error",
    }
}

fn truncate_utf8(value: &str, maximum: usize) -> &str {
    if value.len() <= maximum {
        return value;
    }
    let mut boundary = maximum;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &value[..boundary]
}

pub const fn current_target_triple() -> &'static str {
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    return "aarch64-apple-darwin";
    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    return "x86_64-apple-darwin";
    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    return "x86_64-pc-windows-msvc";
    #[cfg(all(target_arch = "aarch64", target_os = "windows"))]
    return "aarch64-pc-windows-msvc";
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    return "x86_64-unknown-linux-gnu";
    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    return "aarch64-unknown-linux-gnu";
    #[allow(unreachable_code)]
    "unsupported-target"
}
