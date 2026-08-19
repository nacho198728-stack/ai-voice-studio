#pragma once

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <memory>
#include <span>
#include <string>

#include <ai_voice_contracts/generated_contracts.hpp>
#include <voice_engine.h>

namespace ai_voice::runtime {

inline constexpr std::size_t kMaximumLoaderDiagnosticBytes = 512U;
inline constexpr std::size_t kMaximumPluginPathCharacters = 4096U;

class VoiceEngineModule;
class VoiceEngineInstance;
struct VoiceEngineLoadResult;
struct VoiceEngineCreateResult;

class VoiceEngineModule final : public std::enable_shared_from_this<VoiceEngineModule> {
 public:
  static VoiceEngineLoadResult load(const std::filesystem::path& explicit_path) noexcept;

  VoiceEngineModule(const VoiceEngineModule&) = delete;
  VoiceEngineModule& operator=(const VoiceEngineModule&) = delete;
  ~VoiceEngineModule();

  VoiceEngineCreateResult initialize(std::span<const std::uint8_t> configuration) noexcept;

 private:
  friend class VoiceEngineInstance;

  VoiceEngineModule(void* native_handle, aivs_voice_engine_api_t api) noexcept;
  template <typename Call>
  contracts::ErrorCode invoke(Call&& call) noexcept {
    begin_call();
    contracts::ErrorCode result = contracts::ErrorCode::InternalError;
    try {
      const auto mapped = contracts::error_code_from_value(call());
      result = mapped.value_or(contracts::ErrorCode::InternalError);
    } catch (...) {
      result = contracts::ErrorCode::InternalError;
    }
    end_call();
    return result;
  }
  void begin_call() noexcept;
  void end_call() noexcept;
  void abandon() noexcept;

  void* native_handle_{nullptr};
  aivs_voice_engine_api_t api_ AIVS_VOICE_ENGINE_API_OUTPUT_INIT;
  std::atomic<std::uint32_t> in_flight_{0U};
  bool abandoned_{false};
};

class VoiceEngineInstance final {
 public:
  VoiceEngineInstance(const VoiceEngineInstance&) = delete;
  VoiceEngineInstance& operator=(const VoiceEngineInstance&) = delete;
  ~VoiceEngineInstance();

  [[nodiscard]] bool is_live() const noexcept;

  contracts::ErrorCode shutdown(const aivs_shutdown_request_t* request) noexcept;
  contracts::ErrorCode get_engine_info(aivs_engine_info_t* info) noexcept;
  contracts::ErrorCode load_model(const aivs_load_model_request_t* request) noexcept;
  contracts::ErrorCode prepare_stream(
      const aivs_prepare_stream_request_t* request,
      aivs_prepare_stream_result_t* result) noexcept;
  contracts::ErrorCode process_audio(
      const aivs_process_audio_request_t* request,
      aivs_process_audio_result_t* result) noexcept;
  contracts::ErrorCode reset(
      const aivs_reset_request_t* request, aivs_reset_result_t* result) noexcept;
  contracts::ErrorCode get_metrics(
      const aivs_get_metrics_request_t* request, aivs_engine_metrics_t* metrics) noexcept;

 private:
  friend class VoiceEngineModule;

  VoiceEngineInstance(
      std::shared_ptr<VoiceEngineModule> module, aivs_voice_engine_handle_t* engine) noexcept;

  std::shared_ptr<VoiceEngineModule> module_;
  aivs_voice_engine_handle_t* engine_{nullptr};
};

struct VoiceEngineLoadResult {
  std::shared_ptr<VoiceEngineModule> module;
  contracts::ErrorCode error_code;
  std::string diagnostic;
};

struct VoiceEngineCreateResult {
  std::unique_ptr<VoiceEngineInstance> instance;
  contracts::ErrorCode error_code;
};

}  // namespace ai_voice::runtime
