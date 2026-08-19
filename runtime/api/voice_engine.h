// VoiceEngine in-process plugin ABI v1.
// This is the complete C17/C++20 boundary between the isolated C++ Runtime and
// a VoiceEngine module. Rust, Tauri, IPC, C++ objects/exceptions, platform
// handles, allocator ownership, and non-fixed-width public scalar types do not
// cross this header.

#ifndef AIVS_RUNTIME_API_VOICE_ENGINE_H
#define AIVS_RUNTIME_API_VOICE_ENGINE_H

#include <float.h>
#include <stddef.h>
#include <stdint.h>

#include "ai_voice_contracts/generated_contracts_c.h"

#if defined(_WIN32) && defined(AIVS_VOICE_ENGINE_BUILD)
#define AIVS_VOICE_ENGINE_API __declspec(dllexport)
#define AIVS_VOICE_ENGINE_CALL __cdecl
#elif defined(_WIN32)
#define AIVS_VOICE_ENGINE_API
#define AIVS_VOICE_ENGINE_CALL __cdecl
#elif defined(__GNUC__) || defined(__clang__)
#define AIVS_VOICE_ENGINE_API __attribute__((visibility("default")))
#define AIVS_VOICE_ENGINE_CALL
#else
#define AIVS_VOICE_ENGINE_API
#define AIVS_VOICE_ENGINE_CALL
#endif

