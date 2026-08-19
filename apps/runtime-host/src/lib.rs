//! Rust control-plane owner for the isolated native voice Runtime.

mod manager;
mod payload;

pub use ai_voice_capability::{
    Architecture, CapabilityAvailability, CapabilityObservation, CapabilityProfile,
    CapabilityProfileError, EngineCapability, EngineIdentity, ManagerCapability, ManagerHealth,
    NativeCapabilityError, NativeCapabilityMismatch,
    NativeRuntimeCapabilities as RuntimeCapabilities, ObservedRuntimeCapabilities, Platform,
    RuntimeBackend, RuntimeCapability,
};
pub use manager::{
    ManagerError, ManagerErrorKind, RestartPolicyStatus, RuntimeExit, RuntimeExitReason,
    RuntimeManager, RuntimeManagerConfig, RuntimeState, RuntimeStatus,
};
pub use payload::{Hello, MockPipelineSummary};
