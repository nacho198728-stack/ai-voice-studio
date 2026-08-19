use std::{fs, path::PathBuf};

use ai_voice_contracts::{
    ErrorCategory, ErrorCode, IPC_PROTOCOL_CURRENT_VERSION, error_code_category,
    error_code_from_value,
    runtime_message::{
        Command, Decoder, ErrorRule, FrameError, FrameErrorKind, HEADER_SIZE, MAGIC,
        MAX_CONTROL_PAYLOAD_BYTES, MAX_ERROR_PAYLOAD_BYTES, MAX_FRAME_BYTES,
        MAX_HELLO_PAYLOAD_BYTES, MAX_INPUT_BYTES_PER_FEED, MAX_MESSAGES_PER_FEED,
        MAX_PING_PAYLOAD_BYTES, MessageKind, POLICIES, Policy, RUNTIME_MESSAGE_SCHEMA_VERSION,
        RequestIdRule, RuntimeMessage, WIRE_VERSION, encode,
    },
};

const EXPECTED_POLICIES: [Policy; 13] = [
    Policy {
        kind: MessageKind::Hello,
        command: Command::None,
        request_id_rule: RequestIdRule::Zero,
        error_rule: ErrorRule::Success,
        max_payload_bytes: 1024,
    },
    Policy {
        kind: MessageKind::Request,
        command: Command::Ping,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::Success,
        max_payload_bytes: 256,
    },
    Policy {
        kind: MessageKind::Request,
        command: Command::GetCapabilities,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::Success,
        max_payload_bytes: 0,
    },
    Policy {
        kind: MessageKind::Request,
        command: Command::RunMockPipeline,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::Success,
        max_payload_bytes: 65_536,
    },
    Policy {
        kind: MessageKind::Request,
        command: Command::Shutdown,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::Success,
        max_payload_bytes: 0,
    },
    Policy {
        kind: MessageKind::Response,
        command: Command::Ping,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::Success,
        max_payload_bytes: 256,
    },
    Policy {
        kind: MessageKind::Response,
        command: Command::GetCapabilities,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::Success,
        max_payload_bytes: 65_536,
    },
    Policy {
        kind: MessageKind::Response,
        command: Command::RunMockPipeline,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::Success,
        max_payload_bytes: 65_536,
    },
    Policy {
        kind: MessageKind::Response,
        command: Command::Shutdown,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::Success,
        max_payload_bytes: 0,
    },
    Policy {
        kind: MessageKind::Response,
        command: Command::Ping,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::NonSuccess,
        max_payload_bytes: 4096,
    },
    Policy {
        kind: MessageKind::Response,
        command: Command::GetCapabilities,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::NonSuccess,
        max_payload_bytes: 4096,
    },
    Policy {
        kind: MessageKind::Response,
        command: Command::RunMockPipeline,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::NonSuccess,
        max_payload_bytes: 4096,
    },
    Policy {
        kind: MessageKind::Response,
        command: Command::Shutdown,
        request_id_rule: RequestIdRule::NonZero,
        error_rule: ErrorRule::NonSuccess,
        max_payload_bytes: 4096,
    },
];

const CANONICAL_ERROR_CODES: [(ErrorCode, i32, ErrorCategory); 12] = [
    (ErrorCode::Success, 0, ErrorCategory::Success),
    (
        ErrorCode::UnsupportedProtocolVersion,
        1000,
        ErrorCategory::Version,
    ),
    (
        ErrorCode::UnsupportedVoiceEngineAbi,
        1001,
        ErrorCategory::Version,
    ),
    (ErrorCode::MalformedFrame, 1100, ErrorCategory::Framing),
    (ErrorCode::FrameTooLarge, 1101, ErrorCategory::Framing),
    (ErrorCode::RuntimeUnavailable, 1200, ErrorCategory::Runtime),
    (ErrorCode::RuntimeShuttingDown, 1201, ErrorCategory::Runtime),
    (ErrorCode::EngineUnavailable, 1300, ErrorCategory::Engine),
    (ErrorCode::InvalidArgument, 1301, ErrorCategory::Engine),
    (ErrorCode::InvalidState, 1302, ErrorCategory::Engine),
    (ErrorCode::BufferTooSmall, 1303, ErrorCategory::Engine),
    (ErrorCode::InternalError, 1900, ErrorCategory::Internal),
];

