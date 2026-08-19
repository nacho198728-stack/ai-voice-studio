#ifdef NDEBUG
#undef NDEBUG
#endif

#include <cassert>
#include <cstdint>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <vector>

#include <ai_voice_contracts/generated_contracts.hpp>
#include <ai_voice_contracts/runtime_message.hpp>
#include <ai_voice_runtime/session.hpp>

namespace message = ai_voice::contracts::runtime_message;
namespace runtime = ai_voice::runtime;
using ai_voice::contracts::ErrorCode;

namespace {

class FixedPipeline final : public runtime::PipelineService {
 public:
  [[nodiscard]] bool available() const noexcept override { return true; }

  runtime::PipelineRunResult run(std::span<const std::uint8_t> request_payload) override {
    last_request.assign(request_payload.begin(), request_payload.end());
    return {ErrorCode::Success, {0x01U, 0x02U, 0x03U}};
  }

  std::vector<std::uint8_t> last_request;
};

std::vector<std::uint8_t> bytes(std::string_view value) {
  return {value.begin(), value.end()};
}

std::string text(const std::vector<std::uint8_t>& value) {
  return {value.begin(), value.end()};
}

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

void startup_emits_one_canonical_hello_and_enters_running() {
  runtime::Session session(7U);
  const auto initial = session.snapshot();
  assert(initial.generation == 7U);
  assert(initial.state == runtime::SessionState::Starting);
  assert(initial.health == runtime::SessionHealth::Starting);
  assert(initial.exit_reason == runtime::ExitReason::None);

  const auto hello = session.start();
  assert(hello.has_value());
  assert(hello->kind == message::MessageKind::Hello);
  assert(hello->protocol_version == 1U);
  assert(hello->request_id == 0U);
  assert(hello->command == message::Command::None);
  assert(hello->error_code == ErrorCode::Success);
  assert(text(hello->payload) ==
         R"({"runtime_version":"0.0.0","protocol_version":1,"generation":7,"health":"starting"})");
  assert(!session.start().has_value());

  const auto running = session.snapshot();
  assert(running.state == runtime::SessionState::Running);
  assert(running.health == runtime::SessionHealth::Healthy);
  assert(running.exit_reason == runtime::ExitReason::None);
}

void requests_before_start_are_rejected_without_advancing_state() {
  runtime::Session session(1U);
  const auto result = session.handle(request(5U, message::Command::Ping, bytes("early")));
  assert(result.disposition == runtime::DispatchDisposition::Respond);
  assert(result.response->request_id == 5U);
  assert(result.response->command == message::Command::Ping);
  assert(result.response->error_code == ErrorCode::RuntimeUnavailable);
  assert(session.snapshot().state == runtime::SessionState::Starting);
}

void ping_echoes_the_exact_bounded_payload_and_correlation() {
  runtime::Session session(1U);
  assert(session.start().has_value());
  const auto payload = std::vector<std::uint8_t>(message::kMaxPingPayloadBytes, 0xA5U);
  const auto result = session.handle(request(42U, message::Command::Ping, payload));
  assert(result.disposition == runtime::DispatchDisposition::Respond);
  assert(result.response->kind == message::MessageKind::Response);
  assert(result.response->request_id == 42U);
  assert(result.response->command == message::Command::Ping);
  assert(result.response->error_code == ErrorCode::Success);
  assert(result.response->payload == payload);
}

void capabilities_are_minimal_truthful_and_deterministic() {
  runtime::Session session(3U);
  assert(session.start().has_value());
  const auto result = session.handle(request(9U, message::Command::GetCapabilities));
  assert(result.disposition == runtime::DispatchDisposition::Respond);
  assert(result.response->request_id == 9U);
  assert(result.response->command == message::Command::GetCapabilities);
  assert(result.response->error_code == ErrorCode::Success);

#if defined(__APPLE__)
  constexpr std::string_view platform = "macos";
#elif defined(_WIN32)
  constexpr std::string_view platform = "windows";
#elif defined(__linux__)
  constexpr std::string_view platform = "linux";
#else
  constexpr std::string_view platform = "unknown";
#endif

#if defined(__aarch64__) || defined(_M_ARM64)
  constexpr std::string_view architecture = "arm64";
#elif defined(__x86_64__) || defined(_M_X64)
  constexpr std::string_view architecture = "x86_64";
#else
  constexpr std::string_view architecture = "unknown";
#endif

  const auto expected = std::string("{\"platform\":\"") + std::string(platform) +
                        "\",\"architecture\":\"" + std::string(architecture) +
                        "\",\"runtime_version\":\"0.0.0\",\"protocol_version\":1,"
                        "\"backend\":\"unavailable\",\"engine\":\"unavailable\"}";
  assert(text(result.response->payload) == expected);
}

void mock_pipeline_is_uniformly_unavailable_without_engine_behavior() {
  runtime::Session session(1U);
  assert(session.start().has_value());
  const auto result = session.handle(
      request(77U, message::Command::RunMockPipeline, bytes(R"({"frames":128})")));
  assert(result.disposition == runtime::DispatchDisposition::Respond);
  assert(result.response->request_id == 77U);
  assert(result.response->command == message::Command::RunMockPipeline);
  assert(result.response->error_code == ErrorCode::EngineUnavailable);
  assert(text(result.response->payload) == R"({"error":"engine_unavailable"})");
}

void loaded_pipeline_is_reported_truthfully_and_receives_only_control_bytes() {
  FixedPipeline pipeline;
  runtime::Session session(1U, &pipeline);
  assert(session.start().has_value());

  const auto capabilities =
      session.handle(request(76U, message::Command::GetCapabilities));
  assert(capabilities.response->error_code == ErrorCode::Success);
  assert(text(capabilities.response->payload).find(R"("backend":"mock")") !=
         std::string::npos);
  assert(text(capabilities.response->payload).find(R"("engine":"aivs-mock-v1")") !=
         std::string::npos);

  const auto pipeline_result =
      session.handle(request(77U, message::Command::RunMockPipeline, {0xA1U, 0xB2U}));
  assert(pipeline_result.response->error_code == ErrorCode::Success);
  assert(pipeline_result.response->payload == std::vector<std::uint8_t>({0x01U, 0x02U, 0x03U}));
  assert(pipeline.last_request == std::vector<std::uint8_t>({0xA1U, 0xB2U}));
}

void shutdown_is_finite_and_later_requests_are_not_processed() {
  runtime::Session session(11U);
  assert(session.start().has_value());
  const auto shutdown = session.handle(request(100U, message::Command::Shutdown));
  assert(shutdown.disposition == runtime::DispatchDisposition::StopAfterResponse);
  assert(shutdown.response->request_id == 100U);
  assert(shutdown.response->command == message::Command::Shutdown);
  assert(shutdown.response->error_code == ErrorCode::Success);
  assert(shutdown.response->payload.empty());

  const auto stopping = session.snapshot();
  assert(stopping.state == runtime::SessionState::Stopping);
  assert(stopping.health == runtime::SessionHealth::Stopping);
  assert(stopping.exit_reason == runtime::ExitReason::RequestedShutdown);

  const auto late = session.handle(request(101U, message::Command::Ping, bytes("ignored")));
  assert(late.disposition == runtime::DispatchDisposition::Respond);
  assert(late.response->request_id == 101U);
  assert(late.response->command == message::Command::Ping);
  assert(late.response->error_code == ErrorCode::RuntimeShuttingDown);
  assert(late.response->payload != bytes("ignored"));
  assert(session.snapshot().state == runtime::SessionState::Stopping);

  session.mark_shutdown_response_written();
  const auto stopped = session.snapshot();
  assert(stopped.state == runtime::SessionState::Stopped);
  assert(stopped.health == runtime::SessionHealth::Stopped);
  assert(stopped.exit_reason == runtime::ExitReason::RequestedShutdown);
}

void invalid_inbound_direction_causes_a_protocol_failure_without_response() {
  const std::vector<message::RuntimeMessage> invalid_messages{
      {
          message::MessageKind::Hello,
          ai_voice::contracts::kIpcProtocolCurrentVersion,
          0U,
          message::Command::None,
          ErrorCode::Success,
          bytes("peer"),
      },
      {
          message::MessageKind::Response,
          ai_voice::contracts::kIpcProtocolCurrentVersion,
          3U,
          message::Command::Ping,
          ErrorCode::Success,
          bytes("pong"),
      },
  };
  for (const auto& inbound : invalid_messages) {
    runtime::Session session(1U);
    assert(session.start().has_value());
    const auto result = session.handle(inbound);
    assert(result.disposition == runtime::DispatchDisposition::ProtocolFailure);
    assert(!result.response.has_value());
    const auto snapshot = session.snapshot();
    assert(snapshot.state == runtime::SessionState::Error);
    assert(snapshot.health == runtime::SessionHealth::Unhealthy);
    assert(snapshot.exit_reason == runtime::ExitReason::ProtocolFailure);
  }
}

void clean_eof_and_failures_have_distinct_terminal_snapshots() {
  runtime::Session eof_session(1U);
  assert(eof_session.start().has_value());
  eof_session.mark_clean_eof();
  assert(eof_session.snapshot().state == runtime::SessionState::Stopped);
  assert(eof_session.snapshot().health == runtime::SessionHealth::Stopped);
  assert(eof_session.snapshot().exit_reason == runtime::ExitReason::CleanEof);

  runtime::Session output_session(1U);
  assert(output_session.start().has_value());
  output_session.mark_failure(runtime::ExitReason::OutputFailure);
  assert(output_session.snapshot().state == runtime::SessionState::Error);
  assert(output_session.snapshot().health == runtime::SessionHealth::Unhealthy);
  assert(output_session.snapshot().exit_reason == runtime::ExitReason::OutputFailure);
}

}  // namespace

int main() {
  startup_emits_one_canonical_hello_and_enters_running();
  requests_before_start_are_rejected_without_advancing_state();
  ping_echoes_the_exact_bounded_payload_and_correlation();
  capabilities_are_minimal_truthful_and_deterministic();
  mock_pipeline_is_uniformly_unavailable_without_engine_behavior();
  loaded_pipeline_is_reported_truthfully_and_receives_only_control_bytes();
  shutdown_is_finite_and_later_requests_are_not_processed();
  invalid_inbound_direction_causes_a_protocol_failure_without_response();
  clean_eof_and_failures_have_distinct_terminal_snapshots();
}
