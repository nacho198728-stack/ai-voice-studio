#pragma once

#include <cstdint>
#include <optional>

#include <ai_voice_contracts/runtime_message.hpp>

namespace ai_voice::runtime {

enum class SessionState {
  Starting,
  Running,
  Stopping,
  Stopped,
  Error,
};

enum class SessionHealth {
  Starting,
  Healthy,
  Stopping,
  Stopped,
  Unhealthy,
};

enum class ExitReason {
  None,
  CleanEof,
  RequestedShutdown,
  ProtocolFailure,
  OutputFailure,
  UnexpectedFailure,
};

struct SessionSnapshot {
  std::uint64_t generation;
  SessionState state;
  SessionHealth health;
  ExitReason exit_reason;

  bool operator==(const SessionSnapshot&) const = default;
};

enum class DispatchDisposition {
  Respond,
  StopAfterResponse,
  ProtocolFailure,
};

struct DispatchResult {
  std::optional<contracts::runtime_message::RuntimeMessage> response;
  DispatchDisposition disposition;
};

class Session {
 public:
  explicit Session(std::uint64_t generation);

  [[nodiscard]] SessionSnapshot snapshot() const noexcept;
  [[nodiscard]] std::optional<contracts::runtime_message::RuntimeMessage> start();
  [[nodiscard]] DispatchResult handle(
      const contracts::runtime_message::RuntimeMessage& inbound);

  void mark_clean_eof() noexcept;
  void mark_shutdown_response_written() noexcept;
  void mark_failure(ExitReason reason) noexcept;

 private:
  [[nodiscard]] DispatchResult reject_for_state(
      const contracts::runtime_message::RuntimeMessage& request) const;
  [[nodiscard]] DispatchResult protocol_failure() noexcept;

  SessionSnapshot snapshot_;
};

}  // namespace ai_voice::runtime
