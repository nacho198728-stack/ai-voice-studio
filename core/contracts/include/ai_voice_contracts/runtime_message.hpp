#pragma once

#include <ai_voice_contracts/generated_contracts.hpp>
#include <ai_voice_contracts/runtime_message_generated.hpp>

#include <algorithm>
#include <bit>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <new>
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
  BatchTooLarge,
  AllocationFailure,
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

constexpr FrameError batch_too_large() {
  return {ErrorCode::FrameTooLarge, FrameErrorKind::BatchTooLarge};
}

constexpr FrameError allocation_failure() {
  return {ErrorCode::InternalError, FrameErrorKind::AllocationFailure};
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

constexpr std::int32_t decode_i32_bits(std::uint32_t bits) {
  return std::bit_cast<std::int32_t>(bits);
}

inline std::int32_t read_i32(std::span<const std::uint8_t> bytes, std::size_t offset) {
  return decode_i32_bits(read_u32(bytes, offset));
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

  const auto kind_policy = std::find_if(
      kPolicies.begin(), kPolicies.end(), [kind](const Policy& policy) { return policy.kind == kind; });
  const bool invalid_request_id =
      kind_policy->request_id_rule == RequestIdRule::Zero ? request_id != 0U : request_id == 0U;
  if (invalid_request_id) {
    return malformed(FrameErrorKind::RequestId);
  }
  const auto command_policy = std::find_if(
      kPolicies.begin(), kPolicies.end(), [kind, command](const Policy& policy) {
        return policy.kind == kind && policy.command == command;
      });
  if (command_policy == kPolicies.end()) {
    return malformed(FrameErrorKind::Command);
  }
  const auto error_rule =
      error_code == ErrorCode::Success ? ErrorRule::Success : ErrorRule::NonSuccess;
  const auto policy = std::find_if(
      command_policy, kPolicies.end(), [kind, command, error_rule](const Policy& candidate) {
        return candidate.kind == kind && candidate.command == command &&
               candidate.error_rule == error_rule;
      });
  if (policy == kPolicies.end()) {
    return malformed(FrameErrorKind::ErrorInvariant);
  }
  if (payload_length > policy->max_payload_bytes) {
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
    if (input.size() > kMaxInputBytesPerFeed) {
      result.error = fail(detail::batch_too_large());
      return result;
    }

    try {
      while (!input.empty()) {
        if (!header_.has_value()) {
          const auto needed = kHeaderSize - header_size_;
          const auto take = std::min(needed, input.size());
          std::copy_n(input.begin(), take, header_buffer_.begin() +
                                               static_cast<std::ptrdiff_t>(header_size_));
          header_size_ += take;
          input = input.subspan(take);
          if (header_size_ < kHeaderSize) {
            break;
          }
          const auto parsed = detail::parse_header(header_buffer_);
          if (parsed.error.has_value()) {
            std::vector<RuntimeMessage>().swap(result.messages);
            result.error = fail(*parsed.error);
            return result;
          }
          if (parsed.header->payload_length != 0U) {
            if (!frame_storage_) {
              frame_storage_ =
                  std::make_unique<std::array<std::uint8_t, kMaxFrameBytes>>();
              ++storage_allocation_count_;
            }
            std::copy(header_buffer_.begin(), header_buffer_.end(), frame_storage_->begin());
          }
          header_ = parsed.header;
        }

        const auto needed = header_->payload_length - body_size_;
        const auto take = std::min(needed, input.size());
        if (take != 0U) {
          std::copy_n(
              input.begin(),
              take,
              frame_storage_->begin() + static_cast<std::ptrdiff_t>(kHeaderSize + body_size_));
          body_size_ += take;
        }
        input = input.subspan(take);
        if (body_size_ < header_->payload_length) {
          break;
        }

        if (result.messages.size() >= kMaxMessagesPerFeed) {
          std::vector<RuntimeMessage>().swap(result.messages);
          result.error = fail(detail::batch_too_large());
          return result;
        }
        result.messages.push_back({
            header_->kind,
            header_->protocol_version,
            header_->request_id,
            header_->command,
            header_->error_code,
            header_->payload_length == 0U
                ? std::vector<std::uint8_t>{}
                : std::vector<std::uint8_t>(
                      frame_storage_->begin() + static_cast<std::ptrdiff_t>(kHeaderSize),
                      frame_storage_->begin() +
                          static_cast<std::ptrdiff_t>(kHeaderSize + header_->payload_length)),
        });
        header_size_ = 0U;
        body_size_ = 0U;
        header_.reset();
      }
      return result;
    } catch (const std::bad_alloc&) {
      std::vector<RuntimeMessage>().swap(result.messages);
      result.error = fail(detail::allocation_failure());
      return result;
    }
  }

  std::optional<FrameError> finish() {
    if (failure_.has_value()) {
      return failure_;
    }
    if (buffered_size() == 0U) {
      return std::nullopt;
    }
    return fail(detail::malformed(FrameErrorKind::Truncated));
  }

  void reset() {
    header_size_ = 0U;
    frame_storage_.reset();
    body_size_ = 0U;
    header_.reset();
    failure_.reset();
  }

  [[nodiscard]] std::size_t buffered_size() const { return header_size_ + body_size_; }
  [[nodiscard]] std::size_t allocated_storage_bytes() const {
    return frame_storage_ ? kMaxFrameBytes : 0U;
  }
  [[nodiscard]] std::size_t storage_allocation_count() const {
    return storage_allocation_count_;
  }
  [[nodiscard]] bool failed() const { return failure_.has_value(); }

 private:
  FrameError fail(FrameError error) {
    header_size_ = 0U;
    frame_storage_.reset();
    body_size_ = 0U;
    header_.reset();
    failure_ = error;
    return error;
  }

  std::array<std::uint8_t, kHeaderSize> header_buffer_{};
  std::size_t header_size_{0U};
  std::unique_ptr<std::array<std::uint8_t, kMaxFrameBytes>> frame_storage_;
  std::size_t body_size_{0U};
  std::optional<detail::ParsedHeader> header_;
  std::optional<FrameError> failure_;
  std::size_t storage_allocation_count_{0U};
};

}  // namespace ai_voice::contracts::runtime_message
