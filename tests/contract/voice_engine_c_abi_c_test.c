#include <assert.h>
#include <float.h>
#include <stdint.h>

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

int main(void) {
  uint32_t selected = UINT32_C(99);
  uint64_t bytes = UINT64_C(99);
  aivs_voice_engine_api_t api = AIVS_VOICE_ENGINE_API_OUTPUT_INIT;

  assert(!aivs_voice_engine_api_v1_capacity_is_sufficient(AIVS_VOICE_ENGINE_API_V1_SIZE - 1U));
  assert(aivs_voice_engine_api_v1_capacity_is_sufficient(AIVS_VOICE_ENGINE_API_V1_SIZE));
  assert(aivs_voice_engine_api_v1_capacity_is_sufficient(AIVS_VOICE_ENGINE_API_V1_SIZE + 64U));
  assert(aivs_voice_engine_select_abi_version(1U, 1U, 2U, 2U, &selected) == AIVS_ERROR_UNSUPPORTED_VOICE_ENGINE_ABI);
  assert(selected == 0U);
  assert(aivs_voice_engine_select_abi_version(1U, 4U, 2U, 3U, &selected) == AIVS_ERROR_SUCCESS);
  assert(selected == 3U);
  assert(aivs_voice_engine_pcm_byte_count_is_addressable(0U, 1U, &bytes));
  assert(bytes == 0U);
  assert(aivs_voice_engine_pcm_byte_count_is_addressable(UINT64_MAX / 4U, 1U, &bytes));
  assert(bytes == UINT64_MAX - 3U);
  assert(!aivs_voice_engine_pcm_byte_count_is_addressable(UINT64_MAX, 2U, &bytes));
  assert(bytes == 0U);

  api.abi_version = AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION;
  api.initialize = stub_initialize;
  api.shutdown = stub_shutdown;
  api.get_engine_info = stub_info;
  api.load_model = stub_load;
  api.prepare_stream = stub_prepare;
  api.process_audio = stub_process;
  api.reset = stub_reset;
  api.get_metrics = stub_metrics;
  assert(aivs_voice_engine_api_v1_is_complete(&api));
  api.reserved[0] = 1U;
  assert(!aivs_voice_engine_api_v1_is_complete(&api));
  api.reserved[0] = 0U;
  api.reset = 0;
  assert(!aivs_voice_engine_api_v1_is_complete(&api));

  return 0;
}
