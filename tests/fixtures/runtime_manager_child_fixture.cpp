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
#else
#include <unistd.h>
#endif

namespace {

namespace message = ai_voice::contracts::runtime_message;
using ai_voice::contracts::ErrorCode;

constexpr std::string_view kHello =
    R"({"runtime_version":"0.0.0","protocol_version":1,"generation":1,"health":"starting"})";
constexpr std::string_view kCapabilities =
    R"({"platform":"unknown","architecture":"unknown","runtime_version":"0.0.0","protocol_version":1,"backend":"mock","engine":"aivs-mock-v1"})";

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

void close_stdout() {
  std::fflush(stdout);
#if defined(_WIN32)
  ::_close(::_fileno(stdout));
#else
  ::close(STDOUT_FILENO);
#endif
}

[[noreturn]] void hang() {
  std::this_thread::sleep_for(std::chrono::seconds(30));
  std::exit(9);
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

int run_loop(std::string_view mode) {
  if (!send(hello())) {
    return 3;
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
        if (mode == "request-close-hang") {
          close_stdout();
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
        if (!send(response(request, request.command, bytes(kCapabilities)))) {
          return 3;
        }
        continue;
      }
      if (request.command == message::Command::Shutdown) {
        if (mode == "shutdown-hang") {
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
  if (*mode == "stderr-exit") {
    std::fputs("final-stderr-diagnostic", stderr);
    std::fflush(stderr);
    return 4;
  }
  return run_loop(*mode);
}
