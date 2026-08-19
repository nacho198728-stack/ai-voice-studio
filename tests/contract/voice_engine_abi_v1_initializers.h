#ifndef AIVS_TESTS_CONTRACT_VOICE_ENGINE_ABI_V1_INITIALIZERS_H
#define AIVS_TESTS_CONTRACT_VOICE_ENGINE_ABI_V1_INITIALIZERS_H

#include <assert.h>

static void assert_voice_engine_abi_v1_initializers(void) {
  const aivs_bytes_view_t bytes = AIVS_BYTES_VIEW_INIT;
  const aivs_mutable_bytes_buffer_t mutable_bytes = AIVS_MUTABLE_BYTES_BUFFER_INIT;
  const aivs_pcm_buffer_t pcm = AIVS_PCM_BUFFER_INIT;
  const aivs_pcm_mutable_buffer_t mutable_pcm = AIVS_PCM_MUTABLE_BUFFER_INIT;
  const aivs_voice_engine_factory_request_t factory = AIVS_VOICE_ENGINE_FACTORY_REQUEST_INIT;
  const aivs_initialize_request_t initialize_request = AIVS_INITIALIZE_REQUEST_INIT;
  const aivs_initialize_result_t initialize_result = AIVS_INITIALIZE_RESULT_INIT;
  const aivs_engine_info_t info = AIVS_ENGINE_INFO_INIT;
  const aivs_prepare_stream_request_t prepare_request = AIVS_PREPARE_STREAM_REQUEST_INIT;
  const aivs_prepare_stream_result_t prepare_result = AIVS_PREPARE_STREAM_RESULT_INIT;
  const aivs_process_audio_result_t process_result = AIVS_PROCESS_AUDIO_RESULT_INIT;
  const aivs_reset_result_t reset_result = AIVS_RESET_RESULT_INIT;
  const aivs_voice_engine_api_t api = AIVS_VOICE_ENGINE_API_OUTPUT_INIT;

  assert(bytes.struct_size == sizeof(bytes));
  assert(bytes.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(bytes.data == NULL && bytes.size_bytes == 0U);
  assert(bytes.reserved[0] == 0U && bytes.reserved[1] == 0U);

  assert(mutable_bytes.struct_size == sizeof(mutable_bytes));
  assert(mutable_bytes.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(mutable_bytes.data == NULL && mutable_bytes.capacity_bytes == 0U);
  assert(mutable_bytes.written_or_required_bytes == 0U);
  assert(mutable_bytes.reserved[0] == 0U && mutable_bytes.reserved[1] == 0U);

  assert(pcm.struct_size == sizeof(pcm));
  assert(pcm.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(pcm.samples == NULL && pcm.sample_rate_hz == 0U && pcm.channel_count == 0U);
  assert(pcm.format == AIVS_PCM_FORMAT_FLOAT32 && pcm.layout == AIVS_PCM_LAYOUT_INTERLEAVED);
  assert(pcm.frame_count == 0U && pcm.sequence == 0U && pcm.sample_time == 0U);
  assert(pcm.reserved[0] == 0U && pcm.reserved[1] == 0U);

  assert(mutable_pcm.struct_size == sizeof(mutable_pcm));
  assert(mutable_pcm.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(mutable_pcm.samples == NULL && mutable_pcm.sample_rate_hz == 0U);
  assert(mutable_pcm.channel_count == 0U && mutable_pcm.format == 0U && mutable_pcm.layout == 0U);
  assert(mutable_pcm.frame_capacity == 0U && mutable_pcm.frames_written_or_required == 0U);
  assert(mutable_pcm.sequence == 0U && mutable_pcm.sample_time == 0U);
  assert(mutable_pcm.reserved[0] == 0U && mutable_pcm.reserved[1] == 0U);

  assert(factory.struct_size == sizeof(factory));
  assert(factory.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(factory.minimum_abi_version == AIVS_VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION);
  assert(factory.maximum_abi_version == AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION);
  assert(factory.reserved[0] == 0U && factory.reserved[1] == 0U);

  assert(initialize_request.struct_size == sizeof(initialize_request));
  assert(initialize_request.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(initialize_request.configuration_utf8.struct_size == sizeof(aivs_bytes_view_t));
  assert(initialize_request.configuration_utf8.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(initialize_request.reserved[0] == 0U && initialize_request.reserved[1] == 0U);

  assert(initialize_result.struct_size == sizeof(initialize_result));
  assert(initialize_result.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(initialize_result.engine == NULL);
  assert(initialize_result.reserved[0] == 0U && initialize_result.reserved[1] == 0U);

  assert(info.struct_size == sizeof(info));
  assert(info.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(info.engine_name_utf8.struct_size == sizeof(aivs_mutable_bytes_buffer_t));
  assert(info.engine_version_utf8.struct_size == sizeof(aivs_mutable_bytes_buffer_t));
  assert(info.reserved[0] == 0U && info.reserved[1] == 0U);

  assert(prepare_request.struct_size == sizeof(prepare_request));
  assert(prepare_request.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(prepare_request.sample_rate_hz == 0U && prepare_request.channel_count == 0U);
  assert(prepare_request.format == AIVS_PCM_FORMAT_FLOAT32);
  assert(prepare_request.layout == AIVS_PCM_LAYOUT_INTERLEAVED);
  assert(prepare_request.maximum_frame_count == 0U && prepare_request.stream_id == 0U);
  assert(prepare_request.reserved[0] == 0U && prepare_request.reserved[1] == 0U);

  assert(prepare_result.struct_size == sizeof(prepare_result));
  assert(prepare_result.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(prepare_result.algorithmic_latency_frames == 0U && prepare_result.stream_generation == 0U);
  assert(prepare_result.reserved[0] == 0U && prepare_result.reserved[1] == 0U);

  assert(process_result.struct_size == sizeof(process_result));
  assert(process_result.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(process_result.output.struct_size == sizeof(aivs_pcm_mutable_buffer_t));
  assert(process_result.output.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(process_result.processed_frame_count == 0U && process_result.stream_generation == 0U);
  assert(process_result.reserved[0] == 0U && process_result.reserved[1] == 0U);

  assert(reset_result.struct_size == sizeof(reset_result));
  assert(reset_result.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(reset_result.stream_generation == 0U);
  assert(reset_result.reserved[0] == 0U && reset_result.reserved[1] == 0U);

  assert(api.struct_size == AIVS_VOICE_ENGINE_API_V1_SIZE && api.abi_version == 0U);
  assert(api.initialize == NULL && api.shutdown == NULL && api.get_engine_info == NULL);
  assert(api.load_model == NULL && api.prepare_stream == NULL && api.process_audio == NULL);
  assert(api.reset == NULL && api.get_metrics == NULL);
  assert(api.reserved[0] == 0U && api.reserved[1] == 0U);
  assert(api.reserved[2] == 0U && api.reserved[3] == 0U);
}

#endif  // AIVS_TESTS_CONTRACT_VOICE_ENGINE_ABI_V1_INITIALIZERS_H
