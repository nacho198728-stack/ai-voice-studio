#ifdef NDEBUG
#undef NDEBUG
#endif

#include <array>
#include <cassert>
#include <chrono>
#include <cstdint>
#include <cstring>
#include <filesystem>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include <ai_voice_runtime/voice_engine_loader.hpp>

#if defined(_WIN32)
#include <windows.h>
#else
#include <dlfcn.h>
#endif

namespace runtime = ai_voice::runtime;
using ai_voice::contracts::ErrorCode;

namespace {

bool module_is_loaded(const std::filesystem::path& path) {
#if defined(_WIN32)
  HMODULE module = nullptr;
  return GetModuleHandleExW(
             GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, path.c_str(), &module) != 0;
#else
  void* module = dlopen(path.c_str(), RTLD_NOW | RTLD_NOLOAD);
  if (module == nullptr) {
    return false;
  }
  assert(dlclose(module) == 0);
  return true;
#endif
}

class BlockingControls {
 public:
  using VoidFunction = void (*)();
  using IntFunction = int (*)();

  explicit BlockingControls(const std::filesystem::path& path) {
#if defined(_WIN32)
    handle_ = static_cast<void*>(LoadLibraryW(path.c_str()));
#else
    handle_ = dlopen(path.c_str(), RTLD_NOW | RTLD_LOCAL);
#endif
    assert(handle_ != nullptr);
    reset = symbol<VoidFunction>("aivs_test_blocking_reset");
    entered = symbol<IntFunction>("aivs_test_blocking_entered");
    release = symbol<VoidFunction>("aivs_test_blocking_release");
  }

  ~BlockingControls() { close(); }

  void close() {
    if (handle_ == nullptr) {
      return;
    }
#if defined(_WIN32)
    assert(FreeLibrary(static_cast<HMODULE>(handle_)) != 0);
#else
    assert(dlclose(handle_) == 0);
#endif
    handle_ = nullptr;
  }

  VoidFunction reset{};
  IntFunction entered{};
  VoidFunction release{};

 private:
  template <typename Function>
  Function symbol(const char* name) {
#if defined(_WIN32)
    const auto raw = GetProcAddress(static_cast<HMODULE>(handle_), name);
#else
    const auto raw = dlsym(handle_, name);
#endif
    assert(raw != nullptr);
    Function result{};
    static_assert(sizeof(result) == sizeof(raw));
    std::memcpy(&result, &raw, sizeof(result));
    return result;
  }

  void* handle_{nullptr};
};

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

  auto native_with_nul = plugin_path.native();
  native_with_nul.push_back(std::filesystem::path::value_type{});
  native_with_nul.append(
      {static_cast<std::filesystem::path::value_type>('x'),
       static_cast<std::filesystem::path::value_type>('y')});
  const auto embedded_nul =
      runtime::VoiceEngineModule::load(std::filesystem::path(native_with_nul));
  assert(embedded_nul.error_code == ErrorCode::InvalidArgument);
  assert(!embedded_nul.module);
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

void malformed_factories_and_initialize_failures_are_rejected_and_unloaded(
    const std::filesystem::path& missing_factory,
    const std::filesystem::path& unsupported_factory,
    const std::filesystem::path& incomplete_factory,
    const std::filesystem::path& initialize_failure) {
  const std::array cases{
      std::pair{missing_factory, ErrorCode::EngineUnavailable},
      std::pair{unsupported_factory, ErrorCode::UnsupportedVoiceEngineAbi},
      std::pair{incomplete_factory, ErrorCode::UnsupportedVoiceEngineAbi},
  };
  for (const auto& [path, expected] : cases) {
    assert(!module_is_loaded(path));
    const auto result = runtime::VoiceEngineModule::load(path);
    assert(result.error_code == expected);
    assert(!result.module);
    assert(!result.diagnostic.empty());
    assert(!module_is_loaded(path));
  }

  assert(!module_is_loaded(initialize_failure));
  auto loaded = runtime::VoiceEngineModule::load(initialize_failure);
  assert(loaded.error_code == ErrorCode::Success);
  assert(module_is_loaded(initialize_failure));
  const auto initialized = loaded.module->initialize({});
  assert(initialized.error_code == ErrorCode::InternalError);
  assert(!initialized.instance);
  loaded.module.reset();
  assert(!module_is_loaded(initialize_failure));
}

void failed_destructor_shutdown_retains_the_module(
    const std::filesystem::path& shutdown_failure) {
  assert(!module_is_loaded(shutdown_failure));
  auto loaded = runtime::VoiceEngineModule::load(shutdown_failure);
  assert(loaded.error_code == ErrorCode::Success);
  auto initialized = loaded.module->initialize({});
  assert(initialized.error_code == ErrorCode::Success);
  loaded.module.reset();
  assert(module_is_loaded(shutdown_failure));
  initialized.instance.reset();
  assert(module_is_loaded(shutdown_failure));
}

void instance_and_inflight_call_pin_module_until_successful_shutdown(
    const std::filesystem::path& blocking_metrics) {
  assert(!module_is_loaded(blocking_metrics));
  auto loaded = runtime::VoiceEngineModule::load(blocking_metrics);
  assert(loaded.error_code == ErrorCode::Success);
  auto initialized = loaded.module->initialize({});
  assert(initialized.error_code == ErrorCode::Success);
  loaded.module.reset();
  assert(module_is_loaded(blocking_metrics));

  BlockingControls controls(blocking_metrics);
  controls.reset();
  aivs_get_metrics_request_t request{
      sizeof(aivs_get_metrics_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
  aivs_engine_metrics_t metrics{
      sizeof(aivs_engine_metrics_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, 0U, 0U, 0U, 0U, {0U, 0U}};
  ErrorCode call_result = ErrorCode::InternalError;
  std::thread caller([&] { call_result = initialized.instance->get_metrics(&request, &metrics); });
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
  while (controls.entered() == 0 && std::chrono::steady_clock::now() < deadline) {
    std::this_thread::yield();
  }
  assert(controls.entered() == 1);
  controls.close();
  assert(module_is_loaded(blocking_metrics));
  controls.release();
  caller.join();
  assert(call_result == ErrorCode::Success);

  aivs_shutdown_request_t shutdown_request{
      sizeof(aivs_shutdown_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
  assert(initialized.instance->shutdown(&shutdown_request) == ErrorCode::Success);
  initialized.instance.reset();
  assert(!module_is_loaded(blocking_metrics));
}

}  // namespace

int main(int argc, char** argv) {
  assert(argc == 8);
  const auto plugin_path = std::filesystem::absolute(std::filesystem::path(argv[1]));
  const auto missing_factory = std::filesystem::absolute(std::filesystem::path(argv[2]));
  const auto unsupported_factory = std::filesystem::absolute(std::filesystem::path(argv[3]));
  const auto incomplete_factory = std::filesystem::absolute(std::filesystem::path(argv[4]));
  const auto initialize_failure = std::filesystem::absolute(std::filesystem::path(argv[5]));
  const auto shutdown_failure = std::filesystem::absolute(std::filesystem::path(argv[6]));
  const auto blocking_metrics = std::filesystem::absolute(std::filesystem::path(argv[7]));
  explicit_paths_and_factory_failures_are_bounded(plugin_path, missing_factory);
  module_owns_a_retryable_engine_handle(plugin_path);
  malformed_factories_and_initialize_failures_are_rejected_and_unloaded(
      missing_factory, unsupported_factory, incomplete_factory, initialize_failure);
  failed_destructor_shutdown_retains_the_module(shutdown_failure);
  instance_and_inflight_call_pin_module_until_successful_shutdown(blocking_metrics);
}
