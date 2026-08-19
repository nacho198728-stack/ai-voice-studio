use std::{fs, path::PathBuf};

use ai_voice_contracts::{
    ErrorCode, IPC_PROTOCOL_CURRENT_VERSION,
    runtime_message::{
        Command, Decoder, FrameErrorKind, HEADER_SIZE, MAX_CONTROL_PAYLOAD_BYTES,
        MAX_ERROR_PAYLOAD_BYTES, MAX_FRAME_BYTES, MAX_HELLO_PAYLOAD_BYTES,
        MAX_INPUT_BYTES_PER_FEED, MAX_MESSAGES_PER_FEED, MAX_PING_PAYLOAD_BYTES, MessageKind,
        RuntimeMessage, encode,
    },
};

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
}
