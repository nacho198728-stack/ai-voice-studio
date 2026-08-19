#pragma once

#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <optional>
#include <span>
#include <string>
#include <string_view>

#include <ai_voice_runtime/logging.hpp>

namespace ai_voice::runtime {

inline constexpr std::size_t kMaximumRuntimeOptionDiagnosticBytes = 256U;

struct RuntimeOptions {
  std::optional<std::filesystem::path> plugin_path;
  std::uint32_t mock_work_iterations{0U};
  std::filesystem::path log_directory;
  LoggingPolicy logging_policy{LoggingPolicy::production()};
  std::uint64_t generation{0U};
};

struct RuntimeOptionsParseResult {
  std::optional<RuntimeOptions> options;
  std::string diagnostic;
};

RuntimeOptionsParseResult parse_runtime_options(
    std::span<const std::string_view> arguments) noexcept;
RuntimeOptionsParseResult parse_runtime_options(
    std::span<const std::wstring_view> arguments) noexcept;

}  // namespace ai_voice::runtime
