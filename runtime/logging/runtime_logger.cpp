#include <ai_voice_runtime/logging.hpp>

#include <algorithm>
#include <array>
#include <charconv>
#include <chrono>
#include <cstdio>
#include <ctime>
#include <system_error>
#include <utility>
#include <vector>

#include <spdlog/logger.h>
#include <spdlog/sinks/basic_file_sink.h>
#include <spdlog/sinks/stdout_sinks.h>

namespace ai_voice::runtime {
namespace {

std::string bounded_diagnostic(std::string_view value) {
  return std::string(value.substr(0U, kMaximumLogDiagnosticBytes));
}

std::size_t valid_utf8_sequence_length(std::string_view value, std::size_t offset) noexcept {
  const auto available = value.size() - offset;
  const auto first = static_cast<unsigned char>(value[offset]);
  const auto continuation = [&](std::size_t index) {
    return index < available &&
           (static_cast<unsigned char>(value[offset + index]) & 0xC0U) == 0x80U;
  };
  if (first <= 0x7FU) {
    return 1U;
  }
  if (first >= 0xC2U && first <= 0xDFU && continuation(1U)) {
    return 2U;
  }
  if (available >= 3U && continuation(1U) && continuation(2U) &&
      ((first == 0xE0U && static_cast<unsigned char>(value[offset + 1U]) >= 0xA0U) ||
       (first >= 0xE1U && first <= 0xECU) ||
       (first == 0xEDU && static_cast<unsigned char>(value[offset + 1U]) <= 0x9FU) ||
       (first >= 0xEEU && first <= 0xEFU))) {
    return 3U;
  }
  if (available >= 4U && continuation(1U) && continuation(2U) && continuation(3U) &&
      ((first == 0xF0U && static_cast<unsigned char>(value[offset + 1U]) >= 0x90U) ||
       (first >= 0xF1U && first <= 0xF3U) ||
       (first == 0xF4U && static_cast<unsigned char>(value[offset + 1U]) <= 0x8FU))) {
    return 4U;
  }
  return 0U;
}

bool is_valid_utf8(std::string_view value) noexcept {
  for (std::size_t offset = 0U; offset < value.size();) {
    const auto length = valid_utf8_sequence_length(value, offset);
    if (length == 0U) {
      return false;
    }
    offset += length;
  }
  return true;
}

std::string sanitize_bounded_utf8(std::string_view value, std::size_t maximum) {
  constexpr std::string_view replacement = "\xEF\xBF\xBD";
  std::string output;
  output.reserve(std::min(value.size(), maximum));
  for (std::size_t offset = 0U; offset < value.size();) {
    const auto length = valid_utf8_sequence_length(value, offset);
    const auto emitted_length = length == 0U ? replacement.size() : length;
    if (output.size() + emitted_length > maximum) {
      break;
    }
    if (length == 0U) {
      output.append(replacement);
      ++offset;
    } else {
      output.append(value.substr(offset, length));
      offset += length;
    }
  }
  return output;
}

void append_json_string(std::string& output, std::string_view value) {
  constexpr char hex[] = "0123456789abcdef";
  output.push_back('"');
  for (const auto character : value) {
    const auto byte = static_cast<unsigned char>(character);
    switch (character) {
      case '"':
        output.append("\\\"");
        break;
      case '\\':
        output.append("\\\\");
        break;
      case '\b':
        output.append("\\b");
        break;
      case '\f':
        output.append("\\f");
        break;
      case '\n':
        output.append("\\n");
        break;
      case '\r':
        output.append("\\r");
        break;
      case '\t':
        output.append("\\t");
        break;
      default:
        if (byte < 0x20U) {
          output.append("\\u00");
          output.push_back(hex[(byte >> 4U) & 0x0FU]);
          output.push_back(hex[byte & 0x0FU]);
        } else {
          output.push_back(character);
        }
        break;
    }
  }
  output.push_back('"');
}

void append_unsigned(std::string& output, std::uint64_t value) {
  std::array<char, 32> digits{};
  const auto [end, error] =
      std::to_chars(digits.data(), digits.data() + digits.size(), value);
  if (error == std::errc{}) {
    output.append(digits.data(), end);
  }
}

std::string timestamp_utc() {
  const auto now = std::chrono::system_clock::now();
  const auto seconds = std::chrono::time_point_cast<std::chrono::seconds>(now);
  const auto subseconds =
      std::chrono::duration_cast<std::chrono::microseconds>(now - seconds).count();
  const auto time = std::chrono::system_clock::to_time_t(seconds);
  std::tm utc{};
#if defined(_WIN32)
  if (::gmtime_s(&utc, &time) != 0) {
    return {};
  }
#else
  if (::gmtime_r(&time, &utc) == nullptr) {
    return {};
  }
#endif
  std::array<char, 40> text{};
  const auto count = std::snprintf(
      text.data(),
      text.size(),
      "%04d-%02d-%02dT%02d:%02d:%02d.%06lldZ",
      utc.tm_year + 1900,
      utc.tm_mon + 1,
      utc.tm_mday,
      utc.tm_hour,
      utc.tm_min,
      utc.tm_sec,
      static_cast<long long>(subseconds));
  if (count <= 0 || static_cast<std::size_t>(count) >= text.size()) {
    return {};
  }
  return std::string(text.data(), static_cast<std::size_t>(count));
}

spdlog::level::level_enum spdlog_level(LogLevel level) noexcept {
  switch (level) {
    case LogLevel::Trace:
      return spdlog::level::trace;
    case LogLevel::Debug:
      return spdlog::level::debug;
    case LogLevel::Info:
      return spdlog::level::info;
    case LogLevel::Warn:
      return spdlog::level::warn;
    case LogLevel::Error:
      return spdlog::level::err;
  }
  return spdlog::level::off;
}

}  // namespace

std::optional<LogLevel> parse_log_level(std::string_view value) noexcept {
  if (value == "trace") {
    return LogLevel::Trace;
  }
  if (value == "debug") {
    return LogLevel::Debug;
  }
  if (value == "info") {
    return LogLevel::Info;
  }
  if (value == "warn") {
    return LogLevel::Warn;
  }
  if (value == "error") {
    return LogLevel::Error;
  }
  return std::nullopt;
}

std::string_view log_level_name(LogLevel level) noexcept {
  switch (level) {
    case LogLevel::Trace:
      return "trace";
    case LogLevel::Debug:
      return "debug";
    case LogLevel::Info:
      return "info";
    case LogLevel::Warn:
      return "warn";
    case LogLevel::Error:
      return "error";
  }
  return "error";
}

std::optional<LoggingPolicy> LoggingPolicy::create(
    bool debug_enabled,
    LogLevel level) noexcept {
  switch (level) {
    case LogLevel::Trace:
    case LogLevel::Debug:
    case LogLevel::Info:
    case LogLevel::Warn:
    case LogLevel::Error:
      break;
    default:
      return std::nullopt;
  }
  if (!debug_enabled && (level == LogLevel::Trace || level == LogLevel::Debug)) {
    return std::nullopt;
  }
  return LoggingPolicy(debug_enabled, level);
}

class RuntimeLogger::Impl {
 public:
  Impl(
      std::shared_ptr<spdlog::logger> logger,
      std::string component,
      std::optional<std::uint64_t> generation)
      : logger_(std::move(logger)),
        component_(std::move(component)),
        generation_(generation) {}

