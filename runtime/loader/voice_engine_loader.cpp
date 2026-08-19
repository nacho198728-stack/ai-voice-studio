#include <ai_voice_runtime/voice_engine_loader.hpp>

#include <algorithm>
#include <cstring>
#include <new>
#include <string_view>
#include <utility>

#if defined(_WIN32)
#include <windows.h>
#else
#include <dlfcn.h>
#endif

namespace ai_voice::runtime {
namespace {

using contracts::ErrorCode;

static_assert(std::atomic<std::uint32_t>::is_always_lock_free);

std::string bounded(std::string_view text) {
  return std::string(text.substr(0U, kMaximumLoaderDiagnosticBytes));
}

ErrorCode from_abi(aivs_error_code_t code) noexcept {
  const auto mapped = contracts::error_code_from_value(code);
  return mapped.value_or(ErrorCode::InternalError);
}

#if defined(_WIN32)
void close_library(void* handle) noexcept {
  if (handle != nullptr) {
    static_cast<void>(FreeLibrary(static_cast<HMODULE>(handle)));
  }
}

std::string platform_error(std::string_view prefix) {
  return bounded(std::string(prefix) + ": Windows error " + std::to_string(GetLastError()));
}
#else
void close_library(void* handle) noexcept {
  if (handle != nullptr) {
    static_cast<void>(dlclose(handle));
  }
}

std::string platform_error(std::string_view prefix) {
  const char* detail = dlerror();
  std::string result(prefix);
  if (detail != nullptr) {
    result.append(": ");
    result.append(detail);
  }
  return bounded(result);
}
#endif

}  // namespace

VoiceEngineModule::VoiceEngineModule(void* native_handle, aivs_voice_engine_api_t api) noexcept
    : native_handle_(native_handle), api_(api) {}

VoiceEngineModule::~VoiceEngineModule() {
  if (!abandoned_ && in_flight_.load(std::memory_order_acquire) == 0U) {
    close_library(native_handle_);
  }
}

VoiceEngineLoadResult VoiceEngineModule::load(
    const std::filesystem::path& explicit_path) noexcept {
  try {
    if (explicit_path.empty() || !explicit_path.is_absolute() ||
        explicit_path.native().size() > kMaximumPluginPathCharacters) {
      return {nullptr, ErrorCode::InvalidArgument, "VoiceEngine path must be an explicit bounded absolute path"};
    }

    void* handle = nullptr;
#if defined(_WIN32)
    handle = static_cast<void*>(LoadLibraryW(explicit_path.c_str()));
#else
    static_cast<void>(dlerror());
    handle = dlopen(explicit_path.c_str(), RTLD_NOW | RTLD_LOCAL);
#endif
    if (handle == nullptr) {
      return {nullptr, ErrorCode::EngineUnavailable, platform_error("could not load VoiceEngine")};
    }

#if defined(_WIN32)
    const auto symbol = GetProcAddress(static_cast<HMODULE>(handle), "aivs_voice_engine_get_api");
#else
    static_cast<void>(dlerror());
    const auto symbol = dlsym(handle, "aivs_voice_engine_get_api");
#endif
    if (symbol == nullptr) {
      const auto diagnostic = platform_error("VoiceEngine factory export is missing");
      close_library(handle);
      return {nullptr, ErrorCode::EngineUnavailable, diagnostic};
    }
    aivs_voice_engine_get_api_fn factory{};
    static_assert(sizeof(factory) == sizeof(symbol));
    std::memcpy(&factory, &symbol, sizeof(factory));

    auto request = aivs_voice_engine_factory_request_t AIVS_VOICE_ENGINE_FACTORY_REQUEST_INIT;
    auto api = aivs_voice_engine_api_t AIVS_VOICE_ENGINE_API_OUTPUT_INIT;
    aivs_error_code_t factory_result = AIVS_ERROR_INTERNAL_ERROR;
    try {
      factory_result = factory(&request, &api, sizeof(api));
    } catch (...) {
      close_library(handle);
      return {nullptr, ErrorCode::InternalError, "VoiceEngine factory threw across the C boundary"};
    }
    const auto mapped = from_abi(factory_result);
    if (mapped != ErrorCode::Success ||
        aivs_voice_engine_api_is_complete_for_version(&api, api.abi_version) != AIVS_TRUE) {
      close_library(handle);
      return {
          nullptr,
          mapped == ErrorCode::Success ? ErrorCode::UnsupportedVoiceEngineAbi : mapped,
          "VoiceEngine factory negotiation failed",
      };
    }
    return {
        std::shared_ptr<VoiceEngineModule>(new VoiceEngineModule(handle, api)),
        ErrorCode::Success,
        {},
    };
  } catch (...) {
    return {nullptr, ErrorCode::InternalError, "VoiceEngine loader failed unexpectedly"};
  }
}

VoiceEngineCreateResult VoiceEngineModule::initialize(
    std::span<const std::uint8_t> configuration) noexcept {
  aivs_initialize_request_t request = AIVS_INITIALIZE_REQUEST_INIT;
  request.configuration_utf8.data = configuration.empty() ? nullptr : configuration.data();
  request.configuration_utf8.size_bytes = configuration.size();
  aivs_initialize_result_t result = AIVS_INITIALIZE_RESULT_INIT;
  const auto status = invoke([&] { return api_.initialize(&request, &result); });
  if (status != ErrorCode::Success) {
    return {nullptr, status};
  }
  if (result.engine == nullptr) {
    return {nullptr, ErrorCode::InternalError};
  }
  try {
    return {
        std::unique_ptr<VoiceEngineInstance>(
            new VoiceEngineInstance(shared_from_this(), result.engine)),
        ErrorCode::Success,
    };
  } catch (...) {
    aivs_shutdown_request_t shutdown = {
        sizeof(aivs_shutdown_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
    if (invoke([&] { return api_.shutdown(result.engine, &shutdown); }) !=
        ErrorCode::Success) {
      abandon();
    }
    return {nullptr, ErrorCode::InternalError};
  }
}

void VoiceEngineModule::begin_call() noexcept {
  in_flight_.fetch_add(1U, std::memory_order_acq_rel);
}

void VoiceEngineModule::end_call() noexcept {
  in_flight_.fetch_sub(1U, std::memory_order_acq_rel);
}

void VoiceEngineModule::abandon() noexcept {
  abandoned_ = true;
}

VoiceEngineInstance::VoiceEngineInstance(
    std::shared_ptr<VoiceEngineModule> module, aivs_voice_engine_handle_t* engine) noexcept
    : module_(std::move(module)), engine_(engine) {}

VoiceEngineInstance::~VoiceEngineInstance() {
  if (engine_ == nullptr) {
    return;
  }
  aivs_shutdown_request_t request = {
      sizeof(aivs_shutdown_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
  if (shutdown(&request) != ErrorCode::Success) {
    module_->abandon();
  }
}

bool VoiceEngineInstance::is_live() const noexcept {
  return engine_ != nullptr;
}

ErrorCode VoiceEngineInstance::shutdown(const aivs_shutdown_request_t* request) noexcept {
  if (engine_ == nullptr) {
    return ErrorCode::InvalidState;
  }
  const auto status = module_->invoke([&] { return module_->api_.shutdown(engine_, request); });
  if (status == ErrorCode::Success) {
    engine_ = nullptr;
  }
  return status;
}

ErrorCode VoiceEngineInstance::get_engine_info(aivs_engine_info_t* info) noexcept {
  if (engine_ == nullptr) {
    return ErrorCode::InvalidState;
  }
  return module_->invoke([&] { return module_->api_.get_engine_info(engine_, info); });
}

ErrorCode VoiceEngineInstance::load_model(const aivs_load_model_request_t* request) noexcept {
  if (engine_ == nullptr) {
    return ErrorCode::InvalidState;
  }
  return module_->invoke([&] { return module_->api_.load_model(engine_, request); });
}

ErrorCode VoiceEngineInstance::prepare_stream(
    const aivs_prepare_stream_request_t* request,
    aivs_prepare_stream_result_t* result) noexcept {
  if (engine_ == nullptr) {
    return ErrorCode::InvalidState;
  }
  return module_->invoke([&] { return module_->api_.prepare_stream(engine_, request, result); });
}

ErrorCode VoiceEngineInstance::process_audio(
    const aivs_process_audio_request_t* request,
    aivs_process_audio_result_t* result) noexcept {
  if (engine_ == nullptr) {
    return ErrorCode::InvalidState;
  }
  return module_->invoke([&] { return module_->api_.process_audio(engine_, request, result); });
}

ErrorCode VoiceEngineInstance::reset(
    const aivs_reset_request_t* request, aivs_reset_result_t* result) noexcept {
  if (engine_ == nullptr) {
    return ErrorCode::InvalidState;
  }
  return module_->invoke([&] { return module_->api_.reset(engine_, request, result); });
}

ErrorCode VoiceEngineInstance::get_metrics(
    const aivs_get_metrics_request_t* request, aivs_engine_metrics_t* metrics) noexcept {
  if (engine_ == nullptr) {
    return ErrorCode::InvalidState;
  }
  return module_->invoke([&] { return module_->api_.get_metrics(engine_, request, metrics); });
}

}  // namespace ai_voice::runtime
