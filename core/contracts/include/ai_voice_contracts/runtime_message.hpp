#pragma once

#include <ai_voice_contracts/generated_contracts.hpp>
#include <ai_voice_contracts/runtime_message_generated.hpp>

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>
#include <utility>
#include <vector>

namespace ai_voice::contracts::runtime_message {

struct RuntimeMessage {
  MessageKind kind;
  std::uint32_t protocol_version;
  std::uint64_t request_id;
  Command command;
  ErrorCode error_code;
  std::vector<std::uint8_t> payload;

  bool operator==(const RuntimeMessage&) const = default;
};

enum class FrameErrorKind {
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
};

struct FrameError {
  ErrorCode code;
  FrameErrorKind kind;

  bool operator==(const FrameError&) const = default;
};

struct EncodeResult {
  std::vector<std::uint8_t> bytes;
  std::optional<FrameError> error;
};

struct DecodeResult {
  std::vector<RuntimeMessage> messages;
  std::optional<FrameError> error;
};

namespace detail {

constexpr FrameError malformed(FrameErrorKind kind) {
  return {ErrorCode::MalformedFrame, kind};
}

constexpr FrameError unsupported(FrameErrorKind kind) {
  return {ErrorCode::UnsupportedProtocolVersion, kind};
}

constexpr FrameError too_large() {
  return {ErrorCode::FrameTooLarge, FrameErrorKind::PayloadTooLarge};
}

inline std::uint16_t read_u16(std::span<const std::uint8_t> bytes, std::size_t offset) {
  return static_cast<std::uint16_t>(bytes[offset]) |
         (static_cast<std::uint16_t>(bytes[offset + 1U]) << 8U);
}

inline std::uint32_t read_u32(std::span<const std::uint8_t> bytes, std::size_t offset) {
  std::uint32_t value = 0U;
  for (std::size_t index = 0; index < 4U; ++index) {
    value |= static_cast<std::uint32_t>(bytes[offset + index]) << (index * 8U);
  }
  return value;
}

inline std::int32_t read_i32(std::span<const std::uint8_t> bytes, std::size_t offset) {
  return static_cast<std::int32_t>(read_u32(bytes, offset));
}

inline std::uint64_t read_u64(std::span<const std::uint8_t> bytes, std::size_t offset) {
  std::uint64_t value = 0U;
  for (std::size_t index = 0; index < 8U; ++index) {
    value |= static_cast<std::uint64_t>(bytes[offset + index]) << (index * 8U);
  }
  return value;
}

inline void write_u16(std::vector<std::uint8_t>& bytes, std::uint16_t value) {
  bytes.push_back(static_cast<std::uint8_t>(value));
  bytes.push_back(static_cast<std::uint8_t>(value >> 8U));
}

inline void write_u32(std::vector<std::uint8_t>& bytes, std::uint32_t value) {
  for (std::size_t index = 0; index < 4U; ++index) {
    bytes.push_back(static_cast<std::uint8_t>(value >> (index * 8U)));
  }
}

inline void write_u64(std::vector<std::uint8_t>& bytes, std::uint64_t value) {
  for (std::size_t index = 0; index < 8U; ++index) {
    bytes.push_back(static_cast<std::uint8_t>(value >> (index * 8U)));
  }
}

inline std::optional<FrameError> validate_message(
    MessageKind kind,
    std::uint32_t protocol_version,
    std::uint64_t request_id,
    Command command,
    ErrorCode error_code,
    std::size_t payload_length) {
  if (!message_kind_from_value(static_cast<std::uint8_t>(kind)).has_value()) {
    return malformed(FrameErrorKind::MessageKind);
  }
  if (!command_from_value(static_cast<std::uint16_t>(command)).has_value()) {
    return malformed(FrameErrorKind::Command);
  }
  if (!error_code_from_value(error_code_value(error_code)).has_value()) {
    return malformed(FrameErrorKind::ErrorCode);
  }
  if (!is_ipc_protocol_compatible(protocol_version)) {
    return unsupported(FrameErrorKind::ProtocolVersion);
  }
  if (payload_length > kMaxControlPayloadBytes) {
    return too_large();
  }

  if (kind == MessageKind::Hello) {
    if (request_id != 0U) {
      return malformed(FrameErrorKind::RequestId);
    }
    if (command != Command::None) {
      return malformed(FrameErrorKind::Command);
    }
    if (error_code != ErrorCode::Success) {
      return malformed(FrameErrorKind::ErrorInvariant);
    }
  } else {
    if (request_id == 0U) {
      return malformed(FrameErrorKind::RequestId);
    }
    if (command == Command::None) {
      return malformed(FrameErrorKind::Command);
    }
    if (kind == MessageKind::Request && error_code != ErrorCode::Success) {
      return malformed(FrameErrorKind::ErrorInvariant);
    }
  }

  std::size_t limit = kMaxControlPayloadBytes;
  if (kind == MessageKind::Response && error_code != ErrorCode::Success) {
    limit = kMaxErrorPayloadBytes;
  } else {
    switch (command) {
      case Command::None:
        limit = kMaxHelloPayloadBytes;
        break;
      case Command::Ping:
        limit = kMaxPingPayloadBytes;
        break;
      case Command::GetCapabilities:
        limit = kind == MessageKind::Request ? 0U : kMaxControlPayloadBytes;
        break;
      case Command::RunMockPipeline:
        limit = kMaxControlPayloadBytes;
        break;
      case Command::Shutdown:
        limit = 0U;
        break;
    }
  }
  if (payload_length > limit) {
    return too_large();
  }
  return std::nullopt;
}

struct ParsedHeader {
  MessageKind kind;
  std::uint32_t protocol_version;
  std::uint64_t request_id;
  Command command;
  ErrorCode error_code;
  std::size_t payload_length;
};

struct HeaderResult {
  std::optional<ParsedHeader> header;
  std::optional<FrameError> error;
};

inline HeaderResult parse_header(std::span<const std::uint8_t> bytes) {
  if (!std::equal(kMagic.begin(), kMagic.end(), bytes.begin())) {
    return {{}, malformed(FrameErrorKind::Magic)};
  }
  if (read_u16(bytes, 4U) != kWireVersion) {
    return {{}, unsupported(FrameErrorKind::WireVersion)};
  }
  const auto kind = message_kind_from_value(bytes[6]);
  if (!kind.has_value()) {
    return {{}, malformed(FrameErrorKind::MessageKind)};
  }
  if (bytes[7] != 0U) {
    return {{}, malformed(FrameErrorKind::Flags)};
  }
  const auto protocol_version = read_u32(bytes, 8U);
  if (!is_ipc_protocol_compatible(protocol_version)) {
    return {{}, unsupported(FrameErrorKind::ProtocolVersion)};
  }
  const auto command = command_from_value(read_u16(bytes, 20U));
  if (!command.has_value()) {
    return {{}, malformed(FrameErrorKind::Command)};
  }
  if (read_u16(bytes, 22U) != 0U) {
    return {{}, malformed(FrameErrorKind::Reserved)};
  }
  const auto error_code = error_code_from_value(read_i32(bytes, 24U));
  if (!error_code.has_value()) {
    return {{}, malformed(FrameErrorKind::ErrorCode)};
  }
  const auto request_id = read_u64(bytes, 12U);
  const auto payload_length = static_cast<std::size_t>(read_u32(bytes, 28U));
  const auto error = validate_message(
      *kind, protocol_version, request_id, *command, *error_code, payload_length);
  if (error.has_value()) {
    return {{}, error};
  }
  return {{ParsedHeader{*kind, protocol_version, request_id, *command, *error_code, payload_length}}, {}};
}

}  // namespace detail

inline EncodeResult encode(const RuntimeMessage& message) {
  const auto error = detail::validate_message(
      message.kind,
      message.protocol_version,
      message.request_id,
      message.command,
      message.error_code,
      message.payload.size());
  if (error.has_value()) {
    return {{}, error};
  }

  std::vector<std::uint8_t> frame;
  frame.reserve(kHeaderSize + message.payload.size());
  frame.insert(frame.end(), kMagic.begin(), kMagic.end());
  detail::write_u16(frame, kWireVersion);
  frame.push_back(static_cast<std::uint8_t>(message.kind));
  frame.push_back(0U);
  detail::write_u32(frame, message.protocol_version);
  detail::write_u64(frame, message.request_id);
  detail::write_u16(frame, static_cast<std::uint16_t>(message.command));
  detail::write_u16(frame, 0U);
  detail::write_u32(frame, static_cast<std::uint32_t>(error_code_value(message.error_code)));
  detail::write_u32(frame, static_cast<std::uint32_t>(message.payload.size()));
  frame.insert(frame.end(), message.payload.begin(), message.payload.end());
  return {std::move(frame), {}};
}

class Decoder {
 public:
  DecodeResult feed(std::span<const std::uint8_t> input) {
    DecodeResult result;
    if (failure_.has_value()) {
      result.error = failure_;
      return result;
    }

    while (!input.empty()) {
      if (!header_.has_value()) {
        const auto needed = kHeaderSize - buffer_.size();
        const auto take = std::min(needed, input.size());
        buffer_.insert(buffer_.end(), input.begin(), input.begin() + static_cast<std::ptrdiff_t>(take));
        input = input.subspan(take);
        if (buffer_.size() < kHeaderSize) {
          break;
        }
        const auto parsed = detail::parse_header(buffer_);
        if (parsed.error.has_value()) {
          result.messages.clear();
          result.error = fail(*parsed.error);
          return result;
        }
        header_ = parsed.header;
      }

      const auto frame_size = kHeaderSize + header_->payload_length;
      const auto needed = frame_size - buffer_.size();
      const auto take = std::min(needed, input.size());
      buffer_.insert(buffer_.end(), input.begin(), input.begin() + static_cast<std::ptrdiff_t>(take));
      input = input.subspan(take);
      if (buffer_.size() < frame_size) {
        break;
      }

      result.messages.push_back({
          header_->kind,
          header_->protocol_version,
          header_->request_id,
          header_->command,
          header_->error_code,
          std::vector<std::uint8_t>(buffer_.begin() + static_cast<std::ptrdiff_t>(kHeaderSize),
                                    buffer_.end()),
      });
      buffer_.clear();
      header_.reset();
    }
    return result;
  }

  std::optional<FrameError> finish() {
    if (failure_.has_value()) {
      return failure_;
    }
    if (buffer_.empty()) {
      return std::nullopt;
    }
    return fail(detail::malformed(FrameErrorKind::Truncated));
  }

  void reset() {
    buffer_.clear();
    header_.reset();
    failure_.reset();
  }

  [[nodiscard]] std::size_t buffered_size() const { return buffer_.size(); }
  [[nodiscard]] bool failed() const { return failure_.has_value(); }

 private:
  FrameError fail(FrameError error) {
    buffer_.clear();
    header_.reset();
    failure_ = error;
    return error;
  }

  std::vector<std::uint8_t> buffer_;
  std::optional<detail::ParsedHeader> header_;
  std::optional<FrameError> failure_;
};

}  // namespace ai_voice::contracts::runtime_message
