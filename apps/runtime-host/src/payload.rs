use ai_voice_contracts::{IPC_PROTOCOL_CURRENT_VERSION, RUNTIME_VERSION};
use serde::{Deserialize, Serialize};

const MOCK_SUMMARY_BYTES: usize = 80;
const MOCK_SUMMARY_SCHEMA_VERSION: u32 = 1;
const MOCK_FRAME_COUNT: u32 = 128;
const MOCK_CHANNEL_COUNT: u32 = 2;
const MOCK_CHECKSUM: u64 = 0x3ecd_5190_f6f4_f725;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub runtime_version: String,
    pub protocol_version: u32,
    pub generation: u64,
    pub health: String,
}

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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCapabilities {
    pub platform: Platform,
    pub architecture: Architecture,
    pub runtime_version: String,
    pub protocol_version: u32,
    pub backend: String,
    pub engine: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MockPipelineSummary {
    pub schema_version: u32,
    pub result_size: u32,
    pub frames: u32,
    pub channels: u32,
    pub checksum: u64,
    pub elapsed_microseconds: u64,
    pub algorithmic_latency_frames: u64,
    pub stream_generation: u64,
    pub process_call_count: u64,
    pub input_frame_count: u64,
    pub output_frame_count: u64,
    pub process_error_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PayloadError {
    Malformed,
    ContractMismatch,
}

pub(crate) fn parse_hello(payload: &[u8]) -> Result<Hello, PayloadError> {
    let hello: Hello = parse_exact_json(payload)?;
    if hello.runtime_version != RUNTIME_VERSION
        || hello.protocol_version != IPC_PROTOCOL_CURRENT_VERSION
        || hello.generation != 1
        || hello.health != "starting"
    {
        return Err(PayloadError::ContractMismatch);
    }
    Ok(hello)
}

pub(crate) fn parse_capabilities(payload: &[u8]) -> Result<RuntimeCapabilities, PayloadError> {
    let capabilities: RuntimeCapabilities = parse_exact_json(payload)?;
    if capabilities.runtime_version != RUNTIME_VERSION
        || capabilities.protocol_version != IPC_PROTOCOL_CURRENT_VERSION
        || capabilities.backend != "mock"
        || capabilities.engine != "aivs-mock-v1"
    {
        return Err(PayloadError::ContractMismatch);
    }
    Ok(capabilities)
}

fn parse_exact_json<T>(payload: &[u8]) -> Result<T, PayloadError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    let parsed = serde_json::from_slice(payload).map_err(|_| PayloadError::Malformed)?;
    let canonical = serde_json::to_vec(&parsed).map_err(|_| PayloadError::Malformed)?;
    if canonical != payload {
        return Err(PayloadError::Malformed);
    }
    Ok(parsed)
}

pub(crate) fn parse_mock_pipeline_summary(
    payload: &[u8],
) -> Result<MockPipelineSummary, PayloadError> {
    if payload.len() != MOCK_SUMMARY_BYTES {
        return Err(PayloadError::Malformed);
    }
    let summary = MockPipelineSummary {
        schema_version: read_u32(payload, 0),
        result_size: read_u32(payload, 4),
        frames: read_u32(payload, 8),
        channels: read_u32(payload, 12),
        checksum: read_u64(payload, 16),
        elapsed_microseconds: read_u64(payload, 24),
        algorithmic_latency_frames: read_u64(payload, 32),
        stream_generation: read_u64(payload, 40),
        process_call_count: read_u64(payload, 48),
        input_frame_count: read_u64(payload, 56),
        output_frame_count: read_u64(payload, 64),
        process_error_count: read_u64(payload, 72),
    };
    let expected_frames = summary
        .process_call_count
        .checked_mul(u64::from(MOCK_FRAME_COUNT))
        .ok_or(PayloadError::ContractMismatch)?;
    if summary.schema_version != MOCK_SUMMARY_SCHEMA_VERSION
        || summary.result_size as usize != MOCK_SUMMARY_BYTES
        || summary.frames != MOCK_FRAME_COUNT
        || summary.channels != MOCK_CHANNEL_COUNT
        || summary.checksum != MOCK_CHECKSUM
        || summary.algorithmic_latency_frames != 0
        || summary.stream_generation == 0
        || summary.process_call_count == 0
        || summary.input_frame_count != expected_frames
        || summary.output_frame_count != expected_frames
        || summary.process_error_count != 0
    {
        return Err(PayloadError::ContractMismatch);
    }
    Ok(summary)
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed summary"))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("fixed summary"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_parser_accepts_only_the_canonical_healthy_startup_contract() {
        let hello = parse_hello(
            br#"{"runtime_version":"0.0.0","protocol_version":1,"generation":1,"health":"starting"}"#,
        )
        .expect("canonical Hello must parse");

        assert_eq!(hello.runtime_version, "0.0.0");
        assert_eq!(hello.protocol_version, 1);
        assert_eq!(hello.generation, 1);
        assert_eq!(hello.health, "starting");

        for malformed in [
            br#"{"runtime_version":"9.0.0","protocol_version":1,"generation":1,"health":"starting"}"#.as_slice(),
            br#"{"runtime_version":"0.0.0","protocol_version":2,"generation":1,"health":"starting"}"#,
            br#"{"runtime_version":"0.0.0","protocol_version":1,"generation":0,"health":"starting"}"#,
            br#"{"runtime_version":"0.0.0","protocol_version":1,"generation":1,"health":"healthy"}"#,
            br#"{"runtime_version":"0.0.0","protocol_version":1,"generation":1,"health":"starting","extra":true}"#,
            br#"{"protocol_version":1,"runtime_version":"0.0.0","generation":1,"health":"starting"}"#,
            br#"{"runtime_version":"0.0.0","protocol_version":1,"generation":1,"health":"starting"}\n"#,
        ] {
            assert!(parse_hello(malformed).is_err(), "accepted {malformed:?}");
        }
    }

    #[test]
    fn capabilities_parser_validates_the_fixed_mock_identity() {
        let capabilities = parse_capabilities(
            br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#,
        )
        .expect("canonical capabilities must parse");

        assert_eq!(capabilities.platform, Platform::Macos);
        assert_eq!(capabilities.architecture, Architecture::Arm64);
        assert_eq!(capabilities.backend, "mock");
        assert_eq!(capabilities.engine, "aivs-mock-v1");

        for malformed in [
            br#"{"platform":"ios","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}"#.as_slice(),
            br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"unavailable","engine":"unavailable"}"#,
            br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"other"}"#,
            br#"{"platform":"macos","architecture":"arm64","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"}x"#,
        ] {
            assert!(parse_capabilities(malformed).is_err(), "accepted {malformed:?}");
        }
    }

    #[test]
    fn mock_summary_parser_checks_every_fixed_result_invariant() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&80_u32.to_le_bytes());
        bytes.extend_from_slice(&128_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&0x3ecd_5190_f6f4_f725_u64.to_le_bytes());
        bytes.extend_from_slice(&17_u64.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&3_u64.to_le_bytes());
        bytes.extend_from_slice(&384_u64.to_le_bytes());
        bytes.extend_from_slice(&384_u64.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());

        let summary = parse_mock_pipeline_summary(&bytes).expect("fixed summary must parse");
        assert_eq!(summary.frames, 128);
        assert_eq!(summary.channels, 2);
        assert_eq!(summary.checksum, 0x3ecd_5190_f6f4_f725);
        assert_eq!(summary.elapsed_microseconds, 17);
        assert_eq!(summary.process_call_count, 3);

        let mutations = [
            (0, 2_u32.to_le_bytes().to_vec()),
            (4, 79_u32.to_le_bytes().to_vec()),
            (8, 127_u32.to_le_bytes().to_vec()),
            (12, 1_u32.to_le_bytes().to_vec()),
            (16, 0_u64.to_le_bytes().to_vec()),
            (32, 1_u64.to_le_bytes().to_vec()),
            (40, 0_u64.to_le_bytes().to_vec()),
            (48, 0_u64.to_le_bytes().to_vec()),
            (56, 383_u64.to_le_bytes().to_vec()),
            (64, 383_u64.to_le_bytes().to_vec()),
            (72, 1_u64.to_le_bytes().to_vec()),
        ];
        for (offset, value) in mutations {
            let mut malformed = bytes.clone();
            malformed[offset..offset + value.len()].copy_from_slice(&value);
            assert!(
                parse_mock_pipeline_summary(&malformed).is_err(),
                "accepted mutation at {offset}"
            );
        }
        assert!(parse_mock_pipeline_summary(&bytes[..79]).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(parse_mock_pipeline_summary(&trailing).is_err());
    }
}
