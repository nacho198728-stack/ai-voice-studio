//! Stable, privacy-minimal product capability contract.

use ai_voice_contracts::{IPC_PROTOCOL_CURRENT_VERSION, RUNTIME_VERSION};
use serde::{Deserialize, Serialize};

pub const CAPABILITY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Macos,
    Windows,
    Linux,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    Arm64,
    X86_64,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBackend {
    Mock,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum NativeEngine {
    #[serde(rename = "aivs-mock-v1")]
    AivsMockV1,
    #[serde(rename = "unavailable")]
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRuntimeCapabilities {
    pub platform: Platform,
    pub architecture: Architecture,
    pub runtime_version: String,
    pub protocol_version: u32,
    pub backend: RuntimeBackend,
    pub engine: NativeEngine,
}

impl NativeRuntimeCapabilities {
    pub fn from_canonical_json(bytes: &[u8]) -> Result<Self, NativeCapabilityError> {
        let parsed: Self =
            serde_json::from_slice(bytes).map_err(|_| NativeCapabilityError::Malformed)?;
        let canonical =
            serde_json::to_vec(&parsed).map_err(|_| NativeCapabilityError::Malformed)?;
        if canonical != bytes {
            return Err(NativeCapabilityError::Malformed);
        }
        if parsed.runtime_version != RUNTIME_VERSION
            || parsed.protocol_version != IPC_PROTOCOL_CURRENT_VERSION
            || !matches!(
                (parsed.backend, parsed.engine),
                (RuntimeBackend::Mock, NativeEngine::AivsMockV1)
                    | (RuntimeBackend::Unavailable, NativeEngine::Unavailable)
            )
        {
            return Err(NativeCapabilityError::ContractMismatch);
        }
        Ok(parsed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCapabilityError {
    Malformed,
    ContractMismatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAvailability {
    Available,
    Unavailable,
    Unknown,
    NotEvaluated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EngineIdentity {
    #[serde(rename = "aivs-mock-v1")]
    AivsMockV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagerHealth {
    Stopped,
    Starting,
    Connected,
    Stopping,
    Crashed,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagerCapability {
    pub health: ManagerHealth,
    pub generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCapability {
    pub version: String,
    pub protocol_version: u32,
    pub backend: RuntimeBackend,
    pub availability: CapabilityAvailability,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EngineCapability {
    pub identity: Option<EngineIdentity>,
    pub availability: CapabilityAvailability,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityProfile {
    pub schema_version: u32,
    pub platform: Platform,
    pub architecture: Architecture,
    pub runtime: RuntimeCapability,
    pub engine: EngineCapability,
    pub manager: ManagerCapability,
}

impl CapabilityProfile {
    pub fn from_observation(
        configured_backend: RuntimeBackend,
        manager: ManagerCapability,
        native: Option<&NativeRuntimeCapabilities>,
    ) -> Self {
        let configured_identity = match configured_backend {
            RuntimeBackend::Mock => Some(EngineIdentity::AivsMockV1),
            RuntimeBackend::Unavailable => None,
        };
        let (platform, architecture, backend, runtime_availability, identity, engine_availability) =
            if manager.health != ManagerHealth::Connected {
                (
                    Platform::Unknown,
                    Architecture::Unknown,
                    configured_backend,
                    CapabilityAvailability::Unavailable,
                    configured_identity,
                    CapabilityAvailability::Unavailable,
                )
            } else if let Some(native) = native {
                match native.backend {
                    RuntimeBackend::Mock => (
                        native.platform,
                        native.architecture,
                        RuntimeBackend::Mock,
                        CapabilityAvailability::Available,
                        Some(EngineIdentity::AivsMockV1),
                        CapabilityAvailability::Available,
                    ),
                    RuntimeBackend::Unavailable => (
                        native.platform,
                        native.architecture,
                        RuntimeBackend::Unavailable,
                        CapabilityAvailability::Unavailable,
                        None,
                        CapabilityAvailability::Unavailable,
                    ),
                }
            } else {
                (
                    Platform::Unknown,
                    Architecture::Unknown,
                    configured_backend,
                    CapabilityAvailability::NotEvaluated,
                    configured_identity,
                    CapabilityAvailability::NotEvaluated,
                )
            };

        Self {
            schema_version: CAPABILITY_SCHEMA_VERSION,
            platform,
            architecture,
            runtime: RuntimeCapability {
                version: RUNTIME_VERSION.to_owned(),
                protocol_version: IPC_PROTOCOL_CURRENT_VERSION,
                backend,
                availability: runtime_availability,
            },
            engine: EngineCapability {
                identity,
                availability: engine_availability,
            },
            manager,
        }
    }
}