fn fixture_named(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    fs::read_to_string(path)
        .unwrap()
        .split_ascii_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

fn fixture() -> Vec<u8> {
    fixture_named("runtime-message-v1-ping-request.hex")
}

fn request(command: Command, payload: Vec<u8>) -> RuntimeMessage {
    RuntimeMessage {
        kind: MessageKind::Request,
        protocol_version: IPC_PROTOCOL_CURRENT_VERSION,
        request_id: 0x0102_0304_0506_0708,
        command,
        error_code: ErrorCode::Success,
        payload,
    }
}

fn message_for_policy(policy: Policy, payload_len: usize) -> RuntimeMessage {
    RuntimeMessage {
        kind: policy.kind,
        protocol_version: 1,
        request_id: match policy.request_id_rule {
            RequestIdRule::Zero => 0,
            RequestIdRule::NonZero => 1,
        },
        command: policy.command,
        error_code: match policy.error_rule {
            ErrorRule::Success => ErrorCode::Success,
            ErrorRule::NonSuccess => ErrorCode::RuntimeUnavailable,
        },
        payload: vec![0xa5; payload_len],
    }
}

fn assert_terminal_until_reset(input: &[u8], expected: FrameError) {
    let mut decoder = Decoder::new();
    assert_eq!(decoder.feed(input).unwrap_err(), expected);
    assert!(decoder.is_failed());
    assert_eq!(decoder.buffered_len(), 0);
    assert_eq!(decoder.buffered_capacity(), 0);
    assert_eq!(decoder.feed(&fixture()).unwrap_err(), expected);
    assert_eq!(decoder.finish().unwrap_err(), expected);

    decoder.reset();
    assert_eq!(
        decoder.feed(&fixture()).unwrap(),
        vec![request(Command::Ping, b"ping".to_vec())]
    );
}

#[test]
fn pins_the_exact_v1_header_layout_limits_and_little_endian_bytes() {
    assert_eq!(RUNTIME_MESSAGE_SCHEMA_VERSION, 1);
    assert_eq!(MAGIC, *b"AVRM");
    assert_eq!(WIRE_VERSION, 1);
    assert_eq!(HEADER_SIZE, 32);
    assert_eq!(MAX_CONTROL_PAYLOAD_BYTES, 65_536);
    assert_eq!(MAX_FRAME_BYTES, 65_568);
    assert_eq!(MAX_INPUT_BYTES_PER_FEED, 65_568);
    assert_eq!(MAX_MESSAGES_PER_FEED, 64);

    let message = RuntimeMessage {
        kind: MessageKind::Response,
        protocol_version: 1,
        request_id: 0x0102_0304_0506_0708,
        command: Command::RunMockPipeline,
        error_code: ErrorCode::InvalidState,
        payload: vec![0xaa, 0xbb],
    };
    let expected = [
        0x41, 0x56, 0x52, 0x4d, // magic
        0x01, 0x00, // wire_version: u16
        0x03, // message_kind: Response
        0x00, // flags
        0x01, 0x00, 0x00, 0x00, // protocol_version: u32
        0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // request_id: u64
        0x03, 0x00, // command: RunMockPipeline
        0x00, 0x00, // reserved
        0x16, 0x05, 0x00, 0x00, // error_code: InvalidState i32 (1302)
        0x02, 0x00, 0x00, 0x00, // payload_length: u32
        0xaa, 0xbb,
    ];

    assert_eq!(encode(&message).unwrap(), expected);
    assert_eq!(Decoder::new().feed(&expected).unwrap(), vec![message]);
}

#[test]
fn pins_every_canonical_error_code_and_its_signed_wire_field() {
    for (code, value, category) in CANONICAL_ERROR_CODES {
        assert_eq!(code.value(), value);
        assert_eq!(error_code_from_value(value), Some(code));
        assert_eq!(error_code_category(code), category);

        let message = RuntimeMessage {
            kind: MessageKind::Response,
            protocol_version: 1,
            request_id: 7,
            command: Command::Ping,
            error_code: code,
            payload: Vec::new(),
        };
        let frame = encode(&message).unwrap();
        assert_eq!(&frame[24..28], &value.to_le_bytes());
        assert_eq!(Decoder::new().feed(&frame).unwrap(), vec![message]);
    }

    assert_eq!(error_code_from_value(-1), None);
    assert_eq!(error_code_from_value(i32::MIN), None);
    assert_eq!(error_code_from_value(42), None);
    assert_eq!(error_code_from_value(i32::MAX), None);
}

#[test]
fn pins_and_exercises_every_generated_policy_boundary() {
    assert_eq!(POLICIES, EXPECTED_POLICIES);

    for policy in EXPECTED_POLICIES {
        let maximum = message_for_policy(policy, policy.max_payload_bytes);
        let frame = encode(&maximum).unwrap();
        assert_eq!(Decoder::new().feed(&frame).unwrap(), vec![maximum]);

        let oversized = message_for_policy(policy, policy.max_payload_bytes + 1);
        assert_eq!(
            encode(&oversized).unwrap_err().kind,
            FrameErrorKind::PayloadTooLarge
        );

        let mut wrong_request_id = message_for_policy(policy, 0);
        wrong_request_id.request_id = match policy.request_id_rule {
            RequestIdRule::Zero => 1,
            RequestIdRule::NonZero => 0,
        };
        assert_eq!(
            encode(&wrong_request_id).unwrap_err().kind,
            FrameErrorKind::RequestId
        );
    }
}

#[test]
fn rejects_every_illegal_kind_command_and_error_rule_combination() {
    let kinds = [
        MessageKind::Hello,
        MessageKind::Request,
        MessageKind::Response,
    ];
    let commands = [
        Command::None,
        Command::Ping,
        Command::GetCapabilities,
        Command::RunMockPipeline,
        Command::Shutdown,
    ];
    let error_rules = [ErrorRule::Success, ErrorRule::NonSuccess];

    for kind in kinds {
        for command in commands {
            for error_rule in error_rules {
                let is_legal = EXPECTED_POLICIES.iter().any(|policy| {
                    policy.kind == kind
                        && policy.command == command
                        && policy.error_rule == error_rule
                });
                let message = RuntimeMessage {
                    kind,
                    protocol_version: 1,
                    request_id: if kind == MessageKind::Hello { 0 } else { 1 },
                    command,
                    error_code: match error_rule {
                        ErrorRule::Success => ErrorCode::Success,
                        ErrorRule::NonSuccess => ErrorCode::InternalError,
                    },
                    payload: Vec::new(),
                };
                assert_eq!(
                    encode(&message).is_ok(),
                    is_legal,
                    "kind={kind:?}, command={command:?}, error_rule={error_rule:?}"
                );
            }
        }
    }
}

#[test]
fn consumes_the_shared_literal_fixture_and_matches_the_encoder() {
    let bytes = fixture();
    let mut decoder = Decoder::new();
    let messages = decoder.feed(&bytes).unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], request(Command::Ping, b"ping".to_vec()));
    assert_eq!(encode(&messages[0]).unwrap(), bytes);
    assert_eq!(decoder.buffered_len(), 0);
    assert!(!decoder.is_failed());
}

