use std::path::PathBuf;
use std::time::Duration;

use ai_voice_capability::{
    CapabilityAvailability, CapabilityObservation, CapabilityProfileError, EngineIdentity,
    ManagerHealth, ObservedRuntimeCapabilities, RuntimeBackend,
};
use ai_voice_config::ProductConfig;
use ai_voice_runtime_host::{
    Hello, RestartPolicyStatus, RuntimeCapabilities, RuntimeManagerConfig, RuntimeState,
    RuntimeStatus,
};

fn default_config() -> ProductConfig {
    ProductConfig::load_from_bytes(include_bytes!("../../../config/config.json")).unwrap()
}

fn absolute_resource(name: &str) -> PathBuf {
    std::env::current_dir().unwrap().join("build").join(name)
}

fn status(state: RuntimeState, generation: u64) -> RuntimeStatus {
    RuntimeStatus {
        state,
        generation,
        pid: (state == RuntimeState::Connected).then_some(42),
        hello: (state == RuntimeState::Connected).then(|| Hello {
            runtime_version: "0.0.0".to_owned(),
            protocol_version: 1,
            generation: 1,
            health: "starting".to_owned(),
        }),
        last_exit: None,
        last_error: None,
        stderr_tail: Vec::new(),
        restart_policy: RestartPolicyStatus {
            automatic_restart: false,
            max_attempts: 0,
            attempts_observed: 0,
        },
    }
}

#[test]
fn validated_product_config_maps_every_launch_setting_but_not_resource_authority() {
    let product = default_config();
    let runtime = absolute_resource("voice-runtime");
    let plugin = absolute_resource("aivs-mock");

    let launch =
        RuntimeManagerConfig::from_product_config(&product, runtime.clone(), plugin.clone())
            .unwrap();

    assert_eq!(launch.runtime_path, runtime);
    assert_eq!(launch.plugin_path, plugin);
    assert_eq!(launch.mock_work_iterations, 0);
    assert_eq!(launch.handshake_timeout, Duration::from_millis(2_000));
    assert_eq!(launch.request_timeout, Duration::from_millis(2_000));
    assert_eq!(launch.shutdown_timeout, Duration::from_millis(2_000));
    assert_eq!(launch.max_in_flight, 16);
    assert_eq!(launch.command_queue_capacity, 32);
    assert_eq!(launch.event_queue_capacity, 32);
    assert_eq!(launch.stderr_tail_bytes, 8_192);
    assert_eq!(launch.restart_max_attempts, 0);

    let other_runtime = absolute_resource("other-runtime");
    let other_plugin = absolute_resource("other-plugin");
    let other = RuntimeManagerConfig::from_product_config(
        &product,
        other_runtime.clone(),
        other_plugin.clone(),
    )
    .unwrap();
    assert_eq!(other.runtime_path, other_runtime);
    assert_eq!(other.plugin_path, other_plugin);
}

#[test]
fn status_and_generation_bound_native_observation_map_to_shared_profile() {
    let product = default_config();
    let native = RuntimeCapabilities::from_canonical_json(
        br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#,
    )
    .unwrap();
    let observation =
        CapabilityObservation::observed(ObservedRuntimeCapabilities::new(7, native).unwrap());

    let profile = status(RuntimeState::Connected, 7)
        .capability_profile(&product, &observation)
        .unwrap();

    assert_eq!(profile.manager().health(), ManagerHealth::Connected);
    assert_eq!(profile.manager().generation(), 7);
    assert_eq!(profile.runtime().backend(), RuntimeBackend::Mock);
    assert_eq!(
        profile.runtime().availability(),
        CapabilityAvailability::Available
    );
    assert_eq!(
        profile.engine().identity(),
        Some(EngineIdentity::AivsMockV1)
    );
}

#[test]
fn stale_observation_is_rejected_after_generation_advance() {
    let product = default_config();
    let old_native = RuntimeCapabilities::from_canonical_json(
        br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#,
    )
    .unwrap();
    let old_observation =
        CapabilityObservation::observed(ObservedRuntimeCapabilities::new(1, old_native).unwrap());

    let error = status(RuntimeState::Connected, 2)
        .capability_profile(&product, &old_observation)
        .unwrap_err();

    assert_eq!(
        error,
        CapabilityProfileError::StaleObservation {
            manager_generation: 2,
            observation_generation: 1,
        }
    );
}
