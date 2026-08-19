#include <ai_voice_runtime/session.hpp>

#include <array>
#include <charconv>
#include <cstdint>
#include <string>
#include <string_view>
#include <system_error>
#include <utility>
#include <vector>

#include <ai_voice_contracts/generated_contracts.hpp>

namespace ai_voice::runtime {
namespace {

namespace message = contracts::runtime_message;
using contracts::ErrorCode;

std::vector<std::uint8_t> payload(std::string_view text) {
  return {text.begin(), text.end()};
}

void append_unsigned(std::string& output, std::uint64_t value) {
  std::array<char, 32> digits{};
  const auto [end, error] = std::to_chars(digits.data(), digits.data() + digits.size(), value);
  if (error != std::errc{}) {
    throw std::system_error(std::make_error_code(error));
  }
  output.append(digits.data(), end);
}

std::vector<std::uint8_t> hello_payload(std::uint64_t generation) {
  std::string output;
  output.reserve(128U);
  output.append(R"({"runtime_version":")");
  output.append(contracts::kRuntimeVersion);
  output.append(R"(","protocol_version":)");
  append_unsigned(output, contracts::kIpcProtocolCurrentVersion);
  output.append(R"(,"generation":)");
  append_unsigned(output, generation);
  output.append(R"(,"health":"starting"})");
  return payload(output);
}

constexpr std::string_view platform_name() {
#if defined(__APPLE__)
  return "macos";
#elif defined(_WIN32)
  return "windows";
#elif defined(__linux__)
  return "linux";
#else
  return "unknown";
#endif
}

constexpr std::string_view architecture_name() {
#if defined(__aarch64__) || defined(_M_ARM64)
  return "arm64";
#elif defined(__x86_64__) || defined(_M_X64)
  return "x86_64";
#else
  return "unknown";
#endif
}

std::vector<std::uint8_t> capabilities_payload() {
  std::string output;
  output.reserve(192U);
  output.append(R"({"platform":")");
  output.append(platform_name());
  output.append(R"(","architecture":")");
  output.append(architecture_name());
  output.append(R"(","runtime_version":")");
  output.append(contracts::kRuntimeVersion);
  output.append(R"(","protocol_version":)");
  append_unsigned(output, contracts::kIpcProtocolCurrentVersion);
  output.append(R"(,"backend":"unavailable","engine":"unavailable"})");
  return payload(output);
}

message::RuntimeMessage response(
    const message::RuntimeMessage& request,
    ErrorCode error_code,
    std::vector<std::uint8_t> response_payload = {}) {
  return {
      message::MessageKind::Response,
      contracts::kIpcProtocolCurrentVersion,
      request.request_id,
      request.command,
      error_code,
      std::move(response_payload),
  };
}

std::vector<std::uint8_t> state_error_payload(ErrorCode error_code) {
  if (error_code == ErrorCode::RuntimeShuttingDown) {
    return payload(R"({"error":"runtime_shutting_down"})");
  }
  return payload(R"({"error":"runtime_unavailable"})");
}

}  // namespace

Session::Session(std::uint64_t generation)
    : snapshot_{generation, SessionState::Starting, SessionHealth::Starting, ExitReason::None} {}

SessionSnapshot Session::snapshot() const noexcept {
  return snapshot_;
}

std::optional<message::RuntimeMessage> Session::start() {
  if (snapshot_.state != SessionState::Starting) {
    return std::nullopt;
  }
  message::RuntimeMessage hello{
      message::MessageKind::Hello,
      contracts::kIpcProtocolCurrentVersion,
      0U,
      message::Command::None,
      ErrorCode::Success,
      hello_payload(snapshot_.generation),
  };
  snapshot_.state = SessionState::Running;
  snapshot_.health = SessionHealth::Healthy;
  return hello;
}

DispatchResult Session::reject_for_state(const message::RuntimeMessage& request) const {
  const auto error_code = snapshot_.state == SessionState::Stopping
                              ? ErrorCode::RuntimeShuttingDown
                              : ErrorCode::RuntimeUnavailable;
  return {
      response(request, error_code, state_error_payload(error_code)),
      DispatchDisposition::Respond,
  };
}

DispatchResult Session::protocol_failure() noexcept {
  mark_failure(ExitReason::ProtocolFailure);
  return {std::nullopt, DispatchDisposition::ProtocolFailure};
}

DispatchResult Session::handle(const message::RuntimeMessage& inbound) {
  if (inbound.kind != message::MessageKind::Request ||
      inbound.command == message::Command::None) {
    return protocol_failure();
  }
  if (snapshot_.state != SessionState::Running) {
    return reject_for_state(inbound);
  }

  switch (inbound.command) {
    case message::Command::Ping:
      return {
          response(inbound, ErrorCode::Success, inbound.payload), DispatchDisposition::Respond};
    case message::Command::GetCapabilities:
      return {
          response(inbound, ErrorCode::Success, capabilities_payload()),
          DispatchDisposition::Respond,
      };
    case message::Command::RunMockPipeline:
      return {
          response(
              inbound,
              ErrorCode::EngineUnavailable,
              payload(R"({"error":"engine_unavailable"})")),
          DispatchDisposition::Respond,
      };
    case message::Command::Shutdown: {
      auto shutdown_response = response(inbound, ErrorCode::Success);
      snapshot_.state = SessionState::Stopping;
      snapshot_.health = SessionHealth::Stopping;
      snapshot_.exit_reason = ExitReason::RequestedShutdown;
      return {std::move(shutdown_response), DispatchDisposition::StopAfterResponse};
    }
    case message::Command::None:
      break;
  }
  return protocol_failure();
}

void Session::mark_clean_eof() noexcept {
  if (snapshot_.state == SessionState::Running) {
    snapshot_.state = SessionState::Stopped;
    snapshot_.health = SessionHealth::Stopped;
    snapshot_.exit_reason = ExitReason::CleanEof;
  }
}

void Session::mark_shutdown_response_written() noexcept {
  if (snapshot_.state == SessionState::Stopping) {
    snapshot_.state = SessionState::Stopped;
    snapshot_.health = SessionHealth::Stopped;
  } else {
    mark_failure(ExitReason::UnexpectedFailure);
  }
}

void Session::mark_failure(ExitReason reason) noexcept {
  if (reason == ExitReason::None || reason == ExitReason::CleanEof ||
      reason == ExitReason::RequestedShutdown) {
    reason = ExitReason::UnexpectedFailure;
  }
  snapshot_.state = SessionState::Error;
  snapshot_.health = SessionHealth::Unhealthy;
  snapshot_.exit_reason = reason;
}

}  // namespace ai_voice::runtime
