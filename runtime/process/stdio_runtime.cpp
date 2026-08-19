#include <ai_voice_runtime/stdio_runtime.hpp>

#include <array>
#include <cerrno>
#include <cstdio>
#include <csignal>
#include <iostream>
#include <memory>
#include <ostream>
#include <span>
#include <string_view>
#include <vector>

#include <ai_voice_contracts/runtime_message.hpp>
#include <ai_voice_runtime/mock_pipeline.hpp>
#include <ai_voice_runtime/runtime_options.hpp>
#include <ai_voice_runtime/session.hpp>

#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>
#else
#include <unistd.h>
#endif

namespace ai_voice::runtime {
namespace {

namespace message = contracts::runtime_message;

constexpr std::size_t kMaxDiagnosticBytes = 512U;

void diagnostic(std::ostream& output, std::string_view text) noexcept {
  try {
    const auto bounded = text.substr(0U, kMaxDiagnosticBytes);
    output.write(bounded.data(), static_cast<std::streamsize>(bounded.size()));
    output.put('\n');
    output.flush();
  } catch (...) {
  }
}

enum class SendResult {
  Success,
  EncodeFailure,
  OutputFailure,
};

SendResult send(ByteWriter& output, const message::RuntimeMessage& outbound) {
  const auto encoded = message::encode(outbound);
  if (encoded.error.has_value()) {
    return SendResult::EncodeFailure;
  }
  if (!output.write_all(encoded.bytes) || !output.flush()) {
    return SendResult::OutputFailure;
  }
  return SendResult::Success;
}

class NativeStdinReader final : public ByteReader {
 public:
  ReadResult read(std::span<std::uint8_t> destination) noexcept override {
    for (;;) {
#if defined(_WIN32)
      const auto count = ::_read(
          ::_fileno(stdin), destination.data(), static_cast<unsigned int>(destination.size()));
#else
      const auto count = ::read(STDIN_FILENO, destination.data(), destination.size());
#endif
      if (count > 0) {
        return {ReadStatus::Data, static_cast<std::size_t>(count)};
      }
      if (count == 0) {
        return {ReadStatus::Eof, 0U};
      }
      if (errno != EINTR) {
        return {ReadStatus::Error, 0U};
      }
    }
  }
};

class NativeStdoutWriter final : public ByteWriter {
 public:
  bool write_all(std::span<const std::uint8_t> bytes) noexcept override {
    while (!bytes.empty()) {
      const auto written = std::fwrite(bytes.data(), 1U, bytes.size(), stdout);
      if (written == 0U) {
        return false;
      }
      bytes = bytes.subspan(written);
    }
    return true;
  }

