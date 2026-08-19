#include <ai_voice_runtime/runtime_options.hpp>

#include <limits>
#include <type_traits>

#include <ai_voice_runtime/mock_pipeline.hpp>
#include <ai_voice_runtime/voice_engine_loader.hpp>

namespace ai_voice::runtime {
namespace {

RuntimeOptionsParseResult failure(std::string_view diagnostic) {
  return {
      std::nullopt,
      std::string(diagnostic.substr(0U, kMaximumRuntimeOptionDiagnosticBytes)),
  };
}

template <typename Character>
bool contains_nul(std::basic_string_view<Character> value) noexcept {
  return value.find(Character{}) != std::basic_string_view<Character>::npos;
}

template <typename Character>
bool equals_ascii(std::basic_string_view<Character> value, std::string_view ascii) noexcept {
  if (value.size() != ascii.size()) {
    return false;
  }
  for (std::size_t index = 0U; index < value.size(); ++index) {
    if (value[index] != static_cast<Character>(ascii[index])) {
      return false;
    }
  }
  return true;
}

bool append_utf8(std::string& output, std::uint32_t code_point) {
  if (code_point <= 0x7FU) {
    output.push_back(static_cast<char>(code_point));
  } else if (code_point <= 0x7FFU) {
    output.push_back(static_cast<char>(0xC0U | (code_point >> 6U)));
    output.push_back(static_cast<char>(0x80U | (code_point & 0x3FU)));
  } else if (code_point <= 0xFFFFU) {
    if (code_point >= 0xD800U && code_point <= 0xDFFFU) {
      return false;
    }
    output.push_back(static_cast<char>(0xE0U | (code_point >> 12U)));
    output.push_back(static_cast<char>(0x80U | ((code_point >> 6U) & 0x3FU)));
    output.push_back(static_cast<char>(0x80U | (code_point & 0x3FU)));
  } else if (code_point <= 0x10FFFFU) {
    output.push_back(static_cast<char>(0xF0U | (code_point >> 18U)));
    output.push_back(static_cast<char>(0x80U | ((code_point >> 12U) & 0x3FU)));
    output.push_back(static_cast<char>(0x80U | ((code_point >> 6U) & 0x3FU)));
    output.push_back(static_cast<char>(0x80U | (code_point & 0x3FU)));
  } else {
    return false;
  }
  return true;
}

std::optional<std::string> wide_to_utf8(std::wstring_view value) {
  std::string result;
  result.reserve(value.size());
  for (std::size_t index = 0U; index < value.size(); ++index) {
    std::uint32_t code_point = static_cast<std::uint32_t>(value[index]);
    if constexpr (sizeof(wchar_t) == 2U) {
      if (code_point >= 0xD800U && code_point <= 0xDBFFU) {
        if (++index == value.size()) {
          return std::nullopt;
        }
        const auto low = static_cast<std::uint32_t>(value[index]);
        if (low < 0xDC00U || low > 0xDFFFU) {
          return std::nullopt;
        }
        code_point = 0x10000U + ((code_point - 0xD800U) << 10U) + (low - 0xDC00U);
      } else if (code_point >= 0xDC00U && code_point <= 0xDFFFU) {
        return std::nullopt;
      }
    }
    if (!append_utf8(result, code_point)) {
      return std::nullopt;
    }
  }
  return result;
}

std::optional<std::filesystem::path> make_path(std::string_view value) {
  return std::filesystem::path(std::string(value));
}

std::optional<std::filesystem::path> make_path(std::wstring_view value) {
  const auto utf8 = wide_to_utf8(value);
  if (!utf8.has_value()) {
    return std::nullopt;
  }
#if defined(_WIN32)
  return std::filesystem::path(std::wstring(value));
#else
  return std::filesystem::path(*utf8);
#endif
}

template <typename Character>
std::optional<std::uint32_t> parse_work_iterations(
    std::basic_string_view<Character> value) noexcept {
  if (value.empty()) {
    return std::nullopt;
  }
  std::uint64_t parsed = 0U;
  for (const auto character : value) {
    if (character < static_cast<Character>('0') || character > static_cast<Character>('9')) {
      return std::nullopt;
    }
    parsed = parsed * 10U + static_cast<std::uint64_t>(character - static_cast<Character>('0'));
    if (parsed > kMockMaximumWorkIterations) {
      return std::nullopt;
    }
  }
  return static_cast<std::uint32_t>(parsed);
}

template <typename Character>
std::optional<std::uint64_t> parse_generation(
    std::basic_string_view<Character> value) noexcept {
  if (value.empty()) {
    return std::nullopt;
  }
  std::uint64_t parsed = 0U;
  for (const auto character : value) {
    if (character < static_cast<Character>('0') || character > static_cast<Character>('9')) {
      return std::nullopt;
    }
    const auto digit = static_cast<std::uint64_t>(character - static_cast<Character>('0'));
    if (parsed > (std::numeric_limits<std::uint64_t>::max() - digit) / 10U) {
      return std::nullopt;
    }
    parsed = parsed * 10U + digit;
  }
  return parsed == 0U ? std::nullopt : std::optional<std::uint64_t>(parsed);
}

template <typename Character>
std::optional<LogLevel> parse_level(std::basic_string_view<Character> value) noexcept {
  if (equals_ascii(value, "trace")) {
    return LogLevel::Trace;
  }
  if (equals_ascii(value, "debug")) {
    return LogLevel::Debug;
  }
  if (equals_ascii(value, "info")) {
    return LogLevel::Info;
  }
  if (equals_ascii(value, "warn")) {
    return LogLevel::Warn;
  }
  if (equals_ascii(value, "error")) {
    return LogLevel::Error;
  }
  return std::nullopt;
}

template <typename Character>
RuntimeOptionsParseResult parse_options(
    std::span<const std::basic_string_view<Character>> arguments) {
  RuntimeOptions options;
  bool plugin_seen = false;
  bool work_seen = false;
  bool log_directory_seen = false;
  bool log_level_seen = false;
  bool generation_seen = false;
  for (std::size_t index = 0U; index < arguments.size();) {
    const auto option = arguments[index++];
    if (contains_nul(option)) {
      return failure("command-line arguments must not contain embedded NUL");
    }
    if (equals_ascii(option, "--plugin")) {
      if (plugin_seen || index == arguments.size()) {
        return failure("--plugin must appear once with an absolute path value");
      }
      plugin_seen = true;
      const auto value = arguments[index++];
      if (value.empty() || value.size() > kMaximumPluginPathCharacters || contains_nul(value)) {
        return failure("--plugin path is empty, contains NUL, or exceeds the fixed limit");
      }
      const auto path = make_path(value);
      if (!path.has_value() || !path->is_absolute()) {
        return failure("--plugin requires a valid explicit absolute Unicode path");
      }
      options.plugin_path = *path;
      continue;
    }
    if (equals_ascii(option, "--mock-work-iterations")) {
      if (work_seen || index == arguments.size()) {
        return failure("--mock-work-iterations must appear once with a decimal value");
      }
      work_seen = true;
      const auto value = arguments[index++];
      if (contains_nul(value)) {
        return failure("--mock-work-iterations contains embedded NUL");
      }
      const auto parsed = parse_work_iterations(value);
      if (!parsed.has_value()) {
        return failure("--mock-work-iterations is not a bounded unsigned decimal value");
      }
      options.mock_work_iterations = *parsed;
      continue;
    }
    if (equals_ascii(option, "--log-directory")) {
      if (log_directory_seen || index == arguments.size()) {
        return failure("--log-directory must appear once with an absolute path value");
      }
      log_directory_seen = true;
      const auto value = arguments[index++];
      if (value.empty() || value.size() > kMaximumPluginPathCharacters || contains_nul(value)) {
        return failure("--log-directory is empty, contains NUL, or exceeds the fixed limit");
      }
      const auto path = make_path(value);
      if (!path.has_value() || !path->is_absolute()) {
        return failure("--log-directory requires a valid explicit absolute Unicode path");
      }
      options.log_directory = *path;
      continue;
    }
    if (equals_ascii(option, "--log-level")) {
      if (log_level_seen || index == arguments.size()) {
        return failure("--log-level must appear once with a supported value");
      }
      log_level_seen = true;
      const auto value = arguments[index++];
      if (contains_nul(value)) {
        return failure("--log-level contains embedded NUL");
      }
      const auto parsed = parse_level(value);
      if (!parsed.has_value()) {
        return failure("--log-level must be trace, debug, info, warn, or error");
      }
      options.log_level = *parsed;
      continue;
    }
    if (equals_ascii(option, "--generation")) {
      if (generation_seen || index == arguments.size()) {
        return failure("--generation must appear once with a nonzero unsigned value");
      }
      generation_seen = true;
      const auto value = arguments[index++];
      if (contains_nul(value)) {
        return failure("--generation contains embedded NUL");
      }
      const auto parsed = parse_generation(value);
      if (!parsed.has_value()) {
        return failure("--generation is not a nonzero unsigned decimal value");
      }
      options.generation = *parsed;
      continue;
    }
    return failure("unknown voice-runtime command-line option");
  }
  if (work_seen && !plugin_seen) {
    return failure("--mock-work-iterations requires --plugin");
  }
  if (!log_directory_seen || !log_level_seen || !generation_seen) {
    return failure(
        "--log-directory, --log-level, and --generation are required explicit inputs");
  }
  return {std::move(options), {}};
}

}  // namespace

RuntimeOptionsParseResult parse_runtime_options(
    std::span<const std::string_view> arguments) noexcept {
  try {
    return parse_options(arguments);
  } catch (...) {
    return failure("voice-runtime option parsing failed unexpectedly");
  }
}

RuntimeOptionsParseResult parse_runtime_options(
    std::span<const std::wstring_view> arguments) noexcept {
  try {
    return parse_options(arguments);
  } catch (...) {
    return failure("voice-runtime wide option parsing failed unexpectedly");
  }
}

}  // namespace ai_voice::runtime
