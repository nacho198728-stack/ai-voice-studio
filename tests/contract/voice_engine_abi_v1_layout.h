#ifndef AIVS_TESTS_CONTRACT_VOICE_ENGINE_ABI_V1_LAYOUT_H
#define AIVS_TESTS_CONTRACT_VOICE_ENGINE_ABI_V1_LAYOUT_H

#include <stddef.h>
#include <stdint.h>

#include "voice_engine.h"

#if defined(__cplusplus)
#define AIVS_LAYOUT_ASSERT(condition, message) static_assert((condition), message)
#define AIVS_LAYOUT_ALIGNOF(type) alignof(type)
#else
#define AIVS_LAYOUT_ASSERT(condition, message) _Static_assert((condition), message)
#define AIVS_LAYOUT_ALIGNOF(type) _Alignof(type)
#endif

#define AIVS_LAYOUT_FIELD(type, field, expected) \
  AIVS_LAYOUT_ASSERT(offsetof(type, field) == (expected), #type "." #field " offset drifted")
#define AIVS_LAYOUT_TYPE(type, expected_size) \
  AIVS_LAYOUT_ASSERT(sizeof(type) == (expected_size), #type " size drifted"); \
  AIVS_LAYOUT_ASSERT(AIVS_LAYOUT_ALIGNOF(type) == 8U, #type " alignment drifted")

AIVS_LAYOUT_ASSERT(UINTPTR_MAX == UINT64_MAX, "v1 layout manifest requires a 64-bit pointer ABI");

AIVS_LAYOUT_TYPE(aivs_bytes_view_t, 40U);
AIVS_LAYOUT_FIELD(aivs_bytes_view_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_bytes_view_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_bytes_view_t, data, 8U);
AIVS_LAYOUT_FIELD(aivs_bytes_view_t, size_bytes, 16U);
AIVS_LAYOUT_FIELD(aivs_bytes_view_t, reserved, 24U);

AIVS_LAYOUT_TYPE(aivs_mutable_bytes_buffer_t, 48U);
AIVS_LAYOUT_FIELD(aivs_mutable_bytes_buffer_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_mutable_bytes_buffer_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_mutable_bytes_buffer_t, data, 8U);
AIVS_LAYOUT_FIELD(aivs_mutable_bytes_buffer_t, capacity_bytes, 16U);
AIVS_LAYOUT_FIELD(aivs_mutable_bytes_buffer_t, written_or_required_bytes, 24U);
AIVS_LAYOUT_FIELD(aivs_mutable_bytes_buffer_t, reserved, 32U);

AIVS_LAYOUT_TYPE(aivs_pcm_buffer_t, 72U);
AIVS_LAYOUT_FIELD(aivs_pcm_buffer_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_pcm_buffer_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_pcm_buffer_t, samples, 8U);
AIVS_LAYOUT_FIELD(aivs_pcm_buffer_t, sample_rate_hz, 16U);
AIVS_LAYOUT_FIELD(aivs_pcm_buffer_t, channel_count, 20U);
AIVS_LAYOUT_FIELD(aivs_pcm_buffer_t, format, 24U);
AIVS_LAYOUT_FIELD(aivs_pcm_buffer_t, layout, 28U);
AIVS_LAYOUT_FIELD(aivs_pcm_buffer_t, frame_count, 32U);
AIVS_LAYOUT_FIELD(aivs_pcm_buffer_t, sequence, 40U);
AIVS_LAYOUT_FIELD(aivs_pcm_buffer_t, sample_time, 48U);
AIVS_LAYOUT_FIELD(aivs_pcm_buffer_t, reserved, 56U);

AIVS_LAYOUT_TYPE(aivs_pcm_mutable_buffer_t, 80U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, samples, 8U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, sample_rate_hz, 16U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, channel_count, 20U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, format, 24U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, layout, 28U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, frame_capacity, 32U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, frames_written_or_required, 40U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, sequence, 48U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, sample_time, 56U);
AIVS_LAYOUT_FIELD(aivs_pcm_mutable_buffer_t, reserved, 64U);

AIVS_LAYOUT_TYPE(aivs_voice_engine_factory_request_t, 32U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_factory_request_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_factory_request_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_factory_request_t, minimum_abi_version, 8U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_factory_request_t, maximum_abi_version, 12U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_factory_request_t, reserved, 16U);

AIVS_LAYOUT_TYPE(aivs_initialize_request_t, 64U);
AIVS_LAYOUT_FIELD(aivs_initialize_request_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_initialize_request_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_initialize_request_t, configuration_utf8, 8U);
AIVS_LAYOUT_FIELD(aivs_initialize_request_t, reserved, 48U);

AIVS_LAYOUT_TYPE(aivs_initialize_result_t, 32U);
AIVS_LAYOUT_FIELD(aivs_initialize_result_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_initialize_result_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_initialize_result_t, engine, 8U);
AIVS_LAYOUT_FIELD(aivs_initialize_result_t, reserved, 16U);

AIVS_LAYOUT_TYPE(aivs_engine_info_t, 120U);
AIVS_LAYOUT_FIELD(aivs_engine_info_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_engine_info_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_engine_info_t, engine_name_utf8, 8U);
AIVS_LAYOUT_FIELD(aivs_engine_info_t, engine_version_utf8, 56U);
AIVS_LAYOUT_FIELD(aivs_engine_info_t, reserved, 104U);

AIVS_LAYOUT_TYPE(aivs_load_model_request_t, 104U);
AIVS_LAYOUT_FIELD(aivs_load_model_request_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_load_model_request_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_load_model_request_t, model_id_utf8, 8U);
AIVS_LAYOUT_FIELD(aivs_load_model_request_t, model_data, 48U);
AIVS_LAYOUT_FIELD(aivs_load_model_request_t, reserved, 88U);

AIVS_LAYOUT_TYPE(aivs_prepare_stream_request_t, 56U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_request_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_request_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_request_t, sample_rate_hz, 8U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_request_t, channel_count, 12U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_request_t, format, 16U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_request_t, layout, 20U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_request_t, maximum_frame_count, 24U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_request_t, stream_id, 32U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_request_t, reserved, 40U);

AIVS_LAYOUT_TYPE(aivs_prepare_stream_result_t, 40U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_result_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_result_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_result_t, algorithmic_latency_frames, 8U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_result_t, stream_generation, 16U);
AIVS_LAYOUT_FIELD(aivs_prepare_stream_result_t, reserved, 24U);

AIVS_LAYOUT_TYPE(aivs_process_audio_request_t, 104U);
AIVS_LAYOUT_FIELD(aivs_process_audio_request_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_process_audio_request_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_process_audio_request_t, input, 8U);
AIVS_LAYOUT_FIELD(aivs_process_audio_request_t, stream_generation, 80U);
AIVS_LAYOUT_FIELD(aivs_process_audio_request_t, reserved, 88U);

AIVS_LAYOUT_TYPE(aivs_process_audio_result_t, 120U);
AIVS_LAYOUT_FIELD(aivs_process_audio_result_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_process_audio_result_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_process_audio_result_t, output, 8U);
AIVS_LAYOUT_FIELD(aivs_process_audio_result_t, processed_frame_count, 88U);
AIVS_LAYOUT_FIELD(aivs_process_audio_result_t, stream_generation, 96U);
AIVS_LAYOUT_FIELD(aivs_process_audio_result_t, reserved, 104U);

AIVS_LAYOUT_TYPE(aivs_reset_request_t, 32U);
AIVS_LAYOUT_FIELD(aivs_reset_request_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_reset_request_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_reset_request_t, reason, 8U);
AIVS_LAYOUT_FIELD(aivs_reset_request_t, reserved_u32, 12U);
AIVS_LAYOUT_FIELD(aivs_reset_request_t, reserved, 16U);

AIVS_LAYOUT_TYPE(aivs_reset_result_t, 32U);
AIVS_LAYOUT_FIELD(aivs_reset_result_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_reset_result_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_reset_result_t, stream_generation, 8U);
AIVS_LAYOUT_FIELD(aivs_reset_result_t, reserved, 16U);

AIVS_LAYOUT_TYPE(aivs_shutdown_request_t, 24U);
AIVS_LAYOUT_FIELD(aivs_shutdown_request_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_shutdown_request_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_shutdown_request_t, reserved, 8U);

AIVS_LAYOUT_TYPE(aivs_get_metrics_request_t, 24U);
AIVS_LAYOUT_FIELD(aivs_get_metrics_request_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_get_metrics_request_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_get_metrics_request_t, reserved, 8U);

AIVS_LAYOUT_TYPE(aivs_engine_metrics_t, 56U);
AIVS_LAYOUT_FIELD(aivs_engine_metrics_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_engine_metrics_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_engine_metrics_t, process_call_count, 8U);
AIVS_LAYOUT_FIELD(aivs_engine_metrics_t, input_frame_count, 16U);
AIVS_LAYOUT_FIELD(aivs_engine_metrics_t, output_frame_count, 24U);
AIVS_LAYOUT_FIELD(aivs_engine_metrics_t, process_error_count, 32U);
AIVS_LAYOUT_FIELD(aivs_engine_metrics_t, reserved, 40U);

AIVS_LAYOUT_TYPE(aivs_voice_engine_api_t, 104U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_api_t, struct_size, 0U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_api_t, abi_version, 4U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_api_t, initialize, 8U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_api_t, shutdown, 16U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_api_t, get_engine_info, 24U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_api_t, load_model, 32U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_api_t, prepare_stream, 40U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_api_t, process_audio, 48U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_api_t, reset, 56U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_api_t, get_metrics, 64U);
AIVS_LAYOUT_FIELD(aivs_voice_engine_api_t, reserved, 72U);

#undef AIVS_LAYOUT_TYPE
#undef AIVS_LAYOUT_FIELD
#undef AIVS_LAYOUT_ALIGNOF
#undef AIVS_LAYOUT_ASSERT

#endif  // AIVS_TESTS_CONTRACT_VOICE_ENGINE_ABI_V1_LAYOUT_H
