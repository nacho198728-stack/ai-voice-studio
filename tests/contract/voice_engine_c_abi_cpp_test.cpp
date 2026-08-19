#include <voice_engine.h>

#ifdef NDEBUG
#undef NDEBUG
#endif

#include <cassert>
#include <cstddef>
#include <cstdint>
#include <type_traits>

#include "voice_engine_abi_v1_initializers.h"
#include "voice_engine_abi_v1_layout.h"

static_assert(__cplusplus >= 202002L, "VoiceEngine C++ consumer contract requires C++20 or newer");

#if defined(_WIN32)
using aivs_test_expected_factory_fn = int32_t(__cdecl*)(
    const struct aivs_voice_engine_factory_request* request,
    struct aivs_voice_engine_api* out_api,
    uint32_t out_api_capacity_bytes);
#else
using aivs_test_expected_factory_fn = int32_t(*)(
    const struct aivs_voice_engine_factory_request* request,
    struct aivs_voice_engine_api* out_api,
    uint32_t out_api_capacity_bytes);
#endif

static_assert(
    std::is_same_v<aivs_error_code_t, int32_t>,
    "canonical error code type must be exactly int32_t");
static_assert(
    std::is_same_v<decltype(&aivs_voice_engine_get_api), aivs_test_expected_factory_fn>);
static_assert(std::is_same_v<aivs_voice_engine_get_api_fn, aivs_test_expected_factory_fn>);
static_assert(std::is_same_v<decltype(aivs_voice_engine_api_t::initialize), aivs_initialize_fn>);
static_assert(std::is_same_v<decltype(aivs_voice_engine_api_t::shutdown), aivs_shutdown_fn>);
static_assert(std::is_same_v<decltype(aivs_voice_engine_api_t::get_engine_info), aivs_get_engine_info_fn>);
static_assert(std::is_same_v<decltype(aivs_voice_engine_api_t::load_model), aivs_load_model_fn>);
static_assert(std::is_same_v<decltype(aivs_voice_engine_api_t::prepare_stream), aivs_prepare_stream_fn>);
static_assert(std::is_same_v<decltype(aivs_voice_engine_api_t::process_audio), aivs_process_audio_fn>);
static_assert(std::is_same_v<decltype(aivs_voice_engine_api_t::reset), aivs_reset_fn>);
static_assert(std::is_same_v<decltype(aivs_voice_engine_api_t::get_metrics), aivs_get_metrics_fn>);

int main() {
  assert_voice_engine_abi_v1_initializers();

  const aivs_voice_engine_factory_request_t factory = AIVS_VOICE_ENGINE_FACTORY_REQUEST_INIT;
  const aivs_prepare_stream_result_t prepare = AIVS_PREPARE_STREAM_RESULT_INIT;
  const aivs_reset_result_t reset = AIVS_RESET_RESULT_INIT;

  assert(factory.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION);
  assert(factory.minimum_abi_version == AIVS_VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION);
  assert(factory.maximum_abi_version == AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION);
  assert(prepare.algorithmic_latency_frames == 0U);
  assert(prepare.stream_generation == 0U);
  assert(reset.stream_generation == 0U);
  assert(AIVS_PCM_FORMAT_FLOAT32 == 1U);
  assert(AIVS_PCM_LAYOUT_INTERLEAVED == 1U);
  return 0;
}
