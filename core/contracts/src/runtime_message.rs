//! Transport-neutral RuntimeMessage v1 framing.
//!
//! The codec uses explicit little-endian fields and retains no more than one
//! legal frame. It performs no process I/O or command dispatch.

pub use crate::runtime_message_generated::{
    Command, HEADER_SIZE, MAGIC, MAX_CONTROL_PAYLOAD_BYTES, MAX_ERROR_PAYLOAD_BYTES,
    MAX_FRAME_BYTES, MAX_HELLO_PAYLOAD_BYTES, MAX_PING_PAYLOAD_BYTES, MessageKind,
    RUNTIME_MESSAGE_SCHEMA_VERSION, WIRE_VERSION, command_from_value, message_kind_from_value,
};
use crate::{ErrorCode, error_code_from_value, is_ipc_protocol_compatible};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeMessage {
    pub kind: MessageKind,
    pub protocol_version: u32,
    pub request_id: u64,
    pub command: Command,
    pub error_code: ErrorCode,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameErrorKind {
    Magic,
    WireVersion,
    MessageKind,
    Flags,
    ProtocolVersion,
    RequestId,
    Command,
    Reserved,
    ErrorCode,
    ErrorInvariant,
    PayloadTooLarge,
    Truncated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameError {
    pub code: ErrorCode,
    pub kind: FrameErrorKind,
}

impl FrameError {
    const fn malformed(kind: FrameErrorKind) -> Self {
        Self {
            code: ErrorCode::MalformedFrame,
            kind,
        }
    }

    const fn unsupported(kind: FrameErrorKind) -> Self {
        Self {
            code: ErrorCode::UnsupportedProtocolVersion,
            kind,
        }
    }

    const fn too_large() -> Self {
        Self {
            code: ErrorCode::FrameTooLarge,
            kind: FrameErrorKind::PayloadTooLarge,
        }
    }
}

#[derive(Clone, Copy)]
struct ParsedHeader {
    kind: MessageKind,
    protocol_version: u32,
    request_id: u64,
    command: Command,
    error_code: ErrorCode,
    payload_length: usize,
}

#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
    header: Option<ParsedHeader>,
    failure: Option<FrameError>,
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            buffer: Vec::new(),
            header: None,
            failure: None,
        }
    }

    pub fn feed(&mut self, mut input: &[u8]) -> Result<Vec<RuntimeMessage>, FrameError> {
        if let Some(error) = self.failure {
            return Err(error);
        }

        let mut messages = Vec::new();
        while !input.is_empty() {
            if self.header.is_none() {
                let needed = HEADER_SIZE - self.buffer.len();
                let take = needed.min(input.len());
                self.buffer.extend_from_slice(&input[..take]);
                input = &input[take..];
                if self.buffer.len() < HEADER_SIZE {
                    break;
                }
                match parse_header(&self.buffer) {
                    Ok(header) => self.header = Some(header),
                    Err(error) => return Err(self.fail(error)),
                }
            }

            let header = self.header.expect("validated header is present");
            let frame_size = HEADER_SIZE + header.payload_length;
            let needed = frame_size - self.buffer.len();
            let take = needed.min(input.len());
            self.buffer.extend_from_slice(&input[..take]);
            input = &input[take..];
            if self.buffer.len() < frame_size {
                break;
            }

            messages.push(RuntimeMessage {
                kind: header.kind,
                protocol_version: header.protocol_version,
                request_id: header.request_id,
                command: header.command,
                error_code: header.error_code,
                payload: self.buffer[HEADER_SIZE..frame_size].to_vec(),
            });
            self.buffer.clear();
            self.header = None;
        }
        Ok(messages)
    }

    pub fn finish(&mut self) -> Result<(), FrameError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if self.buffer.is_empty() {
            return Ok(());
        }
        Err(self.fail(FrameError::malformed(FrameErrorKind::Truncated)))
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.header = None;
        self.failure = None;
    }

    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_failed(&self) -> bool {
        self.failure.is_some()
    }

    fn fail(&mut self, error: FrameError) -> FrameError {
        self.buffer.clear();
        self.header = None;
        self.failure = Some(error);
        error
    }
}