  std::shared_ptr<spdlog::logger> logger_;
  std::string component_;
  std::optional<std::uint64_t> generation_;
};

RuntimeLogger::RuntimeLogger(std::unique_ptr<Impl> implementation) noexcept
    : implementation_(std::move(implementation)) {}

RuntimeLogger::~RuntimeLogger() {
  flush();
}

RuntimeLogger::RuntimeLogger(RuntimeLogger&&) noexcept = default;
RuntimeLogger& RuntimeLogger::operator=(RuntimeLogger&&) noexcept = default;

RuntimeLoggerInitialization RuntimeLogger::initialize(RuntimeLoggingConfig config) noexcept {
  try {
    if (!config.directory.is_absolute() || config.component.empty() ||
        config.component.size() > kMaximumLogComponentBytes ||
        !is_valid_utf8(config.component)) {
      return {
          nullptr,
          bounded_diagnostic(
              "runtime logging requires an absolute directory and bounded valid UTF-8 component"),
      };
    }
    std::error_code error;
    std::filesystem::create_directories(config.directory, error);
    if (error || !std::filesystem::is_directory(config.directory, error) || error) {
      return {nullptr, bounded_diagnostic("cannot create configured runtime log directory")};
    }
    const auto file_path = config.directory / kRuntimeLogFileName;
#if defined(_WIN32)
    auto file_sink =
        std::make_shared<spdlog::sinks::basic_file_sink_mt>(file_path.native(), false);
#else
    auto file_sink =
        std::make_shared<spdlog::sinks::basic_file_sink_mt>(file_path.string(), false);
#endif
    auto stderr_sink = std::make_shared<spdlog::sinks::stderr_sink_mt>();
    std::vector<spdlog::sink_ptr> sinks{file_sink, stderr_sink};
    auto logger = std::make_shared<spdlog::logger>("voice-runtime", sinks.begin(), sinks.end());
    // spdlog's default handler writes exception details (including sink paths)
    // directly to stderr. Late sink failures are intentionally contained: the
    // handler is instance-owned, non-throwing, and performs no I/O that could
    // recurse into the failing sink or corrupt stdout IPC.
    logger->set_error_handler([](const std::string&) noexcept {});
    logger->set_pattern("%v");
    logger->set_level(spdlog_level(config.policy.level()));
    return {
        std::unique_ptr<RuntimeLogger>(new RuntimeLogger(std::make_unique<Impl>(
            std::move(logger), std::move(config.component), config.generation))),
        {},
    };
  } catch (...) {
    return {nullptr, bounded_diagnostic("cannot open runtime log file in configured directory")};
  }
}

void RuntimeLogger::log(
    LogLevel level,
    std::string_view message,
    LogFields fields) noexcept {
  try {
    if (!implementation_ || !implementation_->logger_->should_log(spdlog_level(level))) {
      return;
    }
    const auto timestamp = timestamp_utc();
    if (timestamp.empty()) {
      return;
    }
    std::string record;
    record.reserve(192U + std::min(message.size(), kMaximumLogMessageBytes));
    record.append(R"({"timestamp":)");
    append_json_string(record, timestamp);
    record.append(R"(,"component":)");
    append_json_string(record, implementation_->component_);
    record.append(R"(,"level":)");
    append_json_string(record, log_level_name(level));
    record.append(R"(,"message":)");
    append_json_string(record, sanitize_bounded_utf8(message, kMaximumLogMessageBytes));
    if (implementation_->generation_.has_value()) {
      fields.generation = implementation_->generation_;
    }
    if (fields.request_id.has_value()) {
      record.append(R"(,"request_id":)");
      append_unsigned(record, *fields.request_id);
    }
    if (fields.generation.has_value()) {
      record.append(R"(,"generation":)");
      append_unsigned(record, *fields.generation);
    }
    record.push_back('}');
    implementation_->logger_->log(spdlog_level(level), record);
  } catch (...) {
  }
}

void RuntimeLogger::flush() noexcept {
  try {
    if (implementation_) {
      implementation_->logger_->flush();
    }
  } catch (...) {
  }
}

}  // namespace ai_voice::runtime
