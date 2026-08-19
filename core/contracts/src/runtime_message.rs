//! Transport-neutral RuntimeMessage v1 framing.
//!
//! The codec uses explicit little-endian fields and retains no more than one
//! legal frame. It performs no process I/O or command dispatch.

pub use crate::runtime_message_generated::{
    Command, ErrorRule, HEADER_SIZE, MAGIC, MAX_CONTROL_PAYLOAD_BYTES, MAX_ERROR_PAYLOAD_BYTES,
    MAX_FRAME_BYTES, MAX_HELLO_PAYLOAD_BYTES, MAX_INPUT_BYTES_PER_FEED, MAX_MESSAGES_PER_FEED,
    MAX_PING_PAYLOAD_BYTES, MessageKind, POLICIES, Policy, RUNTIME_MESSAGE_SCHEMA_VERSION,
    RequestIdRule, WIRE_VERSION, command_from_value, message_kind_from_value,
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
    BatchTooLarge,
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

    const fn batch_too_large() -> Self {
        Self {
            code: ErrorCode::FrameTooLarge,
            kind: FrameErrorKind::BatchTooLarge,
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
    header_bytes: [u8; HEADER_SIZE],
    header_len: usize,
    frame_storage: Option<Box<[u8; MAX_FRAME_BYTES]>>,
    body_len: usize,
    header: Option<ParsedHeader>,
    failure: Option<FrameError>,
    storage_allocation_count: usize,
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            header_bytes: [0; HEADER_SIZE],
            header_len: 0,
            frame_storage: None,
            body_len: 0,
            header: None,
            failure: None,
            storage_allocation_count: 0,
        }
    }

    pub fn feed(&mut self, mut input: &[u8]) -> Result<Vec<RuntimeMessage>, FrameError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if input.len() > MAX_INPUT_BYTES_PER_FEED {
            return Err(self.fail(FrameError::batch_too_large()));
        }

        let mut messages = Vec::new();
        while !input.is_empty() {
            if self.header.is_none() {
                let needed = HEADER_SIZE - self.header_len;
                let take = needed.min(input.len());
                self.header_bytes[self.header_len..self.header_len + take]
                    .copy_from_slice(&input[..take]);
                self.header_len += take;
                input = &input[take..];
                if self.header_len < HEADER_SIZE {
                    break;
                }
                match parse_header(&self.header_bytes) {
                    Ok(header) => {
                        if header.payload_length != 0 {
                            if self.frame_storage.is_none() {
                                self.frame_storage = Some(new_frame_storage());
                                self.storage_allocation_count += 1;
                            }
                            self.frame_storage
                                .as_mut()
                                .expect("non-empty frame has fixed storage")[..HEADER_SIZE]
                                .copy_from_slice(&self.header_bytes);
                        }
                        self.header = Some(header);
                    }
                    Err(error) => return Err(self.fail(error)),
                }
            }

            let header = self.header.expect("validated header is present");
            let needed = header.payload_length - self.body_len;
            let take = needed.min(input.len());
            if take != 0 {
                let body_start = HEADER_SIZE + self.body_len;
                self.frame_storage
                    .as_mut()
                    .expect("non-empty frame has fixed storage")[body_start..body_start + take]
                    .copy_from_slice(&input[..take]);
                self.body_len += take;
            }
            input = &input[take..];
            if self.body_len < header.payload_length {
                break;
            }

            if messages.len() >= MAX_MESSAGES_PER_FEED {
                return Err(self.fail(FrameError::batch_too_large()));
            }
            messages.push(RuntimeMessage {
                kind: header.kind,
                protocol_version: header.protocol_version,
                request_id: header.request_id,
                command: header.command,
                error_code: header.error_code,
                payload: if header.payload_length == 0 {
                    Vec::new()
                } else {
                    self.frame_storage
                        .as_ref()
                        .expect("non-empty frame has fixed storage")
                        [HEADER_SIZE..HEADER_SIZE + header.payload_length]
                        .to_vec()
                },
            });
            self.header_len = 0;
            self.body_len = 0;
            self.header = None;
        }
        Ok(messages)
    }

    pub fn finish(&mut self) -> Result<(), FrameError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if self.buffered_len() == 0 {
            return Ok(());
        }
        Err(self.fail(FrameError::malformed(FrameErrorKind::Truncated)))
    }

    pub fn reset(&mut self) {
        self.header_len = 0;
        self.frame_storage = None;
        self.body_len = 0;
        self.header = None;
        self.failure = None;
    }

    pub fn buffered_len(&self) -> usize {
        self.header_len + self.body_len
    }

    pub fn allocated_storage_bytes(&self) -> usize {
        self.frame_storage
            .as_ref()
            .map_or(0, |storage| storage.len())
    }

    pub fn storage_allocation_count(&self) -> usize {
        self.storage_allocation_count
    }

    pub fn is_failed(&self) -> bool {
        self.failure.is_some()
    }

    fn fail(&mut self, error: FrameError) -> FrameError {
        self.header_len = 0;
        self.frame_storage = None;
        self.body_len = 0;
        self.header = None;
        self.failure = Some(error);
        error
    }
}

fn new_frame_storage() -> Box<[u8; MAX_FRAME_BYTES]> {
    let storage = vec![0; MAX_FRAME_BYTES].into_boxed_slice();
    match storage.try_into() {
        Ok(storage) => storage,
        Err(_) => unreachable!("fixed frame storage has the generated maximum length"),
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

    let kind_policy = POLICIES
        .iter()
        .find(|policy| policy.kind == kind)
        .expect("every generated message kind has a policy");
    let invalid_request_id = match kind_policy.request_id_rule {
        RequestIdRule::Zero => request_id != 0,
        RequestIdRule::NonZero => request_id == 0,
    };
    if invalid_request_id {
        return Err(FrameError::malformed(FrameErrorKind::RequestId));
    }
    if !POLICIES
        .iter()
        .any(|policy| policy.kind == kind && policy.command == command)
    {
        return Err(FrameError::malformed(FrameErrorKind::Command));
    }
    let error_rule = if error_code == ErrorCode::Success {
        ErrorRule::Success
    } else {
        ErrorRule::NonSuccess
    };
    let policy = POLICIES
        .iter()
        .find(|policy| {
            policy.kind == kind && policy.command == command && policy.error_rule == error_rule
        })
        .ok_or_else(|| FrameError::malformed(FrameErrorKind::ErrorInvariant))?;
    if payload_length > policy.max_payload_bytes {
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