#[test]
fn consumes_literal_fixtures_for_every_kind_and_command() {
    let cases = [
        (
            "runtime-message-v1-hello.hex",
            RuntimeMessage {
                kind: MessageKind::Hello,
                protocol_version: 1,
                request_id: 0,
                command: Command::None,
                error_code: ErrorCode::Success,
                payload: Vec::new(),
            },
        ),
        (
            "runtime-message-v1-get-capabilities-request.hex",
            RuntimeMessage {
                kind: MessageKind::Request,
                protocol_version: 1,
                request_id: 1,
                command: Command::GetCapabilities,
                error_code: ErrorCode::Success,
                payload: Vec::new(),
            },
        ),
        (
            "runtime-message-v1-run-mock-pipeline-request.hex",
            RuntimeMessage {
                kind: MessageKind::Request,
                protocol_version: 1,
                request_id: 2,
                command: Command::RunMockPipeline,
                error_code: ErrorCode::Success,
                payload: b"run".to_vec(),
            },
        ),
        (
            "runtime-message-v1-shutdown-request.hex",
            RuntimeMessage {
                kind: MessageKind::Request,
                protocol_version: 1,
                request_id: 3,
                command: Command::Shutdown,
                error_code: ErrorCode::Success,
                payload: Vec::new(),
            },
        ),
        (
            "runtime-message-v1-ping-error-response.hex",
            RuntimeMessage {
                kind: MessageKind::Response,
                protocol_version: 1,
                request_id: 4,
                command: Command::Ping,
                error_code: ErrorCode::RuntimeUnavailable,
                payload: vec![0; 257],
            },
        ),
    ];

    for (name, expected) in cases {
        let bytes = fixture_named(name);
        let mut decoder = Decoder::new();
        assert_eq!(decoder.feed(&bytes).unwrap(), vec![expected.clone()]);
        assert_eq!(encode(&expected).unwrap(), bytes);
    }
}

