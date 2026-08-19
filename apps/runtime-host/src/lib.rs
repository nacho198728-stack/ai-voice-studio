//! Rust control-plane owner for the isolated native voice Runtime.
//!
//! Capability observations are minted by the manager actor and cannot be
//! rebound to another Runtime generation by callers.
//!
//! ```compile_fail
//! use ai_voice_runtime_host::{CapabilityObservation, RuntimeCapabilities};
//!
//! let native = RuntimeCapabilities::from_canonical_json(
//!     br#"{"platform":"unknown","architecture":"unknown","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#,
//! ).unwrap();
//! let copied = native.clone();
//! let forged = CapabilityObservation::observed(2, copied);
//! # let _ = forged;
//! ```
//!
//! Failed and not-yet-run queries cannot be labeled with a caller-selected
//! generation either.
//!
//! ```compile_fail
//! use ai_voice_contracts::ErrorCode;
//! use ai_voice_runtime_host::{CapabilityObservation, ManagerError, ManagerErrorKind};
//!
//! let error = ManagerError {
//!     code: ErrorCode::EngineUnavailable,
//!     kind: ManagerErrorKind::Remote,
//!     message: "bounded failure".to_owned(),
//! };
//! let rebound_failure = CapabilityObservation::inconclusive(2, error);
//! let forged_not_evaluated = CapabilityObservation::not_evaluated(2);
//! # let _ = (rebound_failure, forged_not_evaluated);
//! ```
//!
//! A copied native result cannot bypass the opaque host observation by using a
//! lower-level profile constructor with a caller-selected generation.
//!
//! ```compile_fail
//! use ai_voice_capability::{
//!     CapabilityEvaluation, CapabilityProfile, ManagerCapability, ManagerHealth,
//!     NativeRuntimeCapabilities, RuntimeBackend,
//! };
//!
//! let copied_old_native = NativeRuntimeCapabilities::from_canonical_json(
//!     br#"{"platform":"unknown","architecture":"unknown","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#,
//! ).unwrap();
//! let caller_generation = ManagerCapability::new(ManagerHealth::Connected, 2).unwrap();
//! let relabeled = CapabilityEvaluation::observed(copied_old_native);
//! let forged = CapabilityProfile::from_evaluation(
//!     RuntimeBackend::Mock,
//!     caller_generation,
//!     &relabeled,
//! ).unwrap();
//! # let _ = forged;
//! ```

mod manager;
mod payload;

pub use ai_voice_capability::{
    Architecture, CapabilityAvailability, EngineIdentity, NativeCapabilityError,
    NativeCapabilityMismatch, NativeRuntimeCapabilities as RuntimeCapabilities, Platform,
    RuntimeBackend,
};
pub use manager::{
    CapabilityObservation, CapabilityObservationKind, CapabilityProfile, CapabilityProfileError,
    EngineCapability, ManagerCapability, ManagerError, ManagerErrorKind, ManagerHealth,
    RestartPolicyStatus, RuntimeCapability, RuntimeExit, RuntimeExitReason, RuntimeManager,
    RuntimeManagerConfig, RuntimeState, RuntimeStatus,
};
pub use payload::{Hello, MockPipelineSummary};