#if defined(__cplusplus)
extern "C" {
#endif

#if FLT_RADIX != 2 || FLT_MANT_DIG != 24 || FLT_MAX_EXP != 128 || FLT_MIN_EXP != -125
#error "VoiceEngine ABI requires IEEE-754 binary32 float PCM"
#endif
#if defined(__cplusplus)
static_assert(sizeof(float) == 4U, "VoiceEngine ABI requires four-byte float PCM");
#else
_Static_assert(sizeof(float) == 4U, "VoiceEngine ABI requires four-byte float PCM");
#endif

/* Every public extensible structure is append-only and starts with this logical
 * prefix. Input reserved fields must be zero. Output reserved fields are zero.
 * Nested structures must have the exact ABI version selected by the API table;
 * their struct_size must cover every v1 field the callee reads. */

typedef struct aivs_voice_engine_handle aivs_voice_engine_handle_t;

typedef uint32_t aivs_bool_t;
#define AIVS_FALSE UINT32_C(0)
#define AIVS_TRUE UINT32_C(1)

typedef uint32_t aivs_pcm_format_t;
#define AIVS_PCM_FORMAT_FLOAT32 UINT32_C(1)
typedef uint32_t aivs_pcm_layout_t;
#define AIVS_PCM_LAYOUT_INTERLEAVED UINT32_C(1)

typedef uint32_t aivs_reset_reason_t;
#define AIVS_RESET_REASON_CALLER_REQUEST UINT32_C(1)
#define AIVS_RESET_REASON_DISCONTINUITY UINT32_C(2)
#define AIVS_RESET_REASON_RECOVERY UINT32_C(3)

typedef struct aivs_bytes_view {
  uint32_t struct_size;
  uint32_t abi_version;
  const uint8_t* data;
  uint64_t size_bytes;
  uint64_t reserved[2];
} aivs_bytes_view_t;

typedef struct aivs_mutable_bytes_buffer {
  uint32_t struct_size;
  uint32_t abi_version;
  uint8_t* data;
  uint64_t capacity_bytes;
  uint64_t written_or_required_bytes;
  uint64_t reserved[2];
} aivs_mutable_bytes_buffer_t;

typedef struct aivs_pcm_buffer {
  uint32_t struct_size;
  uint32_t abi_version;
  const float* samples;
  uint32_t sample_rate_hz;
  uint32_t channel_count;
  aivs_pcm_format_t format;
  aivs_pcm_layout_t layout;
  uint64_t frame_count;
  uint64_t sequence;
  uint64_t sample_time;
  uint64_t reserved[2];
} aivs_pcm_buffer_t;

typedef struct aivs_pcm_mutable_buffer {
  uint32_t struct_size;
  uint32_t abi_version;
  float* samples;
  uint32_t sample_rate_hz;
  uint32_t channel_count;
  aivs_pcm_format_t format;
  aivs_pcm_layout_t layout;
  uint64_t frame_capacity;
  uint64_t frames_written_or_required;
  uint64_t sequence;
  uint64_t sample_time;
  uint64_t reserved[2];
} aivs_pcm_mutable_buffer_t;

typedef struct aivs_voice_engine_factory_request {
  uint32_t struct_size;
  uint32_t abi_version;
  uint32_t minimum_abi_version;
  uint32_t maximum_abi_version;
  uint64_t reserved[2];
} aivs_voice_engine_factory_request_t;

typedef struct aivs_initialize_request {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_bytes_view_t configuration_utf8;
  uint64_t reserved[2];
} aivs_initialize_request_t;

typedef struct aivs_initialize_result {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_voice_engine_handle_t* engine;
  uint64_t reserved[2];
} aivs_initialize_result_t;

typedef struct aivs_engine_info {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_mutable_bytes_buffer_t engine_name_utf8;
  aivs_mutable_bytes_buffer_t engine_version_utf8;
  uint64_t reserved[2];
} aivs_engine_info_t;

typedef struct aivs_load_model_request {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_bytes_view_t model_id_utf8;
  aivs_bytes_view_t model_data;
  uint64_t reserved[2];
} aivs_load_model_request_t;

typedef struct aivs_prepare_stream_request {
  uint32_t struct_size;
  uint32_t abi_version;
  uint32_t sample_rate_hz;
  uint32_t channel_count;
  aivs_pcm_format_t format;
  aivs_pcm_layout_t layout;
  uint64_t maximum_frame_count;
  uint64_t stream_id;
  uint64_t reserved[2];
} aivs_prepare_stream_request_t;

typedef struct aivs_prepare_stream_result {
  uint32_t struct_size;
  uint32_t abi_version;
  uint64_t algorithmic_latency_frames;
  uint64_t stream_generation;
  uint64_t reserved[2];
} aivs_prepare_stream_result_t;

typedef struct aivs_process_audio_request {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_pcm_buffer_t input;
  uint64_t stream_generation;
  uint64_t reserved[2];
} aivs_process_audio_request_t;

typedef struct aivs_process_audio_result {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_pcm_mutable_buffer_t output;
  uint64_t processed_frame_count;
  uint64_t stream_generation;
  uint64_t reserved[2];
} aivs_process_audio_result_t;

typedef struct aivs_reset_request {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_reset_reason_t reason;
  uint32_t reserved_u32;
  uint64_t reserved[2];
} aivs_reset_request_t;

typedef struct aivs_reset_result {
  uint32_t struct_size;
  uint32_t abi_version;
  uint64_t stream_generation;
  uint64_t reserved[2];
} aivs_reset_result_t;

typedef struct aivs_shutdown_request {
  uint32_t struct_size;
  uint32_t abi_version;
  uint64_t reserved[2];
} aivs_shutdown_request_t;

typedef struct aivs_get_metrics_request {
  uint32_t struct_size;
  uint32_t abi_version;
  uint64_t reserved[2];
} aivs_get_metrics_request_t;

typedef struct aivs_engine_metrics {
  uint32_t struct_size;
  uint32_t abi_version;
  uint64_t process_call_count;
  uint64_t input_frame_count;
  uint64_t output_frame_count;
  uint64_t process_error_count;
  uint64_t reserved[2];
} aivs_engine_metrics_t;

typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_initialize_fn)(
    const aivs_initialize_request_t* request, aivs_initialize_result_t* result);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_shutdown_fn)(
    aivs_voice_engine_handle_t* engine, const aivs_shutdown_request_t* request);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_get_engine_info_fn)(
    aivs_voice_engine_handle_t* engine, aivs_engine_info_t* info);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_load_model_fn)(
    aivs_voice_engine_handle_t* engine, const aivs_load_model_request_t* request);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_prepare_stream_fn)(
    aivs_voice_engine_handle_t* engine,
    const aivs_prepare_stream_request_t* request,
    aivs_prepare_stream_result_t* result);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_process_audio_fn)(
    aivs_voice_engine_handle_t* engine,
    const aivs_process_audio_request_t* request,
    aivs_process_audio_result_t* result);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_reset_fn)(
    aivs_voice_engine_handle_t* engine,
    const aivs_reset_request_t* request,
    aivs_reset_result_t* result);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_get_metrics_fn)(
    aivs_voice_engine_handle_t* engine,
    const aivs_get_metrics_request_t* request,
    aivs_engine_metrics_t* metrics);