#[test]
fn round_trips_hello_requests_and_success_or_error_responses() {
    let cases = [
        RuntimeMessage {
            kind: MessageKind::Hello,
            protocol_version: 1,
            request_id: 0,
            command: Command::None,
            error_code: ErrorCode::Success,
            payload: b"runtime=0.0.0".to_vec(),
        },
        request(Command::GetCapabilities, Vec::new()),
        RuntimeMessage {
            kind: MessageKind::Response,
            protocol_version: 1,
            request_id: 7,
            command: Command::RunMockPipeline,
            error_code: ErrorCode::Success,
            payload: b"frames=480".to_vec(),
        },
        RuntimeMessage {
            kind: MessageKind::Response,
            protocol_version: 1,
            request_id: 8,
            command: Command::Ping,
            error_code: ErrorCode::RuntimeUnavailable,
            payload: b"unavailable".to_vec(),
        },
    ];

    for expected in cases {
        let bytes = encode(&expected).unwrap();
        let mut decoder = Decoder::new();
        assert_eq!(decoder.feed(&bytes).unwrap(), vec![expected]);
        assert_eq!(decoder.finish(), Ok(()));
    }
}

#[test]
fn accepts_byte_chunks_and_multiple_sticky_frames() {
    let first = fixture();
    let second = encode(&request(Command::RunMockPipeline, b"frames=16".to_vec())).unwrap();

    let mut bytewise = Decoder::new();
    let mut decoded = Vec::new();
    for byte in &first {
        decoded.extend(bytewise.feed(std::slice::from_ref(byte)).unwrap());
        assert!(bytewise.buffered_len() <= HEADER_SIZE + MAX_CONTROL_PAYLOAD_BYTES);
    }
    assert_eq!(decoded, vec![request(Command::Ping, b"ping".to_vec())]);

    let mut sticky = Decoder::new();
    let mut combined = first;
    combined.extend(second);
    assert_eq!(sticky.feed(&combined).unwrap().len(), 2);
    assert_eq!(sticky.finish(), Ok(()));
}

#[test]
fn decodes_minimum_and_maximum_frames_at_every_split_point() {
    let minimum_message = RuntimeMessage {
        kind: MessageKind::Hello,
        protocol_version: 1,
        request_id: 0,
        command: Command::None,
        error_code: ErrorCode::Success,
        payload: Vec::new(),
    };
    let maximum_message = request(
        Command::RunMockPipeline,
        vec![0xa5; MAX_CONTROL_PAYLOAD_BYTES],
    );
    let cases = [
        (encode(&minimum_message).unwrap(), minimum_message),
        (encode(&maximum_message).unwrap(), maximum_message),
    ];

    for (frame, expected) in cases {
        for split in 0..=frame.len() {
            let mut decoder = Decoder::new();
            let mut decoded = decoder.feed(&frame[..split]).unwrap();
            decoded.extend(decoder.feed(&frame[split..]).unwrap());
            assert_eq!(decoded, vec![expected.clone()], "split={split}");
            assert_eq!(decoder.finish(), Ok(()));
            assert!(decoder.buffered_capacity() <= MAX_FRAME_BYTES);
        }
    }
}

