use ai_voice_capability::{
    Architecture, EngineIdentity, NativeCapabilityError, NativeCapabilityMismatch,
    NativeRuntimeCapabilities, Platform, RuntimeBackend,
};

const MOCK_NATIVE: &[u8] = br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#;
const UNAVAILABLE_NATIVE: &[u8] = br#"{"platform":"windows","architecture":"x86_64","runtime_version":"0.0.0","protocol_version":1,"backend":"unavailable","engine":"unavailable"}"#;

fn native(bytes: &[u8]) -> NativeRuntimeCapabilities {
    NativeRuntimeCapabilities::from_canonical_json(bytes).unwrap()
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
