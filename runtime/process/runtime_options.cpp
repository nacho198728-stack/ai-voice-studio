#include <ai_voice_runtime/runtime_options.hpp>

#include <charconv>
#include <system_error>

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

}  // namespace

RuntimeOptionsParseResult parse_runtime_options(
    std::span<const std::string_view> arguments) noexcept {
  try {
    RuntimeOptions options;
    bool plugin_seen = false;
    bool work_seen = false;
    for (std::size_t index = 0U; index < arguments.size();) {
      const auto option = arguments[index++];
      if (option == "--plugin") {
        if (plugin_seen || index == arguments.size()) {
          return failure("--plugin must appear once with an absolute path value");
        }
        plugin_seen = true;
        const auto value = arguments[index++];
        if (value.empty() || value.size() > kMaximumPluginPathCharacters) {
          return failure("--plugin path is empty or exceeds the fixed limit");
        }
        const auto path = std::filesystem::path(value);
        if (!path.is_absolute()) {
          return failure("--plugin requires an explicit absolute path");
        }
        options.plugin_path = path;
        continue;
      }
      if (option == "--mock-work-iterations") {
        if (work_seen || index == arguments.size()) {
          return failure("--mock-work-iterations must appear once with a decimal value");
        }
        work_seen = true;
        const auto value = arguments[index++];
        std::uint32_t parsed = 0U;
        const auto [end, error] =
            std::from_chars(value.data(), value.data() + value.size(), parsed);
        if (value.empty() || error != std::errc{} || end != value.data() + value.size() ||
            parsed > kMockMaximumWorkIterations) {
          return failure("--mock-work-iterations is not a bounded unsigned decimal value");
        }
        options.mock_work_iterations = parsed;
        continue;
      }
      return failure("unknown voice-runtime command-line option");
    }
    if (work_seen && !plugin_seen) {
      return failure("--mock-work-iterations requires --plugin");
    }
    return {std::move(options), {}};
  } catch (...) {
    return failure("voice-runtime option parsing failed unexpectedly");
  }
}

}  // namespace ai_voice::runtime
