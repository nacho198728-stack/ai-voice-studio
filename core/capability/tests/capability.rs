use ai_voice_capability::{
    Architecture, CapabilityAvailability, CapabilityEvaluation, CapabilityProfile,
    CapabilityProfileError, EngineIdentity, ManagerCapability, ManagerHealth,
    NativeCapabilityError, NativeCapabilityMismatch, NativeRuntimeCapabilities, Platform,
    RuntimeBackend,
};

const MOCK_NATIVE: &[u8] = br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#;
const UNAVAILABLE_NATIVE: &[u8] = br#"{"platform":"windows","architecture":"x86_64","runtime_version":"0.0.0","protocol_version":1,"backend":"unavailable","engine":"unavailable"}"#;

fn native(bytes: &[u8]) -> NativeRuntimeCapabilities {
    NativeRuntimeCapabilities::from_canonical_json(bytes).unwrap()
}

fn profile(
    health: ManagerHealth,
    generation: u64,
    evaluation: &CapabilityEvaluation,
) -> Result<CapabilityProfile, CapabilityProfileError> {
    CapabilityProfile::from_evaluation(
        RuntimeBackend::Mock,
        ManagerCapability::new(health, generation)?,
        evaluation,
    )
}

#[test]
fn native_capability_errors_are_field_specific_actionable_and_payload_free() {
    let cases = [
        (MOCK_NATIVE, None),
        (
            br#"{"platform":"macos","architecture":"arm64","runtime_version":"9.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#.as_slice(),
            Some(NativeCapabilityError::Mismatch(NativeCapabilityMismatch::RuntimeVersion)),
        ),
        (
            br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":2,"backend":"mock","engine":"aivs-mock-v1"}"#,
            Some(NativeCapabilityError::Mismatch(NativeCapabilityMismatch::ProtocolVersion)),
        ),
        (
            br#"{"platform":"ios","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#,
            Some(NativeCapabilityError::Mismatch(NativeCapabilityMismatch::Platform)),
        ),
        (
            br#"{"platform":"macos","architecture":"riscv64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#,
            Some(NativeCapabilityError::Mismatch(NativeCapabilityMismatch::Architecture)),
        ),
        (
            br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"unavailable"}"#,
            Some(NativeCapabilityError::Mismatch(NativeCapabilityMismatch::BackendEnginePair)),
        ),
        (
            br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock"}"#,
            Some(NativeCapabilityError::MalformedShape),
        ),
        (
            br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1","cpu":"hidden"}"#,
            Some(NativeCapabilityError::MalformedShape),
        ),
        (
            br#"{"architecture":"arm64","platform":"macos","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#,
            Some(NativeCapabilityError::NonCanonicalJson),
        ),
    ];

    for (bytes, expected) in cases {
        match expected {
            None => {
                NativeRuntimeCapabilities::from_canonical_json(bytes).unwrap();
            }
            Some(expected) => {
                let error = NativeRuntimeCapabilities::from_canonical_json(bytes).unwrap_err();
                assert_eq!(error, expected);
                let diagnostic = error.to_string();
                assert!(!diagnostic.is_empty());
                for raw_value in ["9.0.0", "riscv64", "ios", "unavailable"] {
                    assert!(!diagnostic.contains(raw_value));
                }
                let _: &dyn std::error::Error = &error;
            }
        }
    }
}

#[test]
fn native_capabilities_expose_only_validated_getters() {
    let mock = native(MOCK_NATIVE);
    assert_eq!(mock.platform(), Platform::Macos);
    assert_eq!(mock.architecture(), Architecture::Arm64);
    assert_eq!(mock.backend(), RuntimeBackend::Mock);
    assert_eq!(mock.engine_identity(), Some(EngineIdentity::AivsMockV1));

    let unavailable = native(UNAVAILABLE_NATIVE);
    assert_eq!(unavailable.platform(), Platform::Windows);
    assert_eq!(unavailable.architecture(), Architecture::X86_64);
    assert_eq!(unavailable.backend(), RuntimeBackend::Unavailable);
    assert_eq!(unavailable.engine_identity(), None);
}

