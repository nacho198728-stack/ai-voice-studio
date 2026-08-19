#pragma once

#include <cstddef>
#include <cstdint>
#include <iosfwd>
#include <span>

#include <ai_voice_contracts/runtime_message_generated.hpp>
#include <ai_voice_runtime/logging.hpp>

namespace ai_voice::runtime {

class PipelineService;

inline constexpr std::size_t kStdioReadChunkBytes =
    contracts::runtime_message::kHeaderSize * contracts::runtime_message::kMaxMessagesPerFeed;
static_assert(kStdioReadChunkBytes <= contracts::runtime_message::kMaxInputBytesPerFeed);

enum class ReadStatus {
  Data,
  Eof,
  Error,
};

struct ReadResult {
  ReadStatus status;
  std::size_t size;
};

class ByteReader {
 public:
  virtual ~ByteReader() = default;
  virtual ReadResult read(std::span<std::uint8_t> destination) noexcept = 0;
};

class ByteWriter {
 public:
  virtual ~ByteWriter() = default;
  virtual bool write_all(std::span<const std::uint8_t> bytes) noexcept = 0;
  virtual bool flush() noexcept = 0;
};

enum class ProcessExitCode : int {
  Success = 0,
  ProtocolFailure = 2,
  OutputFailure = 3,
  UnexpectedFailure = 4,
};

[[nodiscard]] ProcessExitCode run_stdio(
    ByteReader& input,
    ByteWriter& output,
    std::ostream& diagnostics,
    std::uint64_t generation,
    PipelineService* pipeline = nullptr,
    LogSink* logging = nullptr) noexcept;

[[nodiscard]] ProcessExitCode run_native_stdio(int argc, char** argv) noexcept;
[[nodiscard]] ProcessExitCode run_native_stdio(int argc, wchar_t** argv) noexcept;

}  // namespace ai_voice::runtime
