#ifdef NDEBUG
#undef NDEBUG
#endif

#include <algorithm>
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <limits>
#include <new>
#include <sstream>
#include <span>
#include <string>
#include <utility>
#include <vector>

#include <ai_voice_contracts/generated_contracts.hpp>
#include <ai_voice_contracts/runtime_message.hpp>
#include <ai_voice_runtime/stdio_runtime.hpp>

namespace message = ai_voice::contracts::runtime_message;
namespace runtime = ai_voice::runtime;
using ai_voice::contracts::ErrorCode;

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

namespace {

class MemoryReader final : public runtime::ByteReader {
 public:
  explicit MemoryReader(std::vector<std::uint8_t> bytes, std::size_t chunk_size = 4096U)
      : bytes_(std::move(bytes)), chunk_size_(chunk_size) {}

  runtime::ReadResult read(std::span<std::uint8_t> destination) noexcept override {
    largest_destination_ = std::max(largest_destination_, destination.size());
    if (position_ == bytes_.size()) {
      return {runtime::ReadStatus::Eof, 0U};
    }
    const auto count = std::min({chunk_size_, destination.size(), bytes_.size() - position_});
    std::copy_n(bytes_.begin() + static_cast<std::ptrdiff_t>(position_), count, destination.begin());
    position_ += count;
    return {runtime::ReadStatus::Data, count};
  }

  [[nodiscard]] std::size_t largest_destination() const { return largest_destination_; }

 private:
  std::vector<std::uint8_t> bytes_;
  std::size_t chunk_size_;
  std::size_t position_{0U};
  std::size_t largest_destination_{0U};
};

class ErrorReader final : public runtime::ByteReader {
 public:
  runtime::ReadResult read(std::span<std::uint8_t>) noexcept override {
    return {runtime::ReadStatus::Error, 0U};
  }
};

class AllocationFailureReader final : public runtime::ByteReader {
 public:
  explicit AllocationFailureReader(std::vector<std::uint8_t> bytes) : bytes_(std::move(bytes)) {}

  runtime::ReadResult read(std::span<std::uint8_t> destination) noexcept override {
    assert(bytes_.size() <= destination.size());
    std::copy(bytes_.begin(), bytes_.end(), destination.begin());
    allocation_fault::fail_after(0U);
    return {runtime::ReadStatus::Data, bytes_.size()};
  }

 private:
  std::vector<std::uint8_t> bytes_;
};

class MemoryWriter final : public runtime::ByteWriter {
 public:
  bool write_all(std::span<const std::uint8_t> bytes) noexcept override {
    ++write_count;
    if (fail_on_write != 0U && write_count == fail_on_write) {
      return false;
    }
    output.insert(output.end(), bytes.begin(), bytes.end());
    return true;
  }

  bool flush() noexcept override {
    ++flush_count;
    return !fail_flush;
  }

