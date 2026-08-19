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