typedef struct aivs_voice_engine_api {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_initialize_fn initialize;
  aivs_shutdown_fn shutdown;
  aivs_get_engine_info_fn get_engine_info;
  aivs_load_model_fn load_model;
  aivs_prepare_stream_fn prepare_stream;
  aivs_process_audio_fn process_audio;
  aivs_reset_fn reset;
  aivs_get_metrics_fn get_metrics;
  uint64_t reserved[4];
} aivs_voice_engine_api_t;

#define AIVS_VOICE_ENGINE_API_V1_SIZE ((uint32_t)sizeof(aivs_voice_engine_api_t))

/* Header-only contract helpers: engines and runtimes must apply these exact
 * selection/capacity rules; they do not load modules or process audio. */
static inline aivs_error_code_t aivs_voice_engine_select_abi_version(
    uint32_t caller_minimum,
    uint32_t caller_maximum,
    uint32_t plugin_minimum,
    uint32_t plugin_maximum,
    uint32_t* selected_version) {
  uint32_t lower;
  uint32_t upper;
  if (selected_version == NULL) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  *selected_version = 0U;
  if (caller_minimum == 0U || plugin_minimum == 0U || caller_minimum > caller_maximum
      || plugin_minimum > plugin_maximum) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  lower = caller_minimum > plugin_minimum ? caller_minimum : plugin_minimum;
  upper = caller_maximum < plugin_maximum ? caller_maximum : plugin_maximum;
  if (lower > upper) {
    return AIVS_ERROR_UNSUPPORTED_VOICE_ENGINE_ABI;
  }
  *selected_version = upper;
  return AIVS_ERROR_SUCCESS;
}

static inline aivs_bool_t aivs_voice_engine_api_v1_capacity_is_sufficient(uint32_t capacity) {
  return capacity >= AIVS_VOICE_ENGINE_API_V1_SIZE ? AIVS_TRUE : AIVS_FALSE;
}

static inline void aivs_voice_engine_clear_factory_output(
    aivs_voice_engine_api_t* output, uint32_t output_capacity_bytes) {
  uint8_t* bytes = (uint8_t*)output;
  uint32_t clear_size = output_capacity_bytes < AIVS_VOICE_ENGINE_API_V1_SIZE
      ? output_capacity_bytes
      : AIVS_VOICE_ENGINE_API_V1_SIZE;
  uint32_t index;
  if (bytes == NULL) {
    return;
  }
  for (index = 0U; index < clear_size; ++index) {
    bytes[index] = 0U;
  }
}

static inline aivs_bool_t aivs_voice_engine_pcm_byte_count_is_addressable(
    uint64_t frame_count, uint32_t channel_count, uint64_t* byte_count) {
  uint64_t sample_count;
  uint64_t bytes;
  if (byte_count == NULL) {
    return AIVS_FALSE;
  }
  *byte_count = 0U;
  if (channel_count == 0U || frame_count > UINT64_MAX / channel_count) {
    return AIVS_FALSE;
  }
  sample_count = frame_count * channel_count;
  if (sample_count > UINT64_MAX / UINT64_C(4)) {
    return AIVS_FALSE;
  }
  bytes = sample_count * UINT64_C(4);
  if (UINTPTR_MAX < UINT64_MAX && bytes > (uint64_t)UINTPTR_MAX) {
    return AIVS_FALSE;
  }
  *byte_count = bytes;
  return AIVS_TRUE;
}

