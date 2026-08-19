//! Shared Rust contract boundary for AI Voice Studio.

mod generated;

pub use generated::*;

#[cfg(test)]
mod tests {
    use super::{
        ErrorCategory, ErrorCode, IPC_PROTOCOL_CURRENT_VERSION,
        IPC_PROTOCOL_MINIMUM_COMPATIBLE_VERSION, RUNTIME_VERSION, VOICE_ENGINE_ABI_CURRENT_VERSION,
        VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION, VOICE_ENGINE_ABI_V1_VERSION,
        error_code_category, error_code_from_value, is_ipc_protocol_compatible,
        is_voice_engine_abi_compatible,
    };

    #[test]
    fn maps_canonical_versions_and_compatibility_ranges() {
        assert_eq!(RUNTIME_VERSION, "0.0.0");
        assert_eq!(IPC_PROTOCOL_CURRENT_VERSION, 1);
        assert_eq!(IPC_PROTOCOL_MINIMUM_COMPATIBLE_VERSION, 1);
        assert!(is_ipc_protocol_compatible(1));
        assert!(!is_ipc_protocol_compatible(0));
        assert!(!is_ipc_protocol_compatible(2));
        assert_eq!(VOICE_ENGINE_ABI_CURRENT_VERSION, 1);
        assert_eq!(VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION, 1);
        assert_eq!(VOICE_ENGINE_ABI_V1_VERSION, 1);
        assert!(is_voice_engine_abi_compatible(1));
        assert!(!is_voice_engine_abi_compatible(0));
    }

    #[test]
    fn maps_canonical_error_code_numbers() {
        assert_eq!(ErrorCode::Success.value(), 0);
        assert_eq!(ErrorCode::UnsupportedProtocolVersion.value(), 1000);
        assert_eq!(ErrorCode::MalformedFrame.value(), 1100);
        assert_eq!(ErrorCode::RuntimeUnavailable.value(), 1200);
        assert_eq!(ErrorCode::EngineUnavailable.value(), 1300);
        assert_eq!(ErrorCode::InternalError.value(), 1900);
        assert_eq!(error_code_from_value(1101), Some(ErrorCode::FrameTooLarge));
        assert_eq!(error_code_from_value(42), None);
        assert_eq!(
            error_code_category(ErrorCode::MalformedFrame),
            ErrorCategory::Framing
        );
        assert_eq!(
            error_code_category(ErrorCode::InternalError),
            ErrorCategory::Internal
        );
    }
}