#[test]
fn bounds_retained_capacity_under_bytewise_maximum_frame_input() {
    let maximum = request(Command::RunMockPipeline, vec![0; MAX_CONTROL_PAYLOAD_BYTES]);
    let frame = encode(&maximum).unwrap();
    let mut decoder = Decoder::new();
    let mut decoded = Vec::new();

    for byte in &frame {
        decoded.extend(decoder.feed(std::slice::from_ref(byte)).unwrap());
        assert!(decoder.buffered_capacity() <= MAX_FRAME_BYTES);
    }

    assert_eq!(decoded, vec![maximum]);
    assert!(decoder.buffered_capacity() <= MAX_FRAME_BYTES);
}

#[test]
fn bounds_input_and_output_resources_per_feed() {
    let hello = fixture_named("runtime-message-v1-hello.hex");
    let many = hello.repeat(MAX_MESSAGES_PER_FEED);
    let mut decoder = Decoder::new();
    assert_eq!(decoder.feed(&many).unwrap().len(), MAX_MESSAGES_PER_FEED);

    let too_many = hello.repeat(MAX_MESSAGES_PER_FEED + 1);
    let mut decoder = Decoder::new();
    let error = decoder.feed(&too_many).unwrap_err();
    assert_eq!(error.code, ErrorCode::FrameTooLarge);
    assert_eq!(error.kind, FrameErrorKind::BatchTooLarge);
    assert_eq!(decoder.buffered_capacity(), 0);

    let mut decoder = Decoder::new();
    let oversized_input = vec![0; MAX_INPUT_BYTES_PER_FEED + 1];
    let error = decoder.feed(&oversized_input).unwrap_err();
    assert_eq!(error.kind, FrameErrorKind::BatchTooLarge);
    assert_eq!(decoder.buffered_capacity(), 0);

    let mut valid_then_fatal = hello.repeat(8);
    let mut malformed = hello;
    malformed[0] = 0;
    valid_then_fatal.extend(malformed);
    let mut decoder = Decoder::new();
    assert_eq!(
        decoder.feed(&valid_then_fatal).unwrap_err().kind,
        FrameErrorKind::Magic
    );
    assert_eq!(decoder.buffered_capacity(), 0);
}

#[test]
fn distinguishes_clean_and_truncated_eof() {
    let mut clean = Decoder::new();
    assert_eq!(clean.finish(), Ok(()));

    for end in [1, HEADER_SIZE - 1, HEADER_SIZE, fixture().len() - 1] {
        let mut decoder = Decoder::new();
        assert!(decoder.feed(&fixture()[..end]).unwrap().is_empty());
        let error = decoder.finish().unwrap_err();
        assert_eq!(error.code, ErrorCode::MalformedFrame);
        assert_eq!(error.kind, FrameErrorKind::Truncated);
        assert!(decoder.is_failed());
    }
}

#[test]
fn rejects_malformed_header_and_message_invariants() {
    let base = fixture();
    let malformed_cases: &[(usize, &[u8], ErrorCode, FrameErrorKind)] = &[
        (0, &[0], ErrorCode::MalformedFrame, FrameErrorKind::Magic),
        (
            4,
            &[2, 0],
            ErrorCode::UnsupportedProtocolVersion,
            FrameErrorKind::WireVersion,
        ),
        (
            6,
            &[99],
            ErrorCode::MalformedFrame,
            FrameErrorKind::MessageKind,
        ),
        (7, &[1], ErrorCode::MalformedFrame, FrameErrorKind::Flags),
        (
            8,
            &[2, 0, 0, 0],
            ErrorCode::UnsupportedProtocolVersion,
            FrameErrorKind::ProtocolVersion,
        ),
        (
            12,
            &[0, 0, 0, 0, 0, 0, 0, 0],
            ErrorCode::MalformedFrame,
            FrameErrorKind::RequestId,
        ),
        (
            20,
            &[99, 0],
            ErrorCode::MalformedFrame,
            FrameErrorKind::Command,
        ),
        (
            22,
            &[1, 0],
            ErrorCode::MalformedFrame,
            FrameErrorKind::Reserved,
        ),
        (
            24,
            &[0xb0, 4, 0, 0],
            ErrorCode::MalformedFrame,
            FrameErrorKind::ErrorInvariant,
        ),
    ];

    for (offset, replacement, code, kind) in malformed_cases {
        let mut bytes = base.clone();
        bytes[*offset..*offset + replacement.len()].copy_from_slice(replacement);
        let mut decoder = Decoder::new();
        let error = decoder.feed(&bytes).unwrap_err();
        assert_eq!(error.code, *code);
        assert_eq!(error.kind, *kind);
    }

    let mut unknown_error = base;
    unknown_error[6] = MessageKind::Response as u8;
    unknown_error[24..28].copy_from_slice(&42_i32.to_le_bytes());
    let error = Decoder::new().feed(&unknown_error).unwrap_err();
    assert_eq!(error.kind, FrameErrorKind::ErrorCode);

    let mut high_bit_error = fixture();
    high_bit_error[6] = MessageKind::Response as u8;
    high_bit_error[24..28].copy_from_slice(&0x8000_0000_u32.to_le_bytes());
    let error = Decoder::new().feed(&high_bit_error).unwrap_err();
    assert_eq!(error.code, ErrorCode::MalformedFrame);
    assert_eq!(error.kind, FrameErrorKind::ErrorCode);

    let mut valid_then_bad = fixture();
    valid_then_bad.extend_from_slice(&unknown_error);
    assert_eq!(
        Decoder::new().feed(&valid_then_bad).unwrap_err().kind,
        FrameErrorKind::ErrorCode
    );
}

