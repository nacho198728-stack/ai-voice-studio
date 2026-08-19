use std::path::PathBuf;
use std::time::Duration;

use ai_voice_config::ProductConfig;
use ai_voice_runtime_host::RuntimeManagerConfig;

fn default_config() -> ProductConfig {
    ProductConfig::load_from_bytes(include_bytes!("../../../config/config.json")).unwrap()
}

fn absolute_resource(name: &str) -> PathBuf {
    std::env::current_dir().unwrap().join("build").join(name)
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