static inline aivs_bool_t aivs_voice_engine_pcm_pointers_match_counts(
    const float* input_samples,
    uint64_t input_frame_count,
    const float* output_samples,
    uint64_t output_frame_capacity) {
  return (input_samples == NULL) == (input_frame_count == 0U)
      && (output_samples == NULL) == (output_frame_capacity == 0U) ? AIVS_TRUE : AIVS_FALSE;
}

static inline aivs_bool_t aivs_voice_engine_pcm_ranges_are_compatible(
    const float* input_samples, const float* output_samples, uint64_t byte_count) {
  uintptr_t input_start;
  uintptr_t output_start;
  uintptr_t byte_count_as_pointer;
  if (byte_count == 0U) {
    return AIVS_TRUE;
  }
  if (input_samples == NULL || output_samples == NULL || byte_count > UINTPTR_MAX) {
    return AIVS_FALSE;
  }
  input_start = (uintptr_t)input_samples;
  output_start = (uintptr_t)output_samples;
  byte_count_as_pointer = (uintptr_t)byte_count;
  if (input_start > UINTPTR_MAX - byte_count_as_pointer
      || output_start > UINTPTR_MAX - byte_count_as_pointer) {
    return AIVS_FALSE;
  }
  if (input_start == output_start) {
    return AIVS_TRUE;
  }
  return input_start + byte_count_as_pointer <= output_start
      || output_start + byte_count_as_pointer <= input_start ? AIVS_TRUE : AIVS_FALSE;
}

static inline aivs_bool_t aivs_voice_engine_frame_capacity_is_sufficient(
    uint64_t required_frame_count, uint64_t output_frame_capacity) {
  return output_frame_capacity >= required_frame_count ? AIVS_TRUE : AIVS_FALSE;
}

static inline aivs_error_code_t aivs_voice_engine_generation_advance(
    uint64_t current_generation, uint64_t* next_generation) {
  if (next_generation == NULL) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  *next_generation = 0U;
  if (current_generation == UINT64_MAX) {
    return AIVS_ERROR_INVALID_STATE;
  }
  *next_generation = current_generation + UINT64_C(1);
  return AIVS_ERROR_SUCCESS;
}

static inline void aivs_voice_engine_process_result_set_failure(
    aivs_process_audio_result_t* result,
    aivs_error_code_t error,
    uint64_t required_frame_count) {
  if (result == NULL) {
    return;
  }
  result->output.frames_written_or_required = error == AIVS_ERROR_BUFFER_TOO_SMALL
      ? required_frame_count
      : UINT64_C(0);
  result->processed_frame_count = UINT64_C(0);
  result->stream_generation = UINT64_C(0);
}

static inline aivs_bool_t aivs_voice_engine_api_is_complete_for_version(
    const aivs_voice_engine_api_t* api, uint32_t selected_abi_version) {
  if (api == NULL || api->struct_size != AIVS_VOICE_ENGINE_API_V1_SIZE
      || selected_abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION
      || api->abi_version != selected_abi_version) {
    return AIVS_FALSE;
  }
  if (api->reserved[0] != 0U || api->reserved[1] != 0U || api->reserved[2] != 0U
      || api->reserved[3] != 0U) {
    return AIVS_FALSE;
  }
  return api->initialize != NULL && api->shutdown != NULL && api->get_engine_info != NULL
      && api->load_model != NULL && api->prepare_stream != NULL && api->process_audio != NULL
      && api->reset != NULL && api->get_metrics != NULL ? AIVS_TRUE : AIVS_FALSE;
}

