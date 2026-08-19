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

mod manager;
mod payload;

pub use ai_voice_capability::{
    Architecture, CapabilityAvailability, CapabilityProfile, CapabilityProfileError,
    EngineCapability, EngineIdentity, ManagerCapability, ManagerHealth, NativeCapabilityError,
    NativeCapabilityMismatch, NativeRuntimeCapabilities as RuntimeCapabilities, Platform,
    RuntimeBackend, RuntimeCapability,
};
pub use manager::{
    CapabilityObservation, CapabilityObservationKind, ManagerError, ManagerErrorKind,
    RestartPolicyStatus, RuntimeExit, RuntimeExitReason, RuntimeManager, RuntimeManagerConfig,
    RuntimeState, RuntimeStatus,
};
pub use payload::{Hello, MockPipelineSummary};
