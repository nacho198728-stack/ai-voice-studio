#include <voice_engine.h>

#include <atomic>
#include <cstdint>
#include <thread>

#ifndef AIVS_FAILURE_FIXTURE_MODE
#error "AIVS_FAILURE_FIXTURE_MODE is required"
#endif

struct aivs_voice_engine_handle {
  std::uint32_t marker;
};

namespace {

[[maybe_unused]] aivs_voice_engine_handle fixture_handle{0xA15U};
[[maybe_unused]] std::atomic<bool> blocking_entered{false};
[[maybe_unused]] std::atomic<bool> blocking_released{false};

[[maybe_unused]] aivs_error_code_t AIVS_VOICE_ENGINE_CALL fixture_initialize(
    const aivs_initialize_request_t*, aivs_initialize_result_t* result) noexcept {
  if (result != nullptr && result->struct_size >= sizeof(*result)) {
    result->engine = nullptr;
  }
#if AIVS_FAILURE_FIXTURE_MODE == 3
  return AIVS_ERROR_INTERNAL_ERROR;
#else
  if (result == nullptr || result->struct_size < sizeof(*result)) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  result->engine = &fixture_handle;
  return AIVS_ERROR_SUCCESS;
#endif
}

[[maybe_unused]] aivs_error_code_t AIVS_VOICE_ENGINE_CALL fixture_shutdown(
    aivs_voice_engine_handle_t*, const aivs_shutdown_request_t*) noexcept {
#if AIVS_FAILURE_FIXTURE_MODE == 4
  return AIVS_ERROR_INVALID_STATE;
#else
  return AIVS_ERROR_SUCCESS;
#endif
}

[[maybe_unused]] aivs_error_code_t AIVS_VOICE_ENGINE_CALL fixture_info(
    aivs_voice_engine_handle_t*, aivs_engine_info_t*) noexcept {
  return AIVS_ERROR_SUCCESS;
}

[[maybe_unused]] aivs_error_code_t AIVS_VOICE_ENGINE_CALL fixture_load(
    aivs_voice_engine_handle_t*, const aivs_load_model_request_t*) noexcept {
  return AIVS_ERROR_SUCCESS;
}

[[maybe_unused]] aivs_error_code_t AIVS_VOICE_ENGINE_CALL fixture_prepare(
    aivs_voice_engine_handle_t*,
    const aivs_prepare_stream_request_t*,
    aivs_prepare_stream_result_t*) noexcept {
  return AIVS_ERROR_SUCCESS;
}

[[maybe_unused]] aivs_error_code_t AIVS_VOICE_ENGINE_CALL fixture_process(
    aivs_voice_engine_handle_t*,
    const aivs_process_audio_request_t*,
    aivs_process_audio_result_t*) noexcept {
  return AIVS_ERROR_SUCCESS;
}

[[maybe_unused]] aivs_error_code_t AIVS_VOICE_ENGINE_CALL fixture_reset(
    aivs_voice_engine_handle_t*, const aivs_reset_request_t*, aivs_reset_result_t*) noexcept {
  return AIVS_ERROR_SUCCESS;
}

[[maybe_unused]] aivs_error_code_t AIVS_VOICE_ENGINE_CALL fixture_metrics(
    aivs_voice_engine_handle_t*,
    const aivs_get_metrics_request_t*,
    aivs_engine_metrics_t* metrics) noexcept {
#if AIVS_FAILURE_FIXTURE_MODE == 5
  blocking_entered.store(true, std::memory_order_release);
  while (!blocking_released.load(std::memory_order_acquire)) {
    std::this_thread::yield();
  }
#endif
  if (metrics != nullptr && metrics->struct_size >= sizeof(*metrics)) {
    metrics->process_call_count = 0U;
    metrics->input_frame_count = 0U;
    metrics->output_frame_count = 0U;
    metrics->process_error_count = 0U;
  }
  return AIVS_ERROR_SUCCESS;
}

}  // namespace

#if AIVS_FAILURE_FIXTURE_MODE == 5
extern "C" AIVS_VOICE_ENGINE_API void aivs_test_blocking_reset() {
  blocking_entered.store(false, std::memory_order_release);
  blocking_released.store(false, std::memory_order_release);
}

extern "C" AIVS_VOICE_ENGINE_API int aivs_test_blocking_entered() {
  return blocking_entered.load(std::memory_order_acquire) ? 1 : 0;
}

extern "C" AIVS_VOICE_ENGINE_API void aivs_test_blocking_release() {
  blocking_released.store(true, std::memory_order_release);
}
#endif

extern "C" AIVS_VOICE_ENGINE_API aivs_error_code_t AIVS_VOICE_ENGINE_CALL
aivs_voice_engine_get_api(
    const aivs_voice_engine_factory_request_t*,
    aivs_voice_engine_api_t* output,
    std::uint32_t capacity) {
  aivs_voice_engine_clear_factory_output(output, capacity);
#if AIVS_FAILURE_FIXTURE_MODE == 1
  return AIVS_ERROR_UNSUPPORTED_VOICE_ENGINE_ABI;
#else
  if (output == nullptr || capacity < AIVS_VOICE_ENGINE_API_V1_SIZE) {
    return AIVS_ERROR_BUFFER_TOO_SMALL;
  }
  output->struct_size = AIVS_VOICE_ENGINE_API_V1_SIZE;
  output->abi_version = AIVS_VOICE_ENGINE_ABI_V1_VERSION;
  output->initialize = &fixture_initialize;
  output->shutdown = &fixture_shutdown;
  output->get_engine_info = &fixture_info;
  output->load_model = &fixture_load;
  output->prepare_stream = &fixture_prepare;
#if AIVS_FAILURE_FIXTURE_MODE == 2
  output->process_audio = nullptr;
#else
  output->process_audio = &fixture_process;
#endif
  output->reset = &fixture_reset;
  output->get_metrics = &fixture_metrics;
  return AIVS_ERROR_SUCCESS;
#endif
}