static inline aivs_bool_t aivs_voice_engine_api_v1_is_complete(
    const aivs_voice_engine_api_t* api) {
  return aivs_voice_engine_api_is_complete_for_version(api, AIVS_VOICE_ENGINE_ABI_V1_VERSION);
}

#define AIVS_BYTES_VIEW_INIT \
  { sizeof(aivs_bytes_view_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, NULL, UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_MUTABLE_BYTES_BUFFER_INIT \
  { sizeof(aivs_mutable_bytes_buffer_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, NULL, UINT64_C(0), UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_PCM_BUFFER_INIT \
  { sizeof(aivs_pcm_buffer_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, NULL, 0U, 0U, AIVS_PCM_FORMAT_FLOAT32, AIVS_PCM_LAYOUT_INTERLEAVED, UINT64_C(0), UINT64_C(0), UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_PCM_MUTABLE_BUFFER_INIT \
  { sizeof(aivs_pcm_mutable_buffer_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, NULL, 0U, 0U, 0U, 0U, UINT64_C(0), UINT64_C(0), UINT64_C(0), UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_VOICE_ENGINE_FACTORY_REQUEST_INIT \
  { sizeof(aivs_voice_engine_factory_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, AIVS_VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION, AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION, {UINT64_C(0), UINT64_C(0)} }
#define AIVS_INITIALIZE_REQUEST_INIT \
  { sizeof(aivs_initialize_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, AIVS_BYTES_VIEW_INIT, {UINT64_C(0), UINT64_C(0)} }
#define AIVS_INITIALIZE_RESULT_INIT \
  { sizeof(aivs_initialize_result_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, NULL, {UINT64_C(0), UINT64_C(0)} }
#define AIVS_ENGINE_INFO_INIT \
  { sizeof(aivs_engine_info_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, AIVS_MUTABLE_BYTES_BUFFER_INIT, AIVS_MUTABLE_BYTES_BUFFER_INIT, {UINT64_C(0), UINT64_C(0)} }
#define AIVS_PREPARE_STREAM_REQUEST_INIT \
  { sizeof(aivs_prepare_stream_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, 0U, 0U, AIVS_PCM_FORMAT_FLOAT32, AIVS_PCM_LAYOUT_INTERLEAVED, UINT64_C(0), UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_PREPARE_STREAM_RESULT_INIT \
  { sizeof(aivs_prepare_stream_result_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, UINT64_C(0), UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_PROCESS_AUDIO_RESULT_INIT \
  { sizeof(aivs_process_audio_result_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, AIVS_PCM_MUTABLE_BUFFER_INIT, UINT64_C(0), UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_RESET_RESULT_INIT \
  { sizeof(aivs_reset_result_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_VOICE_ENGINE_API_OUTPUT_INIT \
  { AIVS_VOICE_ENGINE_API_V1_SIZE, 0U, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, {UINT64_C(0), UINT64_C(0), UINT64_C(0), UINT64_C(0)} }

/* Exactly one plugin export. request gives the caller's inclusive ABI range.
 * On success out_api describes the highest common ABI and a complete table.
 * out_api->struct_size is caller capacity on entry and exact table size on
 * return; out_api->abi_version is zero on entry and selected on success. */
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_voice_engine_get_api_fn)(
    const aivs_voice_engine_factory_request_t* request,
    aivs_voice_engine_api_t* out_api,
    uint32_t out_api_capacity_bytes);

AIVS_VOICE_ENGINE_API aivs_error_code_t AIVS_VOICE_ENGINE_CALL aivs_voice_engine_get_api(
    const aivs_voice_engine_factory_request_t* request,
    aivs_voice_engine_api_t* out_api,
    uint32_t out_api_capacity_bytes);

#if defined(__cplusplus)
}  // extern "C"
#endif

#endif  // AIVS_RUNTIME_API_VOICE_ENGINE_H
