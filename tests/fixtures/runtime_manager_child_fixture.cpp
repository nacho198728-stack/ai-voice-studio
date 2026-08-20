#include <ai_voice_contracts/runtime_message.hpp>

#include <array>
#include <chrono>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <thread>
#include <utility>
#include <vector>

#if defined(_WIN32)
#include <io.h>
#include <windows.h>
#else
#if defined(__linux__)
#include <fcntl.h>
#endif
#include <poll.h>
#include <signal.h>
#include <unistd.h>
#endif

namespace {

namespace message = ai_voice::contracts::runtime_message;
using ai_voice::contracts::ErrorCode;

constexpr std::string_view kHello =
    R"({"runtime_version":"0.0.0","protocol_version":1,"generation":1,"health":"starting"})";
constexpr std::string_view kCapabilities =
    R"({"platform":"unknown","architecture":"unknown","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"})";
std::string g_executable_path;
std::uint64_t g_generation = 1U;

std::vector<std::uint8_t> bytes(std::string_view value) {
  return {value.begin(), value.end()};
}

bool write_bytes(std::span<const std::uint8_t> value) {
  while (!value.empty()) {
    const auto written = std::fwrite(value.data(), 1U, value.size(), stdout);
    if (written == 0U) {
      return false;
    }
    value = value.subspan(written);
  }
  return std::fflush(stdout) == 0;
}

bool send(const message::RuntimeMessage& value) {
  const auto encoded = message::encode(value);
  return !encoded.error.has_value() && write_bytes(encoded.bytes);
}

message::RuntimeMessage hello() {
  return {
      message::MessageKind::Hello,
      ai_voice::contracts::kIpcProtocolCurrentVersion,
      0U,
      message::Command::None,
      ErrorCode::Success,
      bytes(kHello),
  };
}

message::RuntimeMessage response(
    const message::RuntimeMessage& request,
    message::Command command,
    std::vector<std::uint8_t> payload = {}) {
  return {
      message::MessageKind::Response,
      ai_voice::contracts::kIpcProtocolCurrentVersion,
      request.request_id,
      command,
      ErrorCode::Success,
      std::move(payload),
  };
}

message::RuntimeMessage error_response(
    const message::RuntimeMessage& request,
    ErrorCode error_code) {
  return {
      message::MessageKind::Response,
      ai_voice::contracts::kIpcProtocolCurrentVersion,
      request.request_id,
      request.command,
      error_code,
      {},
  };
}

void close_stdout() {
  std::fflush(stdout);
#if defined(_WIN32)
  ::_close(::_fileno(stdout));
#else
  ::close(STDOUT_FILENO);
#endif
}

void close_stdin() {
#if defined(_WIN32)
  ::_close(::_fileno(stdin));
#else
  ::close(STDIN_FILENO);
#endif
}

void write_u32(std::vector<std::uint8_t>& frame, std::size_t offset, std::uint32_t value) {
  for (std::size_t index = 0U; index < 4U; ++index) {
    frame[offset + index] = static_cast<std::uint8_t>(value >> (index * 8U));
  }
}

[[noreturn]] void hang() {
  std::this_thread::sleep_for(std::chrono::seconds(30));
  std::exit(9);
}

[[noreturn]] void descendant_writer(std::uint64_t request_id) {
#if defined(_WIN32)
  // Anonymous-pipe writes are synchronous on Windows. If a hostile inherited
  // handle keeps the pipe nominally open after the Runtime parent exits, the
  // writer can remain blocked inside fwrite and outlive the test that created
  // it. The watchdog is deliberately far beyond the manager's 500 ms drain
  // assertion, so it cannot satisfy that assertion; it only guarantees final
  // fixture cleanup after the lifecycle behavior has already been observed.
  std::thread([] {
    std::this_thread::sleep_for(std::chrono::seconds(5));
    static_cast<void>(::TerminateProcess(::GetCurrentProcess(), 0U));
  }).detach();
#else
  ::signal(SIGPIPE, SIG_DFL);
#endif
  const message::RuntimeMessage value{
      message::MessageKind::Response,
      ai_voice::contracts::kIpcProtocolCurrentVersion,
      request_id,
      message::Command::GetCapabilities,
      ErrorCode::Success,
      bytes(kCapabilities),
  };
  while (send(value)) {
  }
  std::exit(0);
}

[[noreturn]] void descendant_holder() {
  const auto deadline =
      std::chrono::steady_clock::now() + std::chrono::seconds(5);
  while (std::chrono::steady_clock::now() < deadline) {
#if defined(_WIN32)
    DWORD state = 0U;
    if (::GetNamedPipeHandleStateA(
            ::GetStdHandle(STD_OUTPUT_HANDLE), &state, nullptr, nullptr, nullptr,
            nullptr, 0U) == FALSE) {
      std::exit(0);
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(5));
#else
    pollfd descriptor{STDOUT_FILENO, POLLOUT, 0};
    const auto result = ::poll(&descriptor, 1U, 10);
    if (result > 0 &&
        (descriptor.revents & (POLLERR | POLLHUP | POLLNVAL)) != 0) {
      std::exit(0);
    }
#endif
  }
  std::exit(0);
}

bool spawn_writer_descendant(std::uint64_t request_id) {
#if defined(_WIN32)
  auto command = '"' + g_executable_path + "\" --descendant-writer " +
                 std::to_string(request_id);
  STARTUPINFOA startup{};
  startup.cb = sizeof(startup);
  PROCESS_INFORMATION process{};
  const auto created = ::CreateProcessA(
      nullptr,
      command.data(),
      nullptr,
      nullptr,
      TRUE,
      CREATE_NO_WINDOW,
      nullptr,
      nullptr,
      &startup,
      &process);
  if (created == FALSE) {
    return false;
  }
  ::CloseHandle(process.hThread);
  ::CloseHandle(process.hProcess);
  return true;
#else
  const auto pid = ::fork();
  if (pid == 0) {
    descendant_writer(request_id);
  }
  return pid > 0;
#endif
}

bool spawn_holder_descendant() {
#if defined(_WIN32)
  auto command = '"' + g_executable_path + "\" --descendant-holder";
  STARTUPINFOA startup{};
  startup.cb = sizeof(startup);
  PROCESS_INFORMATION process{};
  const auto created = ::CreateProcessA(
      nullptr,
      command.data(),
      nullptr,
      nullptr,
      TRUE,
      CREATE_NO_WINDOW,
      nullptr,
      nullptr,
      &startup,
      &process);
  if (created == FALSE) {
    return false;
  }
  ::CloseHandle(process.hThread);
  ::CloseHandle(process.hProcess);
  return true;
#else
  const auto pid = ::fork();
  if (pid == 0) {
    descendant_holder();
  }
  return pid > 0;
#endif
}

void minimize_stdin_pipe_capacity() {
#if defined(__linux__) && defined(F_SETPIPE_SZ)
  static_cast<void>(::fcntl(STDIN_FILENO, F_SETPIPE_SZ, 4'096));
#endif
}

std::optional<std::string> mode_from_arguments(int argc, char** argv) {
  if (argc < 3 || argv == nullptr) {
    return std::nullopt;
  }
  for (int index = 1; index + 1 < argc; ++index) {
    if (std::string_view(argv[index]) == "--plugin") {
      return std::filesystem::path(argv[index + 1]).filename().string();
    }
  }
  return std::nullopt;
}

std::optional<std::uint64_t> generation_from_arguments(int argc, char** argv) {
  if (argv == nullptr) {
    return std::nullopt;
  }
  for (int index = 1; index + 1 < argc; ++index) {
    if (std::string_view(argv[index]) == "--generation") {
      try {
        return std::stoull(argv[index + 1]);
      } catch (...) {
        return std::nullopt;
      }
    }
  }
  return std::nullopt;
}

int run_loop(std::string_view mode) {
  if (mode == "stderr-flood") {
    constexpr std::array<char, 4'096> kFlood{};
    for (std::size_t index = 0U; index < 256U; ++index) {
      if (std::fwrite(kFlood.data(), 1U, kFlood.size(), stderr) != kFlood.size()) {
        return 8;
      }
    }
    std::fputs("stderr-flood-complete\n", stderr);
    std::fflush(stderr);
  }
  if (!send(hello())) {
    return 3;
  }
  if (mode == "stdin-close-hang") {
    close_stdin();
    std::fputs("stdin-closed\n", stderr);
    std::fflush(stderr);
    hang();
  }
  if (mode == "exit-nonzero") {
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
    return 7;
  }
  if (mode == "exit-descendant") {
    if (!spawn_holder_descendant()) {
      return 8;
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
    return 7;
  }

  message::Decoder decoder;
  std::array<std::uint8_t, 512> buffer{};
  std::optional<message::RuntimeMessage> held_ping;
  for (;;) {
    std::ptrdiff_t read_count = 0;
#if defined(_WIN32)
    read_count = ::_read(::_fileno(stdin), buffer.data(), static_cast<unsigned int>(buffer.size()));
#else
    read_count = ::read(STDIN_FILENO, buffer.data(), buffer.size());
#endif
    if (read_count <= 0) {
      return read_count == 0 ? 0 : 4;
    }
    const auto count = static_cast<std::size_t>(read_count);
    const auto decoded = decoder.feed(std::span<const std::uint8_t>(buffer.data(), count));
    if (decoded.error.has_value()) {
      return 2;
    }
    for (const auto& request : decoded.messages) {
      if (request.command == message::Command::Ping) {
        if (mode == "drain-descendant") {
          std::fputs("drain-parent-final-stderr", stderr);
          std::fflush(stderr);
          for (std::size_t index = 0U; index < 8U; ++index) {
            if (!spawn_writer_descendant(request.request_id)) {
              return 8;
            }
          }
          return 7;
        }
        if (mode == "request-close-hang") {
          close_stdout();
          hang();
        }
        if (mode == "stdin-block") {
          std::fputs("first-request-held\n", stderr);
          std::fflush(stderr);
          hang();
        }
        if (mode == "full-pending" || mode == "cancelled-pending" ||
            mode == "request-timeout") {
          held_ping = request;
          std::fputs("ping-held\n", stderr);
          std::fflush(stderr);
          continue;
        }
        if (mode == "correlation-mismatch") {
          for (std::size_t index = 0U; index < 100U; ++index) {
            if (!send(response(
                    request, message::Command::GetCapabilities, bytes(kCapabilities)))) {
              return 3;
            }
          }
          hang();
        }
        if (mode == "response-unknown-id") {
          auto unknown = response(request, message::Command::Ping, request.payload);
          ++unknown.request_id;
          if (!send(unknown)) {
            return 3;
          }
          hang();
        }
        if (mode == "response-duplicate") {
          const auto duplicate = response(request, message::Command::Ping, request.payload);
          if (!send(duplicate) || !send(duplicate)) {
            return 3;
          }
          hang();
        }
        if (mode == "posthello-request") {
          const message::RuntimeMessage wrong{
              message::MessageKind::Request,
              ai_voice::contracts::kIpcProtocolCurrentVersion,
              request.request_id,
              message::Command::Ping,
              ErrorCode::Success,
              {},
          };
          if (!send(wrong)) {
            return 3;
          }
          hang();
        }
        if (mode == "posthello-hello") {
          if (!send(hello())) {
            return 3;
          }
          hang();
        }
        if (mode == "stale-responses" && g_generation == 1U) {
          for (std::size_t index = 0U; index < 100U; ++index) {
            if (!send(response(
                    request, message::Command::GetCapabilities, bytes(kCapabilities)))) {
              return 3;
            }
          }
          hang();
        }
        if (mode == "environment") {
          const auto clean = std::getenv("AIVS_TEST_SECRET") == nullptr;
          if (!send(response(
                  request,
                  message::Command::Ping,
                  bytes(clean ? std::string_view("clean") : std::string_view("leaked"))))) {
            return 3;
          }
          continue;
        }
        if (!send(response(request, message::Command::Ping, request.payload))) {
          return 3;
        }
        continue;
      }
      if (request.command == message::Command::GetCapabilities) {
        if (mode == "capability-hang") {
          std::fputs("capability-held\n", stderr);
          std::fflush(stderr);
          hang();
        }
        if (mode == "capability-error") {
          if (!send(error_response(request, ErrorCode::EngineUnavailable))) {
            return 3;
          }
          continue;
        }
        if (mode == "malformed-capabilities") {
          if (!send(response(request, request.command, bytes("{}")))) {
            return 3;
          }
          hang();
        }
        if (!send(response(request, request.command, bytes(kCapabilities)))) {
          return 3;
        }
        continue;
      }
      if (request.command == message::Command::Shutdown) {
        if (mode == "shutdown-delay" || mode == "shutdown-error" ||
            mode == "shutdown-hang") {
          std::fputs("shutdown-received\n", stderr);
          std::fflush(stderr);
        }
        if (mode == "shutdown-hang") {
          hang();
        }
        if (mode == "shutdown-delay" || mode == "shutdown-error") {
          std::this_thread::sleep_for(std::chrono::milliseconds(150));
        }
        if (mode == "shutdown-error") {
          if (!send(error_response(request, ErrorCode::EngineUnavailable))) {
            return 3;
          }
          hang();
        }
        if (held_ping.has_value() && mode == "full-pending" &&
            !send(response(*held_ping, message::Command::Ping, held_ping->payload))) {
          return 3;
        }
        if (!send(response(request, message::Command::Shutdown))) {
          return 3;
        }
        return 0;
      }
    }
  }
}

}  // namespace

int main(int argc, char** argv) {
  std::setvbuf(stdout, nullptr, _IONBF, 0U);
  if (argc == 3 && std::string_view(argv[1]) == "--descendant-writer") {
    descendant_writer(std::stoull(argv[2]));
  }
  if (argc == 2 && std::string_view(argv[1]) == "--descendant-holder") {
    descendant_holder();
  }
  if (argc > 0 && argv != nullptr) {
    g_executable_path = argv[0];
  }
  if (const auto generation = generation_from_arguments(argc, argv); generation.has_value()) {
    g_generation = *generation;
  }
  const auto mode = mode_from_arguments(argc, argv);
  if (!mode.has_value()) {
    return 4;
  }
  if (*mode == "handshake-hang") {
    hang();
  }
  if (*mode == "stdout-close-hang") {
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
    close_stdout();
    hang();
  }
  if (*mode == "prehello-response") {
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
    const message::RuntimeMessage wrong{
        message::MessageKind::Response,
        ai_voice::contracts::kIpcProtocolCurrentVersion,
        1U,
        message::Command::Ping,
        ErrorCode::Success,
        bytes("wrong"),
    };
    if (!send(wrong)) {
      return 3;
    }
    hang();
  }
  if (*mode == "prehello-truncated") {
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
    const auto encoded = message::encode(hello());
    if (encoded.error.has_value() || encoded.bytes.size() < 10U ||
        !write_bytes(std::span<const std::uint8_t>(encoded.bytes.data(), 10U))) {
      return 3;
    }
    close_stdout();
    hang();
  }
  if (*mode == "prehello-malformed-frame" || *mode == "prehello-oversized-frame" ||
      *mode == "prehello-wrong-version") {
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
    auto encoded = message::encode(hello());
    if (encoded.error.has_value()) {
      return 3;
    }
    if (*mode == "prehello-malformed-frame") {
      encoded.bytes[0] = static_cast<std::uint8_t>('X');
    } else if (*mode == "prehello-oversized-frame") {
      write_u32(
          encoded.bytes,
          28U,
          static_cast<std::uint32_t>(message::kMaxHelloPayloadBytes + 1U));
    } else {
      write_u32(
          encoded.bytes, 8U, ai_voice::contracts::kIpcProtocolCurrentVersion + 1U);
    }
    if (!write_bytes(encoded.bytes)) {
      return 3;
    }
    hang();
  }
  if (*mode == "prehello-malformed-payload") {
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
    auto malformed = hello();
    malformed.payload = bytes("{");
    if (!send(malformed)) {
      return 3;
    }
    hang();
  }
  if (*mode == "stderr-exit") {
    std::fputs("final-stderr-diagnostic", stderr);
    std::fflush(stderr);
    return 4;
  }
  if (*mode == "stdin-block") {
    minimize_stdin_pipe_capacity();
  }
  return run_loop(*mode);
}