#[test]
fn rejects_oversize_before_body_retention_and_requires_reset() {
    let mut header = fixture()[..HEADER_SIZE].to_vec();
    header[28..32].copy_from_slice(&((MAX_CONTROL_PAYLOAD_BYTES + 1) as u32).to_le_bytes());
    let mut decoder = Decoder::new();

    let error = decoder.feed(&header).unwrap_err();
    assert_eq!(error.code, ErrorCode::FrameTooLarge);
    assert_eq!(error.kind, FrameErrorKind::PayloadTooLarge);
    assert_eq!(decoder.buffered_len(), 0);
    assert!(decoder.is_failed());
    let repeated = decoder.feed(&fixture()).unwrap_err();
    assert_eq!(repeated.code, ErrorCode::FrameTooLarge);
    assert_eq!(repeated.kind, FrameErrorKind::PayloadTooLarge);

    decoder.reset();
    assert_eq!(decoder.feed(&fixture()).unwrap().len(), 1);
}

#[test]
fn every_fatal_class_is_sticky_until_reset_and_discards_batch_prefixes() {
    let mut bad_magic = fixture();
    bad_magic[0] = 0;
    let magic_error = FrameError {
        code: ErrorCode::MalformedFrame,
        kind: FrameErrorKind::Magic,
    };

    let mut bad_wire_version = fixture();
    bad_wire_version[4..6].copy_from_slice(&2_u16.to_le_bytes());
    let wire_error = FrameError {
        code: ErrorCode::UnsupportedProtocolVersion,
        kind: FrameErrorKind::WireVersion,
    };

    let mut bad_protocol_version = fixture();
    bad_protocol_version[8..12].copy_from_slice(&0_u32.to_le_bytes());
    let protocol_error = FrameError {
        code: ErrorCode::UnsupportedProtocolVersion,
        kind: FrameErrorKind::ProtocolVersion,
    };

    let mut oversized_declaration = fixture()[..HEADER_SIZE].to_vec();
    oversized_declaration[28..32]
        .copy_from_slice(&((MAX_CONTROL_PAYLOAD_BYTES + 1) as u32).to_le_bytes());
    let payload_error = FrameError {
        code: ErrorCode::FrameTooLarge,
        kind: FrameErrorKind::PayloadTooLarge,
    };

    for (input, expected) in [
        (bad_magic, magic_error),
        (bad_wire_version, wire_error),
        (bad_protocol_version, protocol_error),
        (oversized_declaration, payload_error),
    ] {
        assert_terminal_until_reset(&input, expected);

        let mut valid_then_invalid = fixture_named("runtime-message-v1-hello.hex");
        valid_then_invalid.extend(input);
        let mut decoder = Decoder::new();
        assert_eq!(decoder.feed(&valid_then_invalid).unwrap_err(), expected);
        assert!(decoder.is_failed());
        assert_eq!(decoder.buffered_capacity(), 0);
    }

    assert_terminal_until_reset(
        &vec![0; MAX_INPUT_BYTES_PER_FEED + 1],
        FrameError {
            code: ErrorCode::FrameTooLarge,
            kind: FrameErrorKind::BatchTooLarge,
        },
    );
    assert_terminal_until_reset(
        &fixture_named("runtime-message-v1-hello.hex").repeat(MAX_MESSAGES_PER_FEED + 1),
        FrameError {
            code: ErrorCode::FrameTooLarge,
            kind: FrameErrorKind::BatchTooLarge,
        },
    );
}

