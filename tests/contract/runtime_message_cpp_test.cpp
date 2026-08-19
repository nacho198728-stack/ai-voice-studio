#ifdef NDEBUG
#undef NDEBUG
#endif
#include <cassert>
#include <cstdlib>
#include <cstddef>
#include <cstdint>
#include <fstream>
#include <iterator>
#include <limits>
#include <new>
#include <span>
#include <sstream>
#include <string>
#include <vector>

#include <ai_voice_contracts/runtime_message.hpp>

namespace allocation_fault {

thread_local std::size_t countdown = std::numeric_limits<std::size_t>::max();

void fail_after(std::size_t successful_allocations) {
  countdown = successful_allocations;
}

void disable() {
  countdown = std::numeric_limits<std::size_t>::max();
}

}  // namespace allocation_fault

void* operator new(std::size_t size) {
  if (allocation_fault::countdown != std::numeric_limits<std::size_t>::max()) {
    if (allocation_fault::countdown == 0U) {
      allocation_fault::disable();
      throw std::bad_alloc();
    }
    --allocation_fault::countdown;
  }
  if (void* pointer = std::malloc(size == 0U ? 1U : size)) {
    return pointer;
  }
  throw std::bad_alloc();
}

void* operator new[](std::size_t size) {
  return ::operator new(size);
}

void operator delete(void* pointer) noexcept {
  std::free(pointer);
}

void operator delete[](void* pointer) noexcept {
  std::free(pointer);
}

void operator delete(void* pointer, std::size_t) noexcept {
  std::free(pointer);
}

void operator delete[](void* pointer, std::size_t) noexcept {
  std::free(pointer);
}

namespace message = ai_voice::contracts::runtime_message;
using ai_voice::contracts::ErrorCode;

static_assert(message::detail::decode_i32_bits(UINT32_C(0x80000000)) ==
              std::numeric_limits<std::int32_t>::min());

