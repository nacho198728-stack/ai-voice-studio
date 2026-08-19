// VoiceEngine in-process plugin ABI v1.
//
// This header is the complete binary boundary between the isolated C++ Runtime
// and a VoiceEngine plugin. Rust, Tauri, IPC messages, platform device handles,
// C++ exceptions, and allocator ownership never cross it.

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

/* Phase 0.5 requires IEEE-754 binary32 PCM. */
#if FLT_RADIX != 2 || FLT_MANT_DIG != 24
#error "VoiceEngine ABI requires IEEE-754 binary32 float PCM"
#endif

/*
 * All public extensible structures begin with struct_size and abi_version.
 * A caller initializes the complete known structure to zero, then uses the
 * supplied initializer macro or writes those two prefix fields. Implementers
 * accept a structure only when its prefix describes at least the fields they
 * read; future compatible versions append fields only. Every reserved field
 * must be zero on input and is returned as zero on output.
 */

typedef struct aivs_voice_engine_handle aivs_voice_engine_handle_t;

typedef uint32_t aivs_bool_t;
#define AIVS_FALSE UINT32_C(0)
#define AIVS_TRUE UINT32_C(1)

typedef uint32_t aivs_pcm_format_t;
#define AIVS_PCM_FORMAT_FLOAT32 UINT32_C(1)

typedef uint32_t aivs_pcm_layout_t;
#define AIVS_PCM_LAYOUT_INTERLEAVED UINT32_C(1)

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

typedef struct aivs_initialize_request {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_bytes_view_t configuration_utf8;
  uint32_t flags;
  uint32_t reserved_u32;
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
  uint64_t feature_flags;
  aivs_bool_t is_initialized;
  uint32_t reserved_u32;
  uint64_t reserved[2];
} aivs_engine_info_t;

typedef struct aivs_load_model_request {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_bytes_view_t model_id_utf8;
  aivs_bytes_view_t model_data;
  uint32_t flags;
  uint32_t reserved_u32;
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

typedef struct aivs_process_audio_request {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_pcm_buffer_t input;
  uint32_t flags;
  uint32_t reserved_u32;
  uint64_t reserved[2];
} aivs_process_audio_request_t;

typedef struct aivs_process_audio_result {
  uint32_t struct_size;
  uint32_t abi_version;
  aivs_pcm_mutable_buffer_t output;
  uint64_t processed_frame_count;
  uint64_t reserved[2];
} aivs_process_audio_result_t;

typedef struct aivs_reset_request {
  uint32_t struct_size;
  uint32_t abi_version;
  uint32_t flags;
  uint32_t reserved_u32;
  uint64_t reserved[2];
} aivs_reset_request_t;

typedef struct aivs_shutdown_request {
  uint32_t struct_size;
  uint32_t abi_version;
  uint32_t flags;
  uint32_t reserved_u32;
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
    const aivs_initialize_request_t* request,
    aivs_initialize_result_t* result);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_shutdown_fn)(
    aivs_voice_engine_handle_t* engine,
    const aivs_shutdown_request_t* request);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_get_engine_info_fn)(
    aivs_voice_engine_handle_t* engine,
    aivs_engine_info_t* info);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_load_model_fn)(
    aivs_voice_engine_handle_t* engine,
    const aivs_load_model_request_t* request);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_prepare_stream_fn)(
    aivs_voice_engine_handle_t* engine,
    const aivs_prepare_stream_request_t* request);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_process_audio_fn)(
    aivs_voice_engine_handle_t* engine,
    const aivs_process_audio_request_t* request,
    aivs_process_audio_result_t* result);
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_reset_fn)(
    aivs_voice_engine_handle_t* engine,
    const aivs_reset_request_t* request);
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

#define AIVS_BYTES_VIEW_INIT \
  { sizeof(aivs_bytes_view_t), AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION, NULL, UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_MUTABLE_BYTES_BUFFER_INIT \
  { sizeof(aivs_mutable_bytes_buffer_t), AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION, NULL, UINT64_C(0), UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_PCM_BUFFER_INIT \
  { sizeof(aivs_pcm_buffer_t), AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION, NULL, 0U, 0U, AIVS_PCM_FORMAT_FLOAT32, AIVS_PCM_LAYOUT_INTERLEAVED, UINT64_C(0), UINT64_C(0), UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_PCM_MUTABLE_BUFFER_INIT \
  { sizeof(aivs_pcm_mutable_buffer_t), AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION, NULL, 0U, 0U, AIVS_PCM_FORMAT_FLOAT32, AIVS_PCM_LAYOUT_INTERLEAVED, UINT64_C(0), UINT64_C(0), UINT64_C(0), UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_INITIALIZE_REQUEST_INIT \
  { sizeof(aivs_initialize_request_t), AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION, AIVS_BYTES_VIEW_INIT, 0U, 0U, {UINT64_C(0), UINT64_C(0)} }
#define AIVS_INITIALIZE_RESULT_INIT \
  { sizeof(aivs_initialize_result_t), AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION, NULL, {UINT64_C(0), UINT64_C(0)} }
#define AIVS_ENGINE_INFO_INIT \
  { sizeof(aivs_engine_info_t), AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION, AIVS_MUTABLE_BYTES_BUFFER_INIT, AIVS_MUTABLE_BYTES_BUFFER_INIT, UINT64_C(0), AIVS_FALSE, 0U, {UINT64_C(0), UINT64_C(0)} }
#define AIVS_PREPARE_STREAM_REQUEST_INIT \
  { sizeof(aivs_prepare_stream_request_t), AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION, 0U, 0U, AIVS_PCM_FORMAT_FLOAT32, AIVS_PCM_LAYOUT_INTERLEAVED, UINT64_C(0), UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_PROCESS_AUDIO_RESULT_INIT \
  { sizeof(aivs_process_audio_result_t), AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION, AIVS_PCM_MUTABLE_BUFFER_INIT, UINT64_C(0), {UINT64_C(0), UINT64_C(0)} }
#define AIVS_VOICE_ENGINE_API_INIT \
  { sizeof(aivs_voice_engine_api_t), AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, {UINT64_C(0), UINT64_C(0), UINT64_C(0), UINT64_C(0)} }

/*
 * The only plugin export is aivs_voice_engine_get_api. A runtime calls it with
 * its supported ABI version and an initialized output table. A plugin returns
 * AIVS_ERROR_UNSUPPORTED_VOICE_ENGINE_ABI for an incompatible version and
 * otherwise fills only fields covered by out_api->struct_size.
 */
typedef aivs_error_code_t(AIVS_VOICE_ENGINE_CALL* aivs_voice_engine_get_api_fn)(
    uint32_t requested_abi_version,
    aivs_voice_engine_api_t* out_api);

AIVS_VOICE_ENGINE_API aivs_error_code_t AIVS_VOICE_ENGINE_CALL aivs_voice_engine_get_api(
    uint32_t requested_abi_version,
    aivs_voice_engine_api_t* out_api);

#if defined(__cplusplus)
}  // extern "C"
#endif

#endif  // AIVS_RUNTIME_API_VOICE_ENGINE_H