#[test]
fn enforces_command_limits_and_message_invariants_on_encode() {
    let maximum = request(Command::RunMockPipeline, vec![0; MAX_CONTROL_PAYLOAD_BYTES]);
    let maximum_frame = encode(&maximum).unwrap();
    assert_eq!(maximum_frame.len(), MAX_FRAME_BYTES);
    assert_eq!(Decoder::new().feed(&maximum_frame).unwrap(), vec![maximum]);
    assert!(encode(&request(Command::Ping, vec![0; MAX_PING_PAYLOAD_BYTES])).is_ok());
    assert_eq!(
        encode(&request(Command::Ping, vec![0; 257]))
            .unwrap_err()
            .code,
        ErrorCode::FrameTooLarge
    );
    assert_eq!(
        encode(&request(
            Command::RunMockPipeline,
            vec![0; MAX_CONTROL_PAYLOAD_BYTES + 1]
        ))
        .unwrap_err()
        .code,
        ErrorCode::FrameTooLarge
    );
    assert_eq!(
        encode(&request(Command::Shutdown, vec![0]))
            .unwrap_err()
            .kind,
        FrameErrorKind::PayloadTooLarge
    );
    assert_eq!(
        encode(&request(Command::GetCapabilities, vec![0]))
            .unwrap_err()
            .kind,
        FrameErrorKind::PayloadTooLarge
    );

    let valid_hello = RuntimeMessage {
        kind: MessageKind::Hello,
        protocol_version: 1,
        request_id: 0,
        command: Command::None,
        error_code: ErrorCode::Success,
        payload: vec![0; MAX_HELLO_PAYLOAD_BYTES],
    };
    assert!(encode(&valid_hello).is_ok());
    let mut oversized_hello = valid_hello;
    oversized_hello.payload.push(0);
    assert_eq!(
        encode(&oversized_hello).unwrap_err().code,
        ErrorCode::FrameTooLarge
    );

    let valid_error = RuntimeMessage {
        kind: MessageKind::Response,
        protocol_version: 1,
        request_id: 9,
        command: Command::Ping,
        error_code: ErrorCode::RuntimeUnavailable,
        payload: vec![0; MAX_ERROR_PAYLOAD_BYTES],
    };
    assert!(encode(&valid_error).is_ok());
    let mut oversized_error = valid_error;
    oversized_error.payload.push(0);
    assert_eq!(
        encode(&oversized_error).unwrap_err().code,
        ErrorCode::FrameTooLarge
    );

    let mut invalid = request(Command::Ping, Vec::new());
    invalid.request_id = 0;
    assert_eq!(
        encode(&invalid).unwrap_err().kind,
        FrameErrorKind::RequestId
    );
    invalid.request_id = 1;
    invalid.error_code = ErrorCode::InternalError;
    assert_eq!(
        encode(&invalid).unwrap_err().kind,
        FrameErrorKind::ErrorInvariant
    );

    let invalid_hello = RuntimeMessage {
        kind: MessageKind::Hello,
        protocol_version: 1,
        request_id: 1,
        command: Command::None,
        error_code: ErrorCode::Success,
        payload: Vec::new(),
    };
    assert_eq!(
        encode(&invalid_hello).unwrap_err().kind,
        FrameErrorKind::RequestId
    );

    for protocol_version in [0, 2] {
        let mut invalid_version = request(Command::Ping, Vec::new());
        invalid_version.protocol_version = protocol_version;
        let error = encode(&invalid_version).unwrap_err();
        assert_eq!(error.code, ErrorCode::UnsupportedProtocolVersion);
        assert_eq!(error.kind, FrameErrorKind::ProtocolVersion);
    }
}