#[test]
fn literal_truth_table_covers_every_manager_state_and_query_outcome() {
    let mock = native(MOCK_NATIVE);
    let unavailable = native(UNAVAILABLE_NATIVE);
    let cases = [
        (
            "stopped",
            ManagerHealth::Stopped,
            0,
            CapabilityEvaluation::not_evaluated(),
            r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"unavailable"},"engine":{"identity":"aivs-mock-v1","availability":"unavailable"},"manager":{"health":"stopped","generation":0}}"#,
        ),
        (
            "starting",
            ManagerHealth::Starting,
            1,
            CapabilityEvaluation::not_evaluated(),
            r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"not_evaluated"},"engine":{"identity":"aivs-mock-v1","availability":"not_evaluated"},"manager":{"health":"starting","generation":1}}"#,
        ),
        (
            "connected-not-evaluated",
            ManagerHealth::Connected,
            1,
            CapabilityEvaluation::not_evaluated(),
            r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"available"},"engine":{"identity":"aivs-mock-v1","availability":"not_evaluated"},"manager":{"health":"connected","generation":1}}"#,
        ),
        (
            "connected-inconclusive",
            ManagerHealth::Connected,
            1,
            CapabilityEvaluation::inconclusive(),
            r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"available"},"engine":{"identity":"aivs-mock-v1","availability":"unknown"},"manager":{"health":"connected","generation":1}}"#,
        ),
        (
            "connected-mock",
            ManagerHealth::Connected,
            1,
            CapabilityEvaluation::observed(mock.clone()),
            r#"{"schema_version":1,"platform":"macos","architecture":"arm64","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"available"},"engine":{"identity":"aivs-mock-v1","availability":"available"},"manager":{"health":"connected","generation":1}}"#,
        ),
        (
            "connected-unavailable-engine",
            ManagerHealth::Connected,
            1,
            CapabilityEvaluation::observed(unavailable),
            r#"{"schema_version":1,"platform":"windows","architecture":"x86_64","runtime":{"version":"0.0.0","protocol_version":1,"backend":"unavailable","availability":"available"},"engine":{"identity":null,"availability":"unavailable"},"manager":{"health":"connected","generation":1}}"#,
        ),
        (
            "stopping",
            ManagerHealth::Stopping,
            1,
            CapabilityEvaluation::not_evaluated(),
            r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"unavailable"},"engine":{"identity":"aivs-mock-v1","availability":"unavailable"},"manager":{"health":"stopping","generation":1}}"#,
        ),
        (
            "crashed",
            ManagerHealth::Crashed,
            1,
            CapabilityEvaluation::not_evaluated(),
            r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"unavailable"},"engine":{"identity":"aivs-mock-v1","availability":"unavailable"},"manager":{"health":"crashed","generation":1}}"#,
        ),
        (
            "error",
            ManagerHealth::Error,
            1,
            CapabilityEvaluation::not_evaluated(),
            r#"{"schema_version":1,"platform":"unknown","architecture":"unknown","runtime":{"version":"0.0.0","protocol_version":1,"backend":"mock","availability":"unavailable"},"engine":{"identity":"aivs-mock-v1","availability":"unavailable"},"manager":{"health":"error","generation":1}}"#,
        ),
    ];

    for (name, health, generation, observation, expected) in cases {
        let actual =
            serde_json::to_string(&profile(health, generation, &observation).unwrap()).unwrap();
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn unknown_requires_an_inconclusive_query() {
    let not_evaluated = profile(
        ManagerHealth::Connected,
        7,
        &CapabilityEvaluation::not_evaluated(),
    )
    .unwrap();
    assert_eq!(
        not_evaluated.engine().availability(),
        CapabilityAvailability::NotEvaluated
    );

    let inconclusive = profile(
        ManagerHealth::Connected,
        7,
        &CapabilityEvaluation::inconclusive(),
    )
    .unwrap();
    assert_eq!(
        inconclusive.engine().availability(),
        CapabilityAvailability::Unknown
    );

    assert!(ManagerCapability::new(ManagerHealth::Connected, 0).is_err());
}

#[test]
fn capability_profile_contains_no_inferred_or_sensitive_hardware() {
    let observation = CapabilityEvaluation::observed(native(MOCK_NATIVE));
    let actual =
        serde_json::to_string(&profile(ManagerHealth::Connected, 7, &observation).unwrap())
            .unwrap();
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