pub fn encode(message: &RuntimeMessage) -> Result<Vec<u8>, FrameError> {
    validate_message(
        message.kind,
        message.protocol_version,
        message.request_id,
        message.command,
        message.error_code,
        message.payload.len(),
    )?;

    let payload_length =
        u32::try_from(message.payload.len()).map_err(|_| FrameError::too_large())?;
    let mut frame = Vec::with_capacity(HEADER_SIZE + message.payload.len());
    frame.extend_from_slice(&MAGIC);
    frame.extend_from_slice(&WIRE_VERSION.to_le_bytes());
    frame.push(message.kind as u8);
    frame.push(0);
    frame.extend_from_slice(&message.protocol_version.to_le_bytes());
    frame.extend_from_slice(&message.request_id.to_le_bytes());
    frame.extend_from_slice(&(message.command as u16).to_le_bytes());
    frame.extend_from_slice(&0_u16.to_le_bytes());
    frame.extend_from_slice(&message.error_code.value().to_le_bytes());
    frame.extend_from_slice(&payload_length.to_le_bytes());
    frame.extend_from_slice(&message.payload);
    Ok(frame)
}

fn parse_header(header: &[u8]) -> Result<ParsedHeader, FrameError> {
    debug_assert_eq!(header.len(), HEADER_SIZE);
    if header[..4] != MAGIC {
        return Err(FrameError::malformed(FrameErrorKind::Magic));
    }
    if read_u16(header, 4) != WIRE_VERSION {
        return Err(FrameError::unsupported(FrameErrorKind::WireVersion));
    }
    let kind = message_kind_from_value(header[6])
        .ok_or_else(|| FrameError::malformed(FrameErrorKind::MessageKind))?;
    if header[7] != 0 {
        return Err(FrameError::malformed(FrameErrorKind::Flags));
    }
    let protocol_version = read_u32(header, 8);
    if !is_ipc_protocol_compatible(protocol_version) {
        return Err(FrameError::unsupported(FrameErrorKind::ProtocolVersion));
    }
    let request_id = read_u64(header, 12);
    let command = command_from_value(read_u16(header, 20))
        .ok_or_else(|| FrameError::malformed(FrameErrorKind::Command))?;
    if read_u16(header, 22) != 0 {
        return Err(FrameError::malformed(FrameErrorKind::Reserved));
    }
    let error_value = read_i32(header, 24);
    let error_code = error_code_from_value(error_value)
        .ok_or_else(|| FrameError::malformed(FrameErrorKind::ErrorCode))?;
    let payload_length = read_u32(header, 28) as usize;
    validate_message(
        kind,
        protocol_version,
        request_id,
        command,
        error_code,
        payload_length,
    )?;
    Ok(ParsedHeader {
        kind,
        protocol_version,
        request_id,
        command,
        error_code,
        payload_length,
    })
}

fn validate_message(
    kind: MessageKind,
    protocol_version: u32,
    request_id: u64,
    command: Command,
    error_code: ErrorCode,
    payload_length: usize,
) -> Result<(), FrameError> {
    if !is_ipc_protocol_compatible(protocol_version) {
        return Err(FrameError::unsupported(FrameErrorKind::ProtocolVersion));
    }
    if payload_length > MAX_CONTROL_PAYLOAD_BYTES {
        return Err(FrameError::too_large());
    }

    match kind {
        MessageKind::Hello => {
            if request_id != 0 {
                return Err(FrameError::malformed(FrameErrorKind::RequestId));
            }
            if command != Command::None {
                return Err(FrameError::malformed(FrameErrorKind::Command));
            }
            if error_code != ErrorCode::Success {
                return Err(FrameError::malformed(FrameErrorKind::ErrorInvariant));
            }
        }
        MessageKind::Request | MessageKind::Response => {
            if request_id == 0 {
                return Err(FrameError::malformed(FrameErrorKind::RequestId));
            }
            if command == Command::None {
                return Err(FrameError::malformed(FrameErrorKind::Command));
            }
            if kind == MessageKind::Request && error_code != ErrorCode::Success {
                return Err(FrameError::malformed(FrameErrorKind::ErrorInvariant));
            }
        }
    }

    let limit = if kind == MessageKind::Response && error_code != ErrorCode::Success {
        MAX_ERROR_PAYLOAD_BYTES
    } else {
        match command {
            Command::None => MAX_HELLO_PAYLOAD_BYTES,
            Command::Ping => MAX_PING_PAYLOAD_BYTES,
            Command::GetCapabilities if kind == MessageKind::Request => 0,
            Command::GetCapabilities | Command::RunMockPipeline => MAX_CONTROL_PAYLOAD_BYTES,
            Command::Shutdown => 0,
        }
    };
    if payload_length > limit {
        return Err(FrameError::too_large());
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("fixed header field"),
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed header field"),
    )
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed header field"),
    )
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("fixed header field"),
    )
}
