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

void logging_and_generation_are_required_explicit_process_inputs() {
  const auto result = runtime::parse_runtime_options(std::span<const std::string_view>{});
  assert(!result.options.has_value());
  assert(!result.diagnostic.empty());
}

void explicit_absolute_plugin_and_bounded_work_are_accepted_in_either_order() {
  const auto plugin = std::filesystem::absolute("mock-plugin").string();
  const auto logs = std::filesystem::absolute("logs-声音").string();
  const std::array<std::string_view, 12> arguments{
      "--mock-work-iterations", "1000000", "--plugin", plugin,
      "--log-directory", logs, "--log-level", "debug", "--debug-enabled", "true",
      "--generation", "9"};
  const auto result = runtime::parse_runtime_options(arguments);
  assert(result.options.has_value());
  assert(result.options->plugin_path == std::filesystem::path(plugin));
  assert(result.options->mock_work_iterations == 1'000'000U);
  assert(result.options->log_directory == std::filesystem::path(logs));
  assert(result.options->logging_policy.level() == runtime::LogLevel::Debug);
  assert(result.options->logging_policy.debug_enabled());
  assert(result.options->generation == 9U);
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

  const auto logs = std::filesystem::absolute("logs").string();
  const std::array<std::string_view, 6> missing_debug_authority{
      "--log-directory", logs, "--log-level", "info", "--generation", "1"};
  assert(!runtime::parse_runtime_options(missing_debug_authority).options.has_value());
  const std::array<std::string_view, 8> bypass{
      "--log-directory", logs, "--log-level", "debug", "--debug-enabled", "false",
      "--generation", "1"};
  const auto bypass_result = runtime::parse_runtime_options(bypass);
  assert(!bypass_result.options.has_value());
  assert(bypass_result.diagnostic.find("debug") != std::string::npos);
}

void wide_arguments_preserve_unicode_paths_and_reject_embedded_nul() {
#if defined(_WIN32)
  constexpr std::wstring_view unicode_path = L"C:\\tmp\\AI-Voice-声音\\mock-engine";
  constexpr std::u8string_view expected_path = u8"C:/tmp/AI-Voice-声音/mock-engine";
#else
  constexpr std::wstring_view unicode_path = L"/tmp/AI-Voice-声音/mock-engine";
  constexpr std::u8string_view expected_path = u8"/tmp/AI-Voice-声音/mock-engine";
#endif
  const auto log_path = std::filesystem::absolute("日志-开发").wstring();
  const std::array<std::wstring_view, 12> arguments{
      L"--plugin", unicode_path, L"--mock-work-iterations", L"42",
      L"--log-directory", log_path, L"--log-level", L"info", L"--debug-enabled", L"false",
      L"--generation", L"3"};
  const auto result = runtime::parse_runtime_options(arguments);
  assert(result.options.has_value());
  assert(result.options->plugin_path.has_value());
  assert(result.options->plugin_path->generic_u8string() == expected_path);
  assert(result.options->mock_work_iterations == 42U);
  assert(result.options->log_directory == std::filesystem::path(log_path));
  assert(result.options->generation == 3U);

  const std::wstring embedded_nul = std::wstring(L"/tmp/plugin") + L'\0' + L"suffix";
  const std::array<std::wstring_view, 2> invalid{
      L"--plugin", std::wstring_view(embedded_nul.data(), embedded_nul.size())};
  const auto rejected = runtime::parse_runtime_options(invalid);
  assert(!rejected.options.has_value());
  assert(!rejected.diagnostic.empty());
}

}  // namespace

int main() {
  logging_and_generation_are_required_explicit_process_inputs();
  explicit_absolute_plugin_and_bounded_work_are_accepted_in_either_order();
  malformed_unknown_duplicate_or_implicit_inputs_are_rejected();
  wide_arguments_preserve_unicode_paths_and_reject_embedded_nul();
}
