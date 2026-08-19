use ai_voice_capability::{
    Architecture, CapabilityAvailability, CapabilityProfile, EngineIdentity, ManagerCapability,
    ManagerHealth, NativeRuntimeCapabilities, Platform, RuntimeBackend,
};

const MOCK_NATIVE: &[u8] = br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#;
const UNAVAILABLE_NATIVE: &[u8] = br#"{"platform":"windows","architecture":"x86_64","runtime_version":"0.0.0","protocol_version":1,"backend":"unavailable","engine":"unavailable"}"#;

#[test]
fn native_capabilities_normalize_only_truthful_backend_engine_pairs() {
    let mock = NativeRuntimeCapabilities::from_canonical_json(MOCK_NATIVE).unwrap();
    assert_eq!(mock.platform, Platform::Macos);
    assert_eq!(mock.architecture, Architecture::Arm64);
    assert_eq!(mock.backend, RuntimeBackend::Mock);

    let unavailable = NativeRuntimeCapabilities::from_canonical_json(UNAVAILABLE_NATIVE).unwrap();
    assert_eq!(unavailable.backend, RuntimeBackend::Unavailable);

    for invalid in [
        br#"{"platform":"macos","architecture":"arm64","runtime_version":"9.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#.as_slice(),
        br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":2,"backend":"mock","engine":"aivs-mock-v1"}"#,
        br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"unavailable"}"#,
        br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"unavailable","engine":"aivs-mock-v1"}"#,
        br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"other"}"#,
        br#"{"platform":"ios","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#,
        br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1","cpu":"M5"}"#,
        br#"{"architecture":"arm64","platform":"macos","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#,
    ] {
        assert!(NativeRuntimeCapabilities::from_canonical_json(invalid).is_err());
    }
}

#[test]
fn capability_profile_json_is_stable_and_contains_no_inferred_hardware() {
    let native = NativeRuntimeCapabilities::from_canonical_json(MOCK_NATIVE).unwrap();
    let profile = CapabilityProfile::from_observation(
        RuntimeBackend::Mock,
        ManagerCapability {
            health: ManagerHealth::Connected,
            generation: 7,
        },
        Some(&native),
    );

    let actual = serde_json::to_string(&profile).unwrap();
    assert_eq!(
        actual,
        r#"{"schema_version":1,"platform":"macos","architecture":"arm64","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"available"},"engine":{"identity":"aivs-mock-v1","availability":"available"},"manager":{"health":"connected","generation":7}}"#
    );
    for forbidden in [
        "cpu",
        "gpu",
        "ram",
        "npu",
        "audio",
        "device",
        "driver",
        "benchmark",
        "machine",
    ] {
        assert!(
            !actual.contains(forbidden),
            "leaked unsupported field {forbidden}"
        );
    }
}

#[test]
fn unavailable_unknown_and_not_evaluated_are_distinct() {
    let connected_unqueried = CapabilityProfile::from_observation(
        RuntimeBackend::Mock,
        ManagerCapability {
            health: ManagerHealth::Connected,
            generation: 1,
        },
        None,
    );
    assert_eq!(connected_unqueried.platform, Platform::Unknown);
    assert_eq!(connected_unqueried.architecture, Architecture::Unknown);
    assert_eq!(
        connected_unqueried.runtime.availability,
        CapabilityAvailability::NotEvaluated
    );
    assert_eq!(
        connected_unqueried.engine.identity,
        Some(EngineIdentity::AivsMockV1)
    );
    assert_eq!(
        connected_unqueried.engine.availability,
        CapabilityAvailability::NotEvaluated
    );

    let stopped = CapabilityProfile::from_observation(
        RuntimeBackend::Mock,
        ManagerCapability {
            health: ManagerHealth::Stopped,
            generation: 0,
        },
        None,
    );
    assert_eq!(
        stopped.runtime.availability,
        CapabilityAvailability::Unavailable
    );
    assert_eq!(
        stopped.engine.availability,
        CapabilityAvailability::Unavailable
    );

    let native = NativeRuntimeCapabilities::from_canonical_json(UNAVAILABLE_NATIVE).unwrap();
    let unavailable = CapabilityProfile::from_observation(
        RuntimeBackend::Mock,
        ManagerCapability {
            health: ManagerHealth::Connected,
            generation: 2,
        },
        Some(&native),
    );
    assert_eq!(unavailable.runtime.backend, RuntimeBackend::Unavailable);
    assert_eq!(
        unavailable.runtime.availability,
        CapabilityAvailability::Unavailable
    );
    assert_eq!(unavailable.engine.identity, None);
    assert_eq!(
        unavailable.engine.availability,
        CapabilityAvailability::Unavailable
    );

    let unknown = serde_json::to_string(&CapabilityAvailability::Unknown).unwrap();
    assert_eq!(unknown, r#""unknown""#);
    assert_ne!(
        CapabilityAvailability::Unknown,
        CapabilityAvailability::Unavailable
    );
    assert_ne!(
        CapabilityAvailability::Unavailable,
        CapabilityAvailability::NotEvaluated
    );
}
