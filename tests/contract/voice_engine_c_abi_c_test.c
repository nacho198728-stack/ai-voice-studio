#include <assert.h>
#include <float.h>
#include <stdint.h>
#include <string.h>

#include "voice_engine_abi_v1_layout.h"

static aivs_error_code_t stub_initialize(
    const aivs_initialize_request_t* request, aivs_initialize_result_t* result) {
  (void)request;
  (void)result;
  return AIVS_ERROR_SUCCESS;
}

static aivs_error_code_t stub_shutdown(
    aivs_voice_engine_handle_t* engine, const aivs_shutdown_request_t* request) {
  (void)engine;
  (void)request;
  return AIVS_ERROR_SUCCESS;
}

static aivs_error_code_t stub_info(aivs_voice_engine_handle_t* engine, aivs_engine_info_t* info) {
  (void)engine;
  (void)info;
  return AIVS_ERROR_SUCCESS;
}

static aivs_error_code_t stub_load(
    aivs_voice_engine_handle_t* engine, const aivs_load_model_request_t* request) {
  (void)engine;
  (void)request;
  return AIVS_ERROR_SUCCESS;
}

static aivs_error_code_t stub_prepare(
    aivs_voice_engine_handle_t* engine,
    const aivs_prepare_stream_request_t* request,
    aivs_prepare_stream_result_t* result) {
  (void)engine;
  (void)request;
  (void)result;
  return AIVS_ERROR_SUCCESS;
}

static aivs_error_code_t stub_process(
    aivs_voice_engine_handle_t* engine,
    const aivs_process_audio_request_t* request,
    aivs_process_audio_result_t* result) {
  (void)engine;
  (void)request;
  (void)result;
  return AIVS_ERROR_SUCCESS;
}

static aivs_error_code_t stub_reset(
    aivs_voice_engine_handle_t* engine,
    const aivs_reset_request_t* request,
    aivs_reset_result_t* result) {
  (void)engine;
  (void)request;
  (void)result;
  return AIVS_ERROR_SUCCESS;
}

static aivs_error_code_t stub_metrics(
    aivs_voice_engine_handle_t* engine,
    const aivs_get_metrics_request_t* request,
    aivs_engine_metrics_t* metrics) {
  (void)engine;
  (void)request;
  (void)metrics;
  return AIVS_ERROR_SUCCESS;
}

_Static_assert(sizeof(float) == 4U, "PCM must use four-byte float storage");
_Static_assert(sizeof(aivs_error_code_t) == 4U, "error code width must remain canonical");
_Static_assert(
    AIVS_VOICE_ENGINE_ABI_V1_VERSION == 1U,
    "the frozen v1 ABI constant must not follow mutable current"
);
_Static_assert(FLT_RADIX == 2, "PCM must use binary radix");
_Static_assert(FLT_MANT_DIG == 24, "PCM must use binary32 precision");
_Static_assert(FLT_MAX_EXP == 128 && FLT_MIN_EXP == -125, "PCM must use binary32 exponent range");
_Static_assert(_Generic(&aivs_voice_engine_get_api, aivs_voice_engine_get_api_fn : 1, default : 0), "factory type drifted");
_Static_assert(_Generic(((aivs_voice_engine_api_t*)0)->initialize, aivs_initialize_fn : 1, default : 0), "initialize type drifted");
_Static_assert(_Generic(((aivs_voice_engine_api_t*)0)->shutdown, aivs_shutdown_fn : 1, default : 0), "shutdown type drifted");
_Static_assert(_Generic(((aivs_voice_engine_api_t*)0)->get_engine_info, aivs_get_engine_info_fn : 1, default : 0), "info type drifted");
_Static_assert(_Generic(((aivs_voice_engine_api_t*)0)->load_model, aivs_load_model_fn : 1, default : 0), "load type drifted");
_Static_assert(_Generic(((aivs_voice_engine_api_t*)0)->prepare_stream, aivs_prepare_stream_fn : 1, default : 0), "prepare type drifted");
_Static_assert(_Generic(((aivs_voice_engine_api_t*)0)->process_audio, aivs_process_audio_fn : 1, default : 0), "process type drifted");
_Static_assert(_Generic(((aivs_voice_engine_api_t*)0)->reset, aivs_reset_fn : 1, default : 0), "reset type drifted");
_Static_assert(_Generic(((aivs_voice_engine_api_t*)0)->get_metrics, aivs_get_metrics_fn : 1, default : 0), "metrics type drifted");

