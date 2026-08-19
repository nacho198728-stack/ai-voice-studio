#pragma once

#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <memory>
#include <optional>
#include <string>
#include <string_view>

namespace ai_voice::runtime {

inline constexpr std::size_t kMaximumLogComponentBytes = 64U;
inline constexpr std::size_t kMaximumLogMessageBytes = 512U;
inline constexpr std::size_t kMaximumLogDiagnosticBytes = 256U;
inline constexpr std::string_view kRuntimeLogFileName = "voice-runtime.jsonl";

enum class LogLevel {
  Trace,
  Debug,
  Info,
  Warn,
  Error,
};

[[nodiscard]] std::optional<LogLevel> parse_log_level(std::string_view value) noexcept;
[[nodiscard]] std::string_view log_level_name(LogLevel level) noexcept;

class LoggingPolicy final {
 public:
  [[nodiscard]] static std::optional<LoggingPolicy> create(
      bool debug_enabled,
      LogLevel level) noexcept;
  [[nodiscard]] static constexpr LoggingPolicy production() noexcept {
    return LoggingPolicy(false, LogLevel::Info);
  }

  [[nodiscard]] constexpr bool debug_enabled() const noexcept {
    return debug_enabled_;
  }
  [[nodiscard]] constexpr LogLevel level() const noexcept {
    return level_;
  }

 private:
  constexpr LoggingPolicy(bool debug_enabled, LogLevel level) noexcept
      : debug_enabled_(debug_enabled), level_(level) {}

  bool debug_enabled_;
  LogLevel level_;
};

struct LogFields {
  std::optional<std::uint64_t> request_id;
  std::optional<std::uint64_t> generation;
};

class LogSink {
 public:
  virtual ~LogSink() = default;
  virtual void log(
      LogLevel level,
      std::string_view message,
      LogFields fields = {}) noexcept = 0;
  virtual void flush() noexcept = 0;
};

struct RuntimeLoggingConfig {
  std::filesystem::path directory;
  LoggingPolicy policy;
  std::string component;
  std::optional<std::uint64_t> generation{std::nullopt};
};

struct RuntimeLoggerInitialization;

class RuntimeLogger final : public LogSink {
 public:
  ~RuntimeLogger() override;
  RuntimeLogger(RuntimeLogger&&) noexcept;
  RuntimeLogger& operator=(RuntimeLogger&&) noexcept;
  RuntimeLogger(const RuntimeLogger&) = delete;
  RuntimeLogger& operator=(const RuntimeLogger&) = delete;

  [[nodiscard]] static RuntimeLoggerInitialization initialize(
      RuntimeLoggingConfig config) noexcept;

  void log(
      LogLevel level,
      std::string_view message,
      LogFields fields = {}) noexcept override;
  void flush() noexcept override;

 private:
  class Impl;
  explicit RuntimeLogger(std::unique_ptr<Impl> implementation) noexcept;
  std::unique_ptr<Impl> implementation_;
};

struct RuntimeLoggerInitialization {
  std::unique_ptr<RuntimeLogger> logger;
  std::string diagnostic;
};

}  // namespace ai_voice::runtime