  std::vector<std::uint8_t> output;
  std::size_t write_count{0U};
  std::size_t flush_count{0U};
  std::size_t fail_on_write{0U};
  bool fail_flush{false};
};

message::RuntimeMessage request(
    std::uint64_t request_id,
    message::Command command,
    std::vector<std::uint8_t> payload = {}) {
  return {
      message::MessageKind::Request,
      ai_voice::contracts::kIpcProtocolCurrentVersion,
      request_id,
      command,
      ErrorCode::Success,
      std::move(payload),
  };
}

std::vector<std::uint8_t> encode_frame(const message::RuntimeMessage& value) {
  const auto result = message::encode(value);
  assert(!result.error.has_value());
  return result.bytes;
}

std::vector<message::RuntimeMessage> decode_all(const std::vector<std::uint8_t>& bytes) {
  message::Decoder decoder;
  std::vector<message::RuntimeMessage> messages;
  for (std::size_t offset = 0U; offset < bytes.size();) {
    const auto count = std::min<std::size_t>(1024U, bytes.size() - offset);
    const auto result = decoder.feed(
        std::span<const std::uint8_t>(bytes.data() + static_cast<std::ptrdiff_t>(offset), count));
    assert(!result.error.has_value());
    messages.insert(messages.end(), result.messages.begin(), result.messages.end());
    offset += count;
  }
  assert(!decoder.finish().has_value());
  return messages;
}

void clean_eof_writes_only_one_flushed_hello() {
  MemoryReader input({});
  MemoryWriter output;
  std::ostringstream diagnostics;
  const auto exit = runtime::run_stdio(input, output, diagnostics, 4U);
  assert(exit == runtime::ProcessExitCode::Success);
  assert(output.write_count == 1U);
  assert(output.flush_count == 1U);
  const auto frames = decode_all(output.output);
  assert(frames.size() == 1U);
  assert(frames[0].kind == message::MessageKind::Hello);
  assert(std::string(frames[0].payload.begin(), frames[0].payload.end()).find(
             R"("generation":4)") != std::string::npos);
  assert(diagnostics.str().empty());
}

void partial_reads_and_sticky_commands_stop_after_flushed_shutdown() {
  auto input_bytes = encode_frame(request(10U, message::Command::Ping, {'o', 'k'}));
  const auto shutdown = encode_frame(request(11U, message::Command::Shutdown));
  const auto ignored = encode_frame(request(12U, message::Command::Ping, {'n', 'o'}));
  input_bytes.insert(input_bytes.end(), shutdown.begin(), shutdown.end());
  input_bytes.insert(input_bytes.end(), ignored.begin(), ignored.end());
  MemoryReader input(std::move(input_bytes), 3U);
  MemoryWriter output;
  std::ostringstream diagnostics;

  const auto exit = runtime::run_stdio(input, output, diagnostics, 1U);
  assert(exit == runtime::ProcessExitCode::Success);
  assert(input.largest_destination() == runtime::kStdioReadChunkBytes);
  const auto frames = decode_all(output.output);
  assert(frames.size() == 3U);
  assert(frames[0].kind == message::MessageKind::Hello);
  assert(frames[1] == message::RuntimeMessage({
                          message::MessageKind::Response,
                          1U,
                          10U,
                          message::Command::Ping,
                          ErrorCode::Success,
                          {'o', 'k'},
                      }));
  assert(frames[2] == message::RuntimeMessage({
                          message::MessageKind::Response,
                          1U,
                          11U,
                          message::Command::Shutdown,
                          ErrorCode::Success,
                          {},
                      }));
  assert(output.flush_count == 3U);
  assert(diagnostics.str().empty());
}

void truncated_or_wrong_direction_input_is_a_protocol_exit_without_stdout_diagnostics() {
  auto truncated = encode_frame(request(1U, message::Command::Ping, {'x'}));
  truncated.pop_back();
  MemoryReader truncated_input(std::move(truncated), 5U);
  MemoryWriter truncated_output;
  std::ostringstream truncated_diagnostics;
  assert(runtime::run_stdio(truncated_input, truncated_output, truncated_diagnostics, 1U) ==
         runtime::ProcessExitCode::ProtocolFailure);
  assert(decode_all(truncated_output.output).size() == 1U);
  assert(!truncated_diagnostics.str().empty());

  const message::RuntimeMessage inbound_response{
      message::MessageKind::Response,
      1U,
      3U,
      message::Command::Ping,
      ErrorCode::Success,
      {'x'},
  };
  MemoryReader direction_input(encode_frame(inbound_response));
  MemoryWriter direction_output;
  std::ostringstream direction_diagnostics;
  assert(runtime::run_stdio(direction_input, direction_output, direction_diagnostics, 1U) ==
         runtime::ProcessExitCode::ProtocolFailure);
  assert(decode_all(direction_output.output).size() == 1U);
}

void stdout_failure_and_input_failure_have_distinct_exits() {
  MemoryReader shutdown_input(encode_frame(request(1U, message::Command::Shutdown)));
  MemoryWriter failed_output;
  failed_output.fail_on_write = 2U;
  std::ostringstream output_diagnostics;
  assert(runtime::run_stdio(shutdown_input, failed_output, output_diagnostics, 1U) ==
         runtime::ProcessExitCode::OutputFailure);
  assert(decode_all(failed_output.output).size() == 1U);

  ErrorReader error_input;
  MemoryWriter normal_output;
  std::ostringstream input_diagnostics;
  assert(runtime::run_stdio(error_input, normal_output, input_diagnostics, 1U) ==
         runtime::ProcessExitCode::UnexpectedFailure);
  assert(decode_all(normal_output.output).size() == 1U);
}

void transport_splits_more_than_one_codec_batch_without_rejecting_valid_sticky_frames() {
  std::vector<std::uint8_t> input_bytes;
  for (std::uint64_t request_id = 1U; request_id <= message::kMaxMessagesPerFeed + 1U;
       ++request_id) {
    const auto frame = encode_frame(request(request_id, message::Command::Ping));
    input_bytes.insert(input_bytes.end(), frame.begin(), frame.end());
  }
  const auto shutdown =
      encode_frame(request(message::kMaxMessagesPerFeed + 2U, message::Command::Shutdown));
  input_bytes.insert(input_bytes.end(), shutdown.begin(), shutdown.end());

  MemoryReader input(std::move(input_bytes), message::kMaxFrameBytes);
  MemoryWriter output;
  std::ostringstream diagnostics;
  assert(runtime::run_stdio(input, output, diagnostics, 1U) == runtime::ProcessExitCode::Success);
  const auto frames = decode_all(output.output);
  assert(frames.size() == message::kMaxMessagesPerFeed + 3U);
  assert(frames.back().command == message::Command::Shutdown);
  assert(diagnostics.str().empty());
}

void decoder_allocation_failure_is_an_unexpected_process_failure() {
  AllocationFailureReader input(encode_frame(request(1U, message::Command::Ping, {'x'})));
  MemoryWriter output;
  std::ostringstream diagnostics;
  assert(runtime::run_stdio(input, output, diagnostics, 1U) ==
         runtime::ProcessExitCode::UnexpectedFailure);
  allocation_fault::disable();
  assert(decode_all(output.output).size() == 1U);
  assert(!diagnostics.str().empty());
}

}  // namespace

int main() {
  clean_eof_writes_only_one_flushed_hello();
  partial_reads_and_sticky_commands_stop_after_flushed_shutdown();
  truncated_or_wrong_direction_input_is_a_protocol_exit_without_stdout_diagnostics();
  stdout_failure_and_input_failure_have_distinct_exits();
  transport_splits_more_than_one_codec_batch_without_rejecting_valid_sticky_frames();
  decoder_allocation_failure_is_an_unexpected_process_failure();
}
