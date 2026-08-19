//! Stable, privacy-minimal product capability contract.

use std::fmt;

use ai_voice_contracts::{IPC_PROTOCOL_CURRENT_VERSION, RUNTIME_VERSION};
use serde::{Deserialize, Serialize};

pub const CAPABILITY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Macos,
    Windows,
    Linux,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    Arm64,
    X86_64,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBackend {
    Mock,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum EngineIdentity {
    #[serde(rename = "aivs-mock-v1")]
    AivsMockV1,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawNativeRuntimeCapabilities {
    platform: String,
    architecture: String,
    runtime_version: String,
    protocol_version: u32,
    backend: String,
    engine: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRuntimeCapabilities {
    platform: Platform,
    architecture: Architecture,
    backend: RuntimeBackend,
    engine_identity: Option<EngineIdentity>,
}

impl NativeRuntimeCapabilities {
    pub fn from_canonical_json(bytes: &[u8]) -> Result<Self, NativeCapabilityError> {
        let raw: RawNativeRuntimeCapabilities =
            serde_json::from_slice(bytes).map_err(|_| NativeCapabilityError::MalformedShape)?;
        let canonical =
            serde_json::to_vec(&raw).map_err(|_| NativeCapabilityError::MalformedShape)?;
        if canonical != bytes {
            return Err(NativeCapabilityError::NonCanonicalJson);
        }
        if raw.runtime_version != RUNTIME_VERSION {
            return Err(NativeCapabilityError::Mismatch(
                NativeCapabilityMismatch::RuntimeVersion,
            ));
        }
        if raw.protocol_version != IPC_PROTOCOL_CURRENT_VERSION {
            return Err(NativeCapabilityError::Mismatch(
                NativeCapabilityMismatch::ProtocolVersion,
            ));
        }
        let platform = match raw.platform.as_str() {
            "macos" => Platform::Macos,
            "windows" => Platform::Windows,
            "linux" => Platform::Linux,
            "unknown" => Platform::Unknown,
            _ => {
                return Err(NativeCapabilityError::Mismatch(
                    NativeCapabilityMismatch::Platform,
                ));
            }
        };
        let architecture = match raw.architecture.as_str() {
            "arm64" => Architecture::Arm64,
            "x86_64" => Architecture::X86_64,
            "unknown" => Architecture::Unknown,
            _ => {
                return Err(NativeCapabilityError::Mismatch(
                    NativeCapabilityMismatch::Architecture,
                ));
            }
        };
        let (backend, engine_identity) = match (raw.backend.as_str(), raw.engine.as_str()) {
            ("mock", "aivs-mock-v1") => (RuntimeBackend::Mock, Some(EngineIdentity::AivsMockV1)),
            ("unavailable", "unavailable") => (RuntimeBackend::Unavailable, None),
            _ => {
                return Err(NativeCapabilityError::Mismatch(
                    NativeCapabilityMismatch::BackendEnginePair,
                ));
            }
        };
        Ok(Self {
            platform,
            architecture,
            backend,
            engine_identity,
        })
    }

    pub const fn platform(&self) -> Platform {
        self.platform
    }

    pub const fn architecture(&self) -> Architecture {
        self.architecture
    }

    pub const fn backend(&self) -> RuntimeBackend {
        self.backend
    }

    pub const fn engine_identity(&self) -> Option<EngineIdentity> {
        self.engine_identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCapabilityMismatch {
    RuntimeVersion,
    ProtocolVersion,
    Platform,
    Architecture,
    BackendEnginePair,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCapabilityError {
    MalformedShape,
    NonCanonicalJson,
    Mismatch(NativeCapabilityMismatch),
}

impl fmt::Display for NativeCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedShape => formatter.write_str("native capability JSON shape is invalid"),
            Self::NonCanonicalJson => {
                formatter.write_str("native capability JSON is not in canonical field order")
            }
            Self::Mismatch(reason) => write!(formatter, "native capability mismatch: {reason}"),
        }
    }
}

impl std::error::Error for NativeCapabilityError {}

impl fmt::Display for NativeCapabilityMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RuntimeVersion => "runtime_version",
            Self::ProtocolVersion => "protocol_version",
            Self::Platform => "platform",
            Self::Architecture => "architecture",
            Self::BackendEnginePair => "backend/engine pair",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAvailability {
    Available,
    Unavailable,
    Unknown,
    NotEvaluated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagerHealth {
    Stopped,
    Starting,
    Connected,
    Stopping,
    Crashed,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ManagerCapability {
    health: ManagerHealth,
    generation: u64,
}

impl ManagerCapability {
    pub fn new(health: ManagerHealth, generation: u64) -> Result<Self, CapabilityProfileError> {
        if health != ManagerHealth::Stopped && generation == 0 {
            return Err(CapabilityProfileError::InvalidManagerGeneration);
        }
        Ok(Self { health, generation })
    }

    pub const fn health(&self) -> ManagerHealth {
        self.health
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedRuntimeCapabilities {
    generation: u64,
    capabilities: NativeRuntimeCapabilities,
}

impl ObservedRuntimeCapabilities {
    pub fn new(
        generation: u64,
        capabilities: NativeRuntimeCapabilities,
    ) -> Result<Self, CapabilityProfileError> {
        if generation == 0 {
            return Err(CapabilityProfileError::InvalidObservationGeneration);
        }
        Ok(Self {
            generation,
            capabilities,
        })
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn capabilities(&self) -> &NativeRuntimeCapabilities {
        &self.capabilities
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ObservationKind {
    NotEvaluated,
    Inconclusive,
    Observed(NativeRuntimeCapabilities),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityObservation {
    generation: u64,
    kind: ObservationKind,
}

impl CapabilityObservation {
    pub const fn not_evaluated(generation: u64) -> Self {
        Self {
            generation,
            kind: ObservationKind::NotEvaluated,
        }
    }

    pub const fn inconclusive(generation: u64) -> Self {
        Self {
            generation,
            kind: ObservationKind::Inconclusive,
        }
    }

    pub fn observed(observation: ObservedRuntimeCapabilities) -> Self {
        Self {
            generation: observation.generation,
            kind: ObservationKind::Observed(observation.capabilities),
        }
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityProfileError {
    InvalidManagerGeneration,
    InvalidObservationGeneration,
    StaleObservation {
        manager_generation: u64,
        observation_generation: u64,
    },
    ObservationNotAllowed {
        health: ManagerHealth,
    },
}

impl fmt::Display for CapabilityProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManagerGeneration => {
                formatter.write_str("non-stopped manager generation must be nonzero")
            }
            Self::InvalidObservationGeneration => {
                formatter.write_str("observed Runtime generation must be nonzero")
            }
            Self::StaleObservation {
                manager_generation,
                observation_generation,
            } => write!(
                formatter,
                "capability observation generation {observation_generation} does not match manager generation {manager_generation}"
            ),
            Self::ObservationNotAllowed { health } => {
                write!(
                    formatter,
                    "capability query observation is invalid while manager is {health:?}"
                )
            }
        }
    }
}

impl std::error::Error for CapabilityProfileError {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeCapability {
    version: String,
    protocol_version: u32,
    backend: RuntimeBackend,
    availability: CapabilityAvailability,
}

impl RuntimeCapability {
    pub fn version(&self) -> &str {
        &self.version
    }

    pub const fn protocol_version(&self) -> u32 {
        self.protocol_version
    }

    pub const fn backend(&self) -> RuntimeBackend {
        self.backend
    }

    pub const fn availability(&self) -> CapabilityAvailability {
        self.availability
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EngineCapability {
    identity: Option<EngineIdentity>,
    availability: CapabilityAvailability,
}

impl EngineCapability {
    pub const fn identity(&self) -> Option<EngineIdentity> {
        self.identity
    }

    pub const fn availability(&self) -> CapabilityAvailability {
        self.availability
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityProfile {
    schema_version: u32,
    platform: Platform,
    architecture: Architecture,
    runtime: RuntimeCapability,
    engine: EngineCapability,
    manager: ManagerCapability,
}

impl CapabilityProfile {
    pub fn from_observation(
        configured_backend: RuntimeBackend,
        manager: ManagerCapability,
        observation: &CapabilityObservation,
    ) -> Result<Self, CapabilityProfileError> {
        if manager.generation != observation.generation {
            return Err(CapabilityProfileError::StaleObservation {
                manager_generation: manager.generation,
                observation_generation: observation.generation,
            });
        }
        if manager.health != ManagerHealth::Connected
            && !matches!(observation.kind, ObservationKind::NotEvaluated)
        {
            return Err(CapabilityProfileError::ObservationNotAllowed {
                health: manager.health,
            });
        }

        let configured_identity = match configured_backend {
            RuntimeBackend::Mock => Some(EngineIdentity::AivsMockV1),
            RuntimeBackend::Unavailable => None,
        };
        let (platform, architecture, backend, runtime_availability, identity, engine_availability) =
            match manager.health {
                ManagerHealth::Starting => (
                    Platform::Unknown,
                    Architecture::Unknown,
                    configured_backend,
                    CapabilityAvailability::NotEvaluated,
                    configured_identity,
                    CapabilityAvailability::NotEvaluated,
                ),
                ManagerHealth::Connected => match &observation.kind {
                    ObservationKind::NotEvaluated => (
                        Platform::Unknown,
                        Architecture::Unknown,
                        configured_backend,
                        CapabilityAvailability::Available,
                        configured_identity,
                        CapabilityAvailability::NotEvaluated,
                    ),
                    ObservationKind::Inconclusive => (
                        Platform::Unknown,
                        Architecture::Unknown,
                        configured_backend,
                        CapabilityAvailability::Available,
                        configured_identity,
                        CapabilityAvailability::Unknown,
                    ),
                    ObservationKind::Observed(native) => (
                        native.platform,
                        native.architecture,
                        native.backend,
                        CapabilityAvailability::Available,
                        native.engine_identity,
                        if native.engine_identity.is_some() {
                            CapabilityAvailability::Available
                        } else {
                            CapabilityAvailability::Unavailable
                        },
                    ),
                },
                ManagerHealth::Stopped
                | ManagerHealth::Stopping
                | ManagerHealth::Crashed
                | ManagerHealth::Error => (
                    Platform::Unknown,
                    Architecture::Unknown,
                    configured_backend,
                    CapabilityAvailability::Unavailable,
                    configured_identity,
                    CapabilityAvailability::Unavailable,
                ),
            };

        Ok(Self {
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
        })
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn platform(&self) -> Platform {
        self.platform
    }

    pub const fn architecture(&self) -> Architecture {
        self.architecture
    }

    pub const fn runtime(&self) -> &RuntimeCapability {
        &self.runtime
    }

    pub const fn engine(&self) -> &EngineCapability {
        &self.engine
    }

    pub const fn manager(&self) -> &ManagerCapability {
        &self.manager
    }
}
