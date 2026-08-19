use std::path::PathBuf;
use std::time::Duration;

use ai_voice_capability::{
    CapabilityAvailability, EngineIdentity, ManagerHealth, NativeEngine, RuntimeBackend,
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

fn connected_status() -> RuntimeStatus {
    RuntimeStatus {
        state: RuntimeState::Connected,
        generation: 7,
        pid: Some(42),
        hello: Some(Hello {
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
fn status_and_native_capabilities_map_to_shared_truthful_profile() {
    let product = default_config();
    let native = RuntimeCapabilities::from_canonical_json(
        br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#,
    )
    .unwrap();
    assert_eq!(native.backend, RuntimeBackend::Mock);
    assert_eq!(native.engine, NativeEngine::AivsMockV1);

    let profile = connected_status().capability_profile(&product, Some(&native));

    assert_eq!(profile.manager.health, ManagerHealth::Connected);
    assert_eq!(profile.manager.generation, 7);
    assert_eq!(profile.runtime.backend, RuntimeBackend::Mock);
    assert_eq!(
        profile.runtime.availability,
        CapabilityAvailability::Available
    );
    assert_eq!(profile.engine.identity, Some(EngineIdentity::AivsMockV1));
    assert_eq!(
        serde_json::to_string(&profile).unwrap(),
        r#"{"schema_version":1,"platform":"macos","architecture":"arm64","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"available"},"engine":{"identity":"aivs-mock-v1","availability":"available"},"manager":{"health":"connected","generation":7}}"#
    );
}

#[test]
fn connected_without_query_is_not_evaluated_and_stopped_is_unavailable() {
    let product = default_config();
    let connected = connected_status().capability_profile(&product, None);
    assert_eq!(
        connected.runtime.availability,
        CapabilityAvailability::NotEvaluated
    );
    assert_eq!(
        connected.engine.availability,
        CapabilityAvailability::NotEvaluated
    );

    let mut stopped_status = connected_status();
    stopped_status.state = RuntimeState::Stopped;
    stopped_status.pid = None;
    let stopped = stopped_status.capability_profile(&product, None);
    assert_eq!(stopped.manager.health, ManagerHealth::Stopped);
    assert_eq!(
        stopped.runtime.availability,
        CapabilityAvailability::Unavailable
    );
    assert_eq!(
        stopped.engine.availability,
        CapabilityAvailability::Unavailable
    );
}