static void assert_factory_clear(uint32_t capacity) {
  union {
    aivs_voice_engine_api_t api;
    uint8_t bytes[sizeof(aivs_voice_engine_api_t) + 16U];
  } storage;
  uint32_t index;
  uint32_t cleared = capacity < AIVS_VOICE_ENGINE_API_V1_SIZE
      ? capacity
      : AIVS_VOICE_ENGINE_API_V1_SIZE;

  memset(storage.bytes, 0xA5, sizeof(storage.bytes));
  aivs_voice_engine_clear_factory_output(&storage.api, capacity);
  for (index = 0U; index < cleared; ++index) {
    assert(storage.bytes[index] == 0U);
  }
  for (index = cleared; index < sizeof(storage.bytes); ++index) {
    assert(storage.bytes[index] == 0xA5U);
  }
}

int main(void) {
  uint32_t selected = UINT32_C(99);
  uint64_t bytes = UINT64_C(99);
  aivs_voice_engine_api_t api = AIVS_VOICE_ENGINE_API_OUTPUT_INIT;
  aivs_voice_engine_factory_request_t future_host = AIVS_VOICE_ENGINE_FACTORY_REQUEST_INIT;
  aivs_process_audio_result_t result = AIVS_PROCESS_AUDIO_RESULT_INIT;
  aivs_process_audio_result_t before_result;
  float samples[8] = {0.0F};
  float before_samples[8];

  assert(!aivs_voice_engine_api_v1_capacity_is_sufficient(AIVS_VOICE_ENGINE_API_V1_SIZE - 1U));
  assert(aivs_voice_engine_api_v1_capacity_is_sufficient(AIVS_VOICE_ENGINE_API_V1_SIZE));
  assert(aivs_voice_engine_api_v1_capacity_is_sufficient(AIVS_VOICE_ENGINE_API_V1_SIZE + 64U));
  assert(aivs_voice_engine_select_abi_version(1U, 1U, 2U, 2U, &selected) == AIVS_ERROR_UNSUPPORTED_VOICE_ENGINE_ABI);
  assert(selected == 0U);
  assert(aivs_voice_engine_select_abi_version(1U, 4U, 2U, 3U, &selected) == AIVS_ERROR_SUCCESS);
  assert(selected == 3U);
  future_host.maximum_abi_version = 2U;
  assert(future_host.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(aivs_voice_engine_select_abi_version(
      future_host.minimum_abi_version, future_host.maximum_abi_version,
      AIVS_VOICE_ENGINE_ABI_V1_VERSION, AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      &selected) == AIVS_ERROR_SUCCESS);
  assert(selected == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(aivs_voice_engine_pcm_byte_count_is_addressable(0U, 1U, &bytes));
  assert(bytes == 0U);
  assert(aivs_voice_engine_generation_advance(0U, &bytes) == AIVS_ERROR_SUCCESS);
  assert(bytes == 1U);
  assert(aivs_voice_engine_generation_advance(UINT64_MAX, &bytes) == AIVS_ERROR_INVALID_STATE);

  aivs_voice_engine_clear_factory_output(NULL, 0U);
  assert_factory_clear(0U);
  assert_factory_clear(7U);
  assert_factory_clear(8U);
  assert_factory_clear(16U);
  assert_factory_clear(AIVS_VOICE_ENGINE_API_V1_SIZE - 1U);
  assert_factory_clear(AIVS_VOICE_ENGINE_API_V1_SIZE);
  assert_factory_clear(AIVS_VOICE_ENGINE_API_V1_SIZE + 8U);

  assert(aivs_voice_engine_pcm_pointers_match_counts(NULL, 0U, NULL, 0U));
  assert(aivs_voice_engine_pcm_pointers_match_counts(samples, 1U, samples, 1U));
  assert(!aivs_voice_engine_pcm_pointers_match_counts(NULL, 1U, samples, 1U));
  assert(aivs_voice_engine_pcm_ranges_are_compatible(samples, samples, 8U));
  assert(!aivs_voice_engine_pcm_ranges_are_compatible(samples, samples + 1, 8U));
  assert(aivs_voice_engine_pcm_ranges_are_compatible(samples, samples + 4, 8U));
  assert(!aivs_voice_engine_pcm_ranges_are_compatible(
      (const float*)(uintptr_t)(UINTPTR_MAX - 2U), samples, 4U));
  assert(!aivs_voice_engine_frame_capacity_is_sufficient(2U, 1U));
  assert(aivs_voice_engine_pcm_byte_count_is_addressable(UINT64_MAX / 4U, 1U, &bytes));
  assert(bytes == UINT64_MAX - 3U);
  assert(!aivs_voice_engine_pcm_byte_count_is_addressable(UINT64_MAX, 2U, &bytes));
  assert(bytes == 0U);

  api.abi_version = AIVS_VOICE_ENGINE_ABI_V1_VERSION;
  api.initialize = stub_initialize;
  api.shutdown = stub_shutdown;
  api.get_engine_info = stub_info;
  api.load_model = stub_load;
  api.prepare_stream = stub_prepare;
  api.process_audio = stub_process;
  api.reset = stub_reset;
  api.get_metrics = stub_metrics;
  assert(aivs_voice_engine_api_v1_is_complete(&api));
  assert(aivs_voice_engine_api_is_complete_for_version(&api, AIVS_VOICE_ENGINE_ABI_V1_VERSION));
  assert(!aivs_voice_engine_api_is_complete_for_version(&api, AIVS_VOICE_ENGINE_ABI_V1_VERSION + 1U));
  api.reserved[0] = 1U;
  assert(!aivs_voice_engine_api_v1_is_complete(&api));
  api.reserved[0] = 0U;
  api.reset = 0;
  assert(!aivs_voice_engine_api_v1_is_complete(&api));

  result.output.samples = samples;
  result.output.frame_capacity = 8U;
  result.output.sample_rate_hz = 48000U;
  result.output.channel_count = 2U;
  result.output.format = AIVS_PCM_FORMAT_FLOAT32;
  result.output.layout = AIVS_PCM_LAYOUT_INTERLEAVED;
  result.output.frames_written_or_required = 7U;
  result.output.sequence = 42U;
  result.output.sample_time = 99U;
  result.processed_frame_count = 7U;
  result.stream_generation = 9U;
  memcpy(before_samples, samples, sizeof(samples));
  before_result = result;
  aivs_voice_engine_process_result_set_failure(&result, AIVS_ERROR_INVALID_ARGUMENT, 7U);
  assert(memcmp(samples, before_samples, sizeof(samples)) == 0);
  assert(memcmp(&result.output, &before_result.output, offsetof(aivs_pcm_mutable_buffer_t, frames_written_or_required)) == 0);
  assert(result.output.frames_written_or_required == 0U);
  assert(result.processed_frame_count == 0U);
  assert(result.stream_generation == 0U);
  aivs_voice_engine_process_result_set_failure(&result, AIVS_ERROR_BUFFER_TOO_SMALL, 7U);
  assert(result.output.frames_written_or_required == 7U);

  assert(AIVS_ERROR_SUCCESS == 0);
  assert(AIVS_ERROR_UNSUPPORTED_VOICE_ENGINE_ABI == 1001);
  assert(AIVS_ERROR_INVALID_ARGUMENT == 1301);
  assert(AIVS_ERROR_INVALID_STATE == 1302);
  assert(AIVS_ERROR_BUFFER_TOO_SMALL == 1303);

  return 0;
}
