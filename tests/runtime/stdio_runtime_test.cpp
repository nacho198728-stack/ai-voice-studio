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
#include <iostream>
#include <sstream>
#include <span>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include <ai_voice_contracts/generated_contracts.hpp>
#include <ai_voice_contracts/runtime_message.hpp>
#include <ai_voice_runtime/session.hpp>
#include <ai_voice_runtime/stdio_runtime.hpp>

namespace message = ai_voice::contracts::runtime_message;
namespace runtime = ai_voice::runtime;
using ai_voice::contracts::ErrorCode;

namespace test_support {

[[noreturn]] void assertion_failed(const char* expression, int line) {
  throw std::runtime_error(
      "stdio runtime assertion failed at line " + std::to_string(line) + ": " + expression);
}

}  // namespace test_support

#undef assert
#define assert(expression) \
  ((expression) ? static_cast<void>(0) : test_support::assertion_failed(#expression, __LINE__))

namespace allocation_fault {

thread_local std::size_t countdown = std::numeric_limits<std::size_t>::max();

void fail_after(std::size_t successful_allocations) {
  countdown = successful_allocations;
}

void disable() {
  countdown = std::numeric_limits<std::size_t>::max();
}

}  // namespace allocation_fault

#ifndef _WIN32
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
#endif

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

#ifndef _WIN32
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
#endif

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

#ifndef _WIN32
class AllocationFailurePipeline final : public runtime::PipelineService {
 public:
  [[nodiscard]] bool available() const noexcept override { return true; }

  runtime::PipelineRunResult run(std::span<const std::uint8_t>) override {
    allocation_fault::fail_after(0U);
    return {ErrorCode::Success, std::vector<std::uint8_t>(1U, 0xA5U)};
  }
};
#endif

struct CapturedLog {
  runtime::LogLevel level;
  std::string message;
  runtime::LogFields fields;
};

class CapturingLogSink final : public runtime::LogSink {
 public:
  void log(
      runtime::LogLevel level,
      std::string_view message,
      runtime::LogFields fields) noexcept override {
    logs.push_back({level, std::string(message), fields});
  }

  void flush() noexcept override { ++flush_count; }

  std::vector<CapturedLog> logs;
  std::size_t flush_count{0U};
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

void lifecycle_and_request_logs_are_injected_without_entering_protocol_output() {
  auto input_bytes = encode_frame(request(21U, message::Command::Ping, {'x'}));
  const auto shutdown = encode_frame(request(22U, message::Command::Shutdown));
  input_bytes.insert(input_bytes.end(), shutdown.begin(), shutdown.end());
  MemoryReader input(std::move(input_bytes));
  MemoryWriter output;
  std::ostringstream emergency_diagnostics;
  CapturingLogSink logging;

  assert(runtime::run_stdio(
             input, output, emergency_diagnostics, 8U, nullptr, &logging) ==
         runtime::ProcessExitCode::Success);
  assert(decode_all(output.output).size() == 3U);
  assert(emergency_diagnostics.str().empty());
  assert(logging.logs.size() >= 4U);
  assert(logging.logs.front().fields.generation == 8U);
  const auto correlated = std::find_if(
      logging.logs.begin(), logging.logs.end(), [](const CapturedLog& record) {
        return record.fields.request_id == 21U && record.fields.generation == 8U;
      });
  assert(correlated != logging.logs.end());
  assert(correlated->level == runtime::LogLevel::Debug);
  assert(logging.flush_count == 1U);
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

#ifndef _WIN32
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

void pipeline_dispatch_allocation_failure_stops_after_the_valid_hello() {
  MemoryReader input(encode_frame(request(1U, message::Command::RunMockPipeline)));
  MemoryWriter output;
  std::ostringstream diagnostics;
  AllocationFailurePipeline pipeline;
  assert(runtime::run_stdio(input, output, diagnostics, 1U, &pipeline) ==
         runtime::ProcessExitCode::UnexpectedFailure);
  allocation_fault::disable();
  const auto frames = decode_all(output.output);
  assert(frames.size() == 1U);
  assert(frames[0].kind == message::MessageKind::Hello);
  assert(!diagnostics.str().empty());
  assert(diagnostics.str().size() <= 513U);
}
#endif

}  // namespace

int main() {
  try {
    std::cerr << "stdio runtime phase: clean EOF" << std::endl;
    clean_eof_writes_only_one_flushed_hello();
    std::cerr << "stdio runtime phase: partial reads and shutdown" << std::endl;
    partial_reads_and_sticky_commands_stop_after_flushed_shutdown();
    std::cerr << "stdio runtime phase: injected logging" << std::endl;
    lifecycle_and_request_logs_are_injected_without_entering_protocol_output();
    std::cerr << "stdio runtime phase: invalid input" << std::endl;
    truncated_or_wrong_direction_input_is_a_protocol_exit_without_stdout_diagnostics();
    std::cerr << "stdio runtime phase: transport failures" << std::endl;
    stdout_failure_and_input_failure_have_distinct_exits();
    std::cerr << "stdio runtime phase: codec batches" << std::endl;
    transport_splits_more_than_one_codec_batch_without_rejecting_valid_sticky_frames();
    std::cerr << "stdio runtime phase: decoder allocation failure" << std::endl;
#ifndef _WIN32
    decoder_allocation_failure_is_an_unexpected_process_failure();
    std::cerr << "stdio runtime phase: pipeline allocation failure" << std::endl;
    pipeline_dispatch_allocation_failure_stops_after_the_valid_hello();
#else
    std::cerr << "stdio runtime allocation injection: skipped on MSVC because process-wide "
                 "operator new interception is not deterministic"
              << std::endl;
#endif
    return 0;
  } catch (const std::exception& error) {
    allocation_fault::disable();
    std::cerr << error.what() << std::endl;
    return 1;
  }
}