  bool flush() noexcept override {
    return std::fflush(stdout) == 0;
  }
};

bool configure_native_stdio() noexcept {
#if defined(_WIN32)
  if (::_setmode(::_fileno(stdin), _O_BINARY) == -1 ||
      ::_setmode(::_fileno(stdout), _O_BINARY) == -1) {
    return false;
  }
#else
  if (std::signal(SIGPIPE, SIG_IGN) == SIG_ERR) {
    return false;
  }
#endif
  return std::setvbuf(stdout, nullptr, _IONBF, 0U) == 0;
}

}  // namespace

ProcessExitCode run_stdio(
    ByteReader& input,
    ByteWriter& output,
    std::ostream& diagnostics,
    std::uint64_t generation,
    PipelineService* pipeline) noexcept {
  Session session(generation, pipeline);
  try {
    const auto hello = session.start();
    if (!hello.has_value()) {
      session.mark_failure(ExitReason::UnexpectedFailure);
      diagnostic(diagnostics, "voice-runtime: could not enter running state");
      return ProcessExitCode::UnexpectedFailure;
    }
    const auto hello_result = send(output, *hello);
    if (hello_result != SendResult::Success) {
      const auto exit = hello_result == SendResult::OutputFailure
                            ? ProcessExitCode::OutputFailure
                            : ProcessExitCode::UnexpectedFailure;
      session.mark_failure(
          exit == ProcessExitCode::OutputFailure ? ExitReason::OutputFailure
                                                 : ExitReason::UnexpectedFailure);
      diagnostic(diagnostics, "voice-runtime: startup frame write failed");
      return exit;
    }

    message::Decoder decoder;
    std::array<std::uint8_t, kStdioReadChunkBytes> buffer{};
    for (;;) {
      const auto read = input.read(buffer);
      if (read.status == ReadStatus::Error ||
          (read.status == ReadStatus::Data && (read.size == 0U || read.size > buffer.size())) ||
          (read.status != ReadStatus::Data && read.size != 0U)) {
        session.mark_failure(ExitReason::UnexpectedFailure);
        diagnostic(diagnostics, "voice-runtime: stdin read failed");
        return ProcessExitCode::UnexpectedFailure;
      }
      if (read.status == ReadStatus::Eof) {
        if (decoder.finish().has_value()) {
          session.mark_failure(ExitReason::ProtocolFailure);
          diagnostic(diagnostics, "voice-runtime: malformed or truncated input frame");
          return ProcessExitCode::ProtocolFailure;
        }
        session.mark_clean_eof();
        return ProcessExitCode::Success;
      }

      const auto decoded = decoder.feed(std::span<const std::uint8_t>(buffer.data(), read.size));
      if (decoded.error.has_value()) {
        if (decoded.error->kind == message::FrameErrorKind::AllocationFailure) {
          session.mark_failure(ExitReason::UnexpectedFailure);
          diagnostic(diagnostics, "voice-runtime: decoder allocation failed");
          return ProcessExitCode::UnexpectedFailure;
        }
        session.mark_failure(ExitReason::ProtocolFailure);
        diagnostic(diagnostics, "voice-runtime: malformed, oversized, or unsupported input frame");
        return ProcessExitCode::ProtocolFailure;
      }
      for (const auto& inbound : decoded.messages) {
        const auto dispatched = session.handle(inbound);
        if (dispatched.disposition == DispatchDisposition::ProtocolFailure ||
            !dispatched.response.has_value()) {
          diagnostic(diagnostics, "voice-runtime: invalid inbound message direction");
          return ProcessExitCode::ProtocolFailure;
        }
        const auto response_result = send(output, *dispatched.response);
        if (response_result != SendResult::Success) {
          const auto exit = response_result == SendResult::OutputFailure
                                ? ProcessExitCode::OutputFailure
                                : ProcessExitCode::UnexpectedFailure;
          session.mark_failure(
              exit == ProcessExitCode::OutputFailure ? ExitReason::OutputFailure
                                                     : ExitReason::UnexpectedFailure);
          diagnostic(diagnostics, "voice-runtime: response frame write failed");
          return exit;
        }
        if (dispatched.disposition == DispatchDisposition::StopAfterResponse) {
          session.mark_shutdown_response_written();
          return ProcessExitCode::Success;
        }
      }
    }
  } catch (...) {
    session.mark_failure(ExitReason::UnexpectedFailure);
    diagnostic(diagnostics, "voice-runtime: unexpected exception");
    return ProcessExitCode::UnexpectedFailure;
  }
}

namespace {

ProcessExitCode run_native_with_options(RuntimeOptionsParseResult parsed) noexcept {
  std::unique_ptr<MockPipeline> pipeline;
  try {
    if (!parsed.options.has_value()) {
      diagnostic(std::cerr, parsed.diagnostic);
      return ProcessExitCode::UnexpectedFailure;
    }
    if (parsed.options->plugin_path.has_value()) {
      auto created = MockPipeline::create(
          *parsed.options->plugin_path, parsed.options->mock_work_iterations);
      if (created.error_code != contracts::ErrorCode::Success || !created.pipeline) {
        diagnostic(std::cerr, created.diagnostic);
        return ProcessExitCode::UnexpectedFailure;
      }
      pipeline = std::move(created.pipeline);
    }
  } catch (...) {
    diagnostic(std::cerr, "voice-runtime: setup failed unexpectedly");
    return ProcessExitCode::UnexpectedFailure;
  }
  if (!configure_native_stdio()) {
    diagnostic(std::cerr, "voice-runtime: stdio configuration failed");
    return ProcessExitCode::UnexpectedFailure;
  }
  NativeStdinReader input;
  NativeStdoutWriter output;
  return run_stdio(input, output, std::cerr, 1U, pipeline.get());
}

template <typename Character>
ProcessExitCode run_native_arguments(int argc, Character** argv) noexcept {
  try {
    if (argc < 1 || argv == nullptr) {
      diagnostic(std::cerr, "voice-runtime: invalid process arguments");
      return ProcessExitCode::UnexpectedFailure;
    }
    std::vector<std::basic_string_view<Character>> arguments;
    arguments.reserve(static_cast<std::size_t>(argc - 1));
    for (int index = 1; index < argc; ++index) {
      if (argv[index] == nullptr) {
        diagnostic(std::cerr, "voice-runtime: invalid null command-line argument");
        return ProcessExitCode::UnexpectedFailure;
      }
      arguments.emplace_back(argv[index]);
    }
    return run_native_with_options(parse_runtime_options(arguments));
  } catch (...) {
    diagnostic(std::cerr, "voice-runtime: argument collection failed unexpectedly");
    return ProcessExitCode::UnexpectedFailure;
  }
}

}  // namespace

ProcessExitCode run_native_stdio(int argc, char** argv) noexcept {
  return run_native_arguments(argc, argv);
}

ProcessExitCode run_native_stdio(int argc, wchar_t** argv) noexcept {
  return run_native_arguments(argc, argv);
}

}  // namespace ai_voice::runtime
