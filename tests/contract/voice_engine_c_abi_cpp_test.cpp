#include <cassert>
#include <cstddef>
#include <type_traits>

#include "voice_engine.h"

static_assert(std::is_same_v<decltype(&aivs_voice_engine_get_api), aivs_voice_engine_get_api_fn>);
static_assert(offsetof(aivs_process_audio_request_t, struct_size) == 0U);
static_assert(offsetof(aivs_process_audio_result_t, abi_version) == sizeof(std::uint32_t));
static_assert(offsetof(aivs_voice_engine_api_t, initialize) > offsetof(aivs_voice_engine_api_t, abi_version));

int main() {
  const aivs_initialize_request_t initialize = AIVS_INITIALIZE_REQUEST_INIT;
  const aivs_prepare_stream_request_t prepare = AIVS_PREPARE_STREAM_REQUEST_INIT;
  const aivs_process_audio_result_t result = AIVS_PROCESS_AUDIO_RESULT_INIT;

  assert(initialize.struct_size == sizeof(aivs_initialize_request_t));
  assert(prepare.abi_version == AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION);
  assert(result.output.frames_written_or_required == 0U);
  assert(AIVS_PCM_FORMAT_FLOAT32 == 1U);
  assert(AIVS_PCM_LAYOUT_INTERLEAVED == 1U);
  return 0;
}
