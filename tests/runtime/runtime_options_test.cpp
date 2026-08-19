#ifdef NDEBUG
#undef NDEBUG
#endif

#include <array>
#include <cassert>
#include <filesystem>
#include <string>
#include <string_view>

#include <ai_voice_runtime/runtime_options.hpp>

namespace runtime = ai_voice::runtime;

namespace {

void empty_arguments_preserve_the_engine_unavailable_mode() {
  const auto result = runtime::parse_runtime_options({});
  assert(result.options.has_value());
  assert(!result.options->plugin_path.has_value());
  assert(result.options->mock_work_iterations == 0U);
  assert(result.diagnostic.empty());
}

void explicit_absolute_plugin_and_bounded_work_are_accepted_in_either_order() {
  const auto plugin = std::filesystem::absolute("mock-plugin").string();
  const std::array<std::string_view, 4> arguments{
      "--mock-work-iterations", "1000000", "--plugin", plugin};
  const auto result = runtime::parse_runtime_options(arguments);
  assert(result.options.has_value());
  assert(result.options->plugin_path == std::filesystem::path(plugin));
  assert(result.options->mock_work_iterations == 1'000'000U);
  assert(result.diagnostic.empty());
}

void malformed_unknown_duplicate_or_implicit_inputs_are_rejected() {
  const auto plugin = std::filesystem::absolute("mock-plugin").string();
  const std::array invalid_cases{
      std::array<std::string_view, 4>{"--plugin", plugin, "--plugin", plugin},
      std::array<std::string_view, 4>{"--plugin", "relative", "--mock-work-iterations", "0"},
      std::array<std::string_view, 4>{"--plugin", plugin, "--unknown", "0"},
      std::array<std::string_view, 4>{"--plugin", plugin, "--mock-work-iterations", "-1"},
      std::array<std::string_view, 4>{"--plugin", plugin, "--mock-work-iterations", "1000001"},
      std::array<std::string_view, 4>{"--mock-work-iterations", "1", "--unknown", "x"},
  };
  for (const auto& arguments : invalid_cases) {
    const auto result = runtime::parse_runtime_options(arguments);
    assert(!result.options.has_value());
    assert(!result.diagnostic.empty());
    assert(result.diagnostic.size() <= runtime::kMaximumRuntimeOptionDiagnosticBytes);
  }

  const std::array<std::string_view, 1> missing_value{"--plugin"};
  assert(!runtime::parse_runtime_options(missing_value).options.has_value());
  const std::array<std::string_view, 2> work_without_plugin{"--mock-work-iterations", "1"};
  assert(!runtime::parse_runtime_options(work_without_plugin).options.has_value());
}

}  // namespace

int main() {
  empty_arguments_preserve_the_engine_unavailable_mode();
  explicit_absolute_plugin_and_bounded_work_are_accepted_in_either_order();
  malformed_unknown_duplicate_or_implicit_inputs_are_rejected();
}
