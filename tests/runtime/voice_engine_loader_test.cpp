#ifdef NDEBUG
#undef NDEBUG
#endif

#include <cassert>
#include <cstdint>
#include <filesystem>
#include <string>
#include <vector>

#include <ai_voice_runtime/voice_engine_loader.hpp>

namespace runtime = ai_voice::runtime;
using ai_voice::contracts::ErrorCode;

namespace {

void explicit_paths_and_factory_failures_are_bounded(
    const std::filesystem::path& plugin_path, const std::filesystem::path& wrong_library) {
  const auto relative = runtime::VoiceEngineModule::load(plugin_path.filename());
  assert(relative.error_code == ErrorCode::InvalidArgument);
  assert(!relative.module);
  assert(!relative.diagnostic.empty());
  assert(relative.diagnostic.size() <= runtime::kMaximumLoaderDiagnosticBytes);

  const auto missing = runtime::VoiceEngineModule::load(plugin_path.parent_path() / "missing-plugin");
  assert(missing.error_code == ErrorCode::EngineUnavailable);
  assert(!missing.module);
  assert(!missing.diagnostic.empty());
  assert(missing.diagnostic.size() <= runtime::kMaximumLoaderDiagnosticBytes);

  const auto wrong = runtime::VoiceEngineModule::load(wrong_library);
  assert(wrong.error_code == ErrorCode::EngineUnavailable);
  assert(!wrong.module);
  assert(!wrong.diagnostic.empty());
  assert(wrong.diagnostic.size() <= runtime::kMaximumLoaderDiagnosticBytes);
}

void module_owns_a_retryable_engine_handle(const std::filesystem::path& plugin_path) {
  auto loaded = runtime::VoiceEngineModule::load(plugin_path);
  assert(loaded.error_code == ErrorCode::Success);
  assert(loaded.module);
  assert(loaded.diagnostic.empty());

  const std::vector<std::uint8_t> malformed_configuration{'{', '}'};
  auto rejected = loaded.module->initialize(malformed_configuration);
  assert(rejected.error_code == ErrorCode::InvalidArgument);
  assert(!rejected.instance);

  const std::string config = R"({"work_iterations":0})";
  auto created = loaded.module->initialize(
      std::vector<std::uint8_t>(config.begin(), config.end()));
  assert(created.error_code == ErrorCode::Success);
  assert(created.instance);
  assert(created.instance->is_live());

  aivs_get_metrics_request_t metrics_request{
      sizeof(aivs_get_metrics_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
  aivs_engine_metrics_t metrics{
      sizeof(aivs_engine_metrics_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, 99U, 99U, 99U, 99U, {0U, 0U}};
  assert(created.instance->get_metrics(&metrics_request, &metrics) == ErrorCode::Success);
  assert(metrics.process_call_count == 0U);

  aivs_shutdown_request_t invalid_shutdown{
      sizeof(aivs_shutdown_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {1U, 0U}};
  assert(created.instance->shutdown(&invalid_shutdown) == ErrorCode::InvalidArgument);
  assert(created.instance->is_live());
  assert(created.instance->get_metrics(&metrics_request, &metrics) == ErrorCode::Success);

  aivs_shutdown_request_t shutdown{
      sizeof(aivs_shutdown_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
  assert(created.instance->shutdown(&shutdown) == ErrorCode::Success);
  assert(!created.instance->is_live());
  assert(created.instance->shutdown(&shutdown) == ErrorCode::InvalidState);
}

}  // namespace

int main(int argc, char** argv) {
  assert(argc == 3);
  const auto plugin_path = std::filesystem::absolute(std::filesystem::path(argv[1]));
  const auto wrong_library = std::filesystem::absolute(std::filesystem::path(argv[2]));
  explicit_paths_and_factory_failures_are_bounded(plugin_path, wrong_library);
  module_owns_a_retryable_engine_handle(plugin_path);
}