namespace {

std::vector<std::uint8_t> fixture_named(const std::string& name) {
  std::ifstream input(std::string(AIVS_RUNTIME_MESSAGE_FIXTURE_DIRECTORY) + "/" + name);
  assert(input.good());
  std::vector<std::uint8_t> bytes;
  std::string token;
  while (input >> token) {
    bytes.push_back(static_cast<std::uint8_t>(std::stoul(token, nullptr, 16)));
  }
  return bytes;
}

std::vector<std::uint8_t> fixture() {
  return fixture_named("runtime-message-v1-ping-request.hex");
}

message::RuntimeMessage request(message::Command command, std::vector<std::uint8_t> payload = {}) {
  return {
      message::MessageKind::Request,
      1U,
      UINT64_C(0x0102030405060708),
      command,
      ErrorCode::Success,
      std::move(payload),
  };
}

void shared_fixture_and_round_trip() {
  const auto bytes = fixture();
  message::Decoder decoder;
  const auto decoded = decoder.feed(bytes);
  assert(!decoded.error.has_value());
  assert(decoded.messages.size() == 1U);
  assert(decoded.messages[0] == request(message::Command::Ping, {'p', 'i', 'n', 'g'}));
  const auto encoded = message::encode(decoded.messages[0]);
  assert(!encoded.error.has_value());
  assert(encoded.bytes == bytes);
  assert(decoder.buffered_size() == 0U);

  const std::vector<message::RuntimeMessage> round_trips{
      {message::MessageKind::Hello,
       1U,
       0U,
       message::Command::None,
       ErrorCode::Success,
       {'r', 'u', 'n', 't', 'i', 'm', 'e'}},
      {message::MessageKind::Response,
       1U,
       7U,
       message::Command::RunMockPipeline,
       ErrorCode::Success,
       {'f', 'r', 'a', 'm', 'e', 's'}},
      {message::MessageKind::Response,
       1U,
       8U,
       message::Command::Ping,
       ErrorCode::RuntimeUnavailable,
       {'e', 'r', 'r', 'o', 'r'}},
  };
  for (const auto& expected : round_trips) {
    const auto frame = message::encode(expected);
    assert(!frame.error.has_value());
    message::Decoder round_trip_decoder;
    const auto round_trip = round_trip_decoder.feed(frame.bytes);
    assert(!round_trip.error.has_value());
    assert(round_trip.messages == std::vector<message::RuntimeMessage>{expected});
  }
}

void all_literal_kinds_and_commands() {
  const std::vector<std::pair<std::string, message::RuntimeMessage>> cases{
      {"runtime-message-v1-hello.hex",
       {message::MessageKind::Hello, 1U, 0U, message::Command::None, ErrorCode::Success, {}}},
      {"runtime-message-v1-get-capabilities-request.hex",
       {message::MessageKind::Request,
        1U,
        1U,
        message::Command::GetCapabilities,
        ErrorCode::Success,
        {}}},
      {"runtime-message-v1-run-mock-pipeline-request.hex",
       {message::MessageKind::Request,
        1U,
        2U,
        message::Command::RunMockPipeline,
        ErrorCode::Success,
        {'r', 'u', 'n'}}},
      {"runtime-message-v1-shutdown-request.hex",
       {message::MessageKind::Request,
        1U,
        3U,
        message::Command::Shutdown,
        ErrorCode::Success,
        {}}},
      {"runtime-message-v1-ping-error-response.hex",
       {message::MessageKind::Response,
        1U,
        4U,
        message::Command::Ping,
        ErrorCode::RuntimeUnavailable,
        std::vector<std::uint8_t>(257U)}},
  };
  for (const auto& [name, expected] : cases) {
    const auto bytes = fixture_named(name);
    message::Decoder decoder;
    assert(decoder.feed(bytes).messages == std::vector<message::RuntimeMessage>{expected});
    assert(message::encode(expected).bytes == bytes);
  }
}

void chunking_sticky_and_eof() {
  const auto bytes = fixture();
  message::Decoder bytewise;
  std::vector<message::RuntimeMessage> messages;
  for (const auto byte : bytes) {
    const auto result = bytewise.feed(std::span<const std::uint8_t>(&byte, 1U));
    assert(!result.error.has_value());
    messages.insert(messages.end(), result.messages.begin(), result.messages.end());
    assert(bytewise.buffered_size() <= message::kHeaderSize + message::kMaxControlPayloadBytes);
  }
  assert(messages.size() == 1U);
  assert(!bytewise.finish().has_value());

  message::Decoder sticky;
  auto combined = bytes;
  combined.insert(combined.end(), bytes.begin(), bytes.end());
  assert(sticky.feed(combined).messages.size() == 2U);

  for (const auto end : std::vector<std::size_t>{
           1U, message::kHeaderSize - 1U, message::kHeaderSize, bytes.size() - 1U}) {
    message::Decoder truncated;
    const auto partial = truncated.feed(std::span<const std::uint8_t>(bytes.data(), end));
    assert(!partial.error.has_value());
    const auto error = truncated.finish();
    assert(error->code == ErrorCode::MalformedFrame);
    assert(error->kind == message::FrameErrorKind::Truncated);
  }
}

void bounded_feed_resources() {
  const auto hello = fixture_named("runtime-message-v1-hello.hex");
  std::vector<std::uint8_t> many;
  for (std::size_t index = 0; index < message::kMaxMessagesPerFeed; ++index) {
    many.insert(many.end(), hello.begin(), hello.end());
  }
  message::Decoder decoder;
  assert(decoder.feed(many).messages.size() == message::kMaxMessagesPerFeed);

  many.insert(many.end(), hello.begin(), hello.end());
  message::Decoder too_many_decoder;
  const auto too_many = too_many_decoder.feed(many);
  assert(too_many.error->code == ErrorCode::FrameTooLarge);
  assert(too_many.error->kind == message::FrameErrorKind::BatchTooLarge);
  assert(too_many.messages.capacity() == 0U);
  assert(too_many_decoder.buffered_capacity() == 0U);

  message::Decoder oversized_decoder;
  const auto oversized = oversized_decoder.feed(
      std::vector<std::uint8_t>(message::kMaxInputBytesPerFeed + 1U));
  assert(oversized.error->kind == message::FrameErrorKind::BatchTooLarge);
  assert(oversized.messages.capacity() == 0U);
  assert(oversized_decoder.buffered_capacity() == 0U);

  std::vector<std::uint8_t> valid_then_fatal;
  for (std::size_t index = 0; index < 8U; ++index) {
    valid_then_fatal.insert(valid_then_fatal.end(), hello.begin(), hello.end());
  }
  auto malformed = hello;
  malformed[0] = 0U;
  valid_then_fatal.insert(valid_then_fatal.end(), malformed.begin(), malformed.end());
  message::Decoder fatal_decoder;
  const auto fatal = fatal_decoder.feed(valid_then_fatal);
  assert(fatal.error->kind == message::FrameErrorKind::Magic);
  assert(fatal.messages.capacity() == 0U);
  assert(fatal_decoder.buffered_capacity() == 0U);
}

void allocation_failures_are_terminal_and_release_resources() {
  const auto hello = fixture_named("runtime-message-v1-hello.hex");
  auto body_frame = message::encode(
      request(message::Command::RunMockPipeline, std::vector<std::uint8_t>(128U)))
                        .bytes;

  const auto exercise = [](message::Decoder& decoder,
                           std::span<const std::uint8_t> input,
                           std::size_t successful_allocations) {
    allocation_fault::fail_after(successful_allocations);
    const auto result = decoder.feed(input);
    allocation_fault::disable();
    assert(result.error->code == ErrorCode::InternalError);
    assert(result.error->kind == message::FrameErrorKind::AllocationFailure);
    assert(result.messages.capacity() == 0U);
    assert(decoder.buffered_capacity() == 0U);
    assert(decoder.failed());
    assert(decoder.feed({}).error == result.error);
  };

  message::Decoder header_growth;
  exercise(header_growth, std::span<const std::uint8_t>(hello.data(), 1U), 0U);

  message::Decoder body_growth;
  exercise(body_growth, body_frame, 1U);

  message::Decoder payload_materialization;
  exercise(payload_materialization, body_frame, 2U);

  message::Decoder result_growth;
  exercise(result_growth, hello, 1U);

  result_growth.reset();
  assert(result_growth.feed(hello).messages.size() == 1U);
}

void malformed_and_failed_state() {
  struct Case {
    std::size_t offset;
    std::vector<std::uint8_t> replacement;
    ErrorCode code;
    message::FrameErrorKind kind;
  };
  const std::vector<Case> cases{
      {0U, {0U}, ErrorCode::MalformedFrame, message::FrameErrorKind::Magic},
      {4U, {2U, 0U}, ErrorCode::UnsupportedProtocolVersion, message::FrameErrorKind::WireVersion},
      {6U, {99U}, ErrorCode::MalformedFrame, message::FrameErrorKind::MessageKind},
      {7U, {1U}, ErrorCode::MalformedFrame, message::FrameErrorKind::Flags},
      {8U, {2U, 0U, 0U, 0U}, ErrorCode::UnsupportedProtocolVersion, message::FrameErrorKind::ProtocolVersion},
      {12U, std::vector<std::uint8_t>(8U), ErrorCode::MalformedFrame, message::FrameErrorKind::RequestId},
      {20U, {99U, 0U}, ErrorCode::MalformedFrame, message::FrameErrorKind::Command},
      {22U, {1U, 0U}, ErrorCode::MalformedFrame, message::FrameErrorKind::Reserved},
      {24U, {0xb0U, 4U, 0U, 0U}, ErrorCode::MalformedFrame, message::FrameErrorKind::ErrorInvariant},
  };
  for (const auto& test_case : cases) {
    auto bytes = fixture();
    std::copy(test_case.replacement.begin(), test_case.replacement.end(), bytes.begin() + test_case.offset);
    message::Decoder decoder;
    const auto result = decoder.feed(bytes);
    assert(result.error->code == test_case.code);
    assert(result.error->kind == test_case.kind);
  }

  auto unknown_error = fixture();
  unknown_error[6] = static_cast<std::uint8_t>(message::MessageKind::Response);
  unknown_error[24] = 42U;
  unknown_error[25] = 0U;
  unknown_error[26] = 0U;
  unknown_error[27] = 0U;
  message::Decoder unknown_error_decoder;
  assert(unknown_error_decoder.feed(unknown_error).error->kind == message::FrameErrorKind::ErrorCode);

  auto high_bit_error = fixture();
  high_bit_error[6] = static_cast<std::uint8_t>(message::MessageKind::Response);
  high_bit_error[24] = 0U;
  high_bit_error[25] = 0U;
  high_bit_error[26] = 0U;
  high_bit_error[27] = 0x80U;
  message::Decoder high_bit_decoder;
  const auto high_bit_result = high_bit_decoder.feed(high_bit_error);
  assert(high_bit_result.error->code == ErrorCode::MalformedFrame);
  assert(high_bit_result.error->kind == message::FrameErrorKind::ErrorCode);

  auto valid_then_bad = fixture();
  valid_then_bad.insert(valid_then_bad.end(), unknown_error.begin(), unknown_error.end());
  message::Decoder atomic_decoder;
  const auto atomic_result = atomic_decoder.feed(valid_then_bad);
  assert(atomic_result.error->kind == message::FrameErrorKind::ErrorCode);
  assert(atomic_result.messages.empty());

  auto oversized = fixture();
  oversized.resize(message::kHeaderSize);
  const std::uint32_t size = static_cast<std::uint32_t>(message::kMaxControlPayloadBytes + 1U);
  oversized[28] = static_cast<std::uint8_t>(size);
  oversized[29] = static_cast<std::uint8_t>(size >> 8U);
  oversized[30] = static_cast<std::uint8_t>(size >> 16U);
  oversized[31] = static_cast<std::uint8_t>(size >> 24U);
  message::Decoder decoder;
  const auto error = decoder.feed(oversized).error;
  assert(error->code == ErrorCode::FrameTooLarge);
  assert(error->kind == message::FrameErrorKind::PayloadTooLarge);
  assert(decoder.buffered_size() == 0U);
  assert(decoder.failed());
  const auto repeated = decoder.feed(fixture()).error;
  assert(repeated->code == ErrorCode::FrameTooLarge);
  assert(repeated->kind == message::FrameErrorKind::PayloadTooLarge);
  decoder.reset();
  assert(decoder.feed(fixture()).messages.size() == 1U);
}

void encode_limits_and_invariants() {
  const auto maximum = request(message::Command::RunMockPipeline,
                               std::vector<std::uint8_t>(message::kMaxControlPayloadBytes));
  const auto maximum_frame = message::encode(maximum);
  assert(!maximum_frame.error.has_value());
  assert(maximum_frame.bytes.size() == message::kMaxFrameBytes);
  message::Decoder maximum_decoder;
  assert(maximum_decoder.feed(maximum_frame.bytes).messages ==
         std::vector<message::RuntimeMessage>{maximum});
  assert(!message::encode(request(message::Command::Ping,
                                  std::vector<std::uint8_t>(message::kMaxPingPayloadBytes)))
              .error.has_value());
  assert(message::encode(request(message::Command::Ping, std::vector<std::uint8_t>(257U))).error->code ==
         ErrorCode::FrameTooLarge);
  assert(message::encode(request(message::Command::RunMockPipeline,
                                 std::vector<std::uint8_t>(message::kMaxControlPayloadBytes + 1U)))
             .error->code == ErrorCode::FrameTooLarge);
  assert(message::encode(request(message::Command::Shutdown, {1U})).error->kind ==
         message::FrameErrorKind::PayloadTooLarge);
  assert(message::encode(request(message::Command::GetCapabilities, {1U})).error->kind ==
         message::FrameErrorKind::PayloadTooLarge);

  message::RuntimeMessage valid_hello{
      message::MessageKind::Hello,
      1U,
      0U,
      message::Command::None,
      ErrorCode::Success,
      std::vector<std::uint8_t>(message::kMaxHelloPayloadBytes),
  };
  assert(!message::encode(valid_hello).error.has_value());
  valid_hello.payload.push_back(0U);
  assert(message::encode(valid_hello).error->code == ErrorCode::FrameTooLarge);

  message::RuntimeMessage valid_error{
      message::MessageKind::Response,
      1U,
      9U,
      message::Command::Ping,
      ErrorCode::RuntimeUnavailable,
      std::vector<std::uint8_t>(message::kMaxErrorPayloadBytes),
  };
  assert(!message::encode(valid_error).error.has_value());
  valid_error.payload.push_back(0U);
  assert(message::encode(valid_error).error->code == ErrorCode::FrameTooLarge);

  auto invalid = request(message::Command::Ping);
  invalid.request_id = 0U;
  assert(message::encode(invalid).error->kind == message::FrameErrorKind::RequestId);
  message::RuntimeMessage invalid_hello{
      message::MessageKind::Hello,
      1U,
      1U,
      message::Command::None,
      ErrorCode::Success,
      {},
  };
  assert(message::encode(invalid_hello).error->kind == message::FrameErrorKind::RequestId);
}

}  // namespace

int main() {
  shared_fixture_and_round_trip();
  all_literal_kinds_and_commands();
  chunking_sticky_and_eof();
  bounded_feed_resources();
  allocation_failures_are_terminal_and_release_resources();
  malformed_and_failed_state();
  encode_limits_and_invariants();
  return 0;
}
