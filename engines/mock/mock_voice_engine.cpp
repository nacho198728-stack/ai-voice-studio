#include <voice_engine.h>

#include <atomic>
#include <bit>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <limits>
#include <new>
#include <string_view>

namespace {

constexpr std::uint32_t kMaximumWorkIterations = 1'000'000U;
constexpr std::uint32_t kMinimumSampleRate = 8'000U;
constexpr std::uint32_t kMaximumSampleRate = 192'000U;
constexpr std::uint32_t kMaximumChannels = 2U;
constexpr std::uint64_t kMaximumFrames = 4'096U;
constexpr std::string_view kEngineName = "AIVS Mock VoiceEngine";
constexpr std::string_view kEngineVersion = "1.0.0";
constexpr std::string_view kModelIdentifier = "mock-v1";
constexpr std::string_view kConfigurationPrefix = R"({"work_iterations":)";
#if defined(AIVS_MOCK_ENABLE_INITIAL_GENERATION_TEST_SEAM)
constexpr std::string_view kInitialGenerationField = R"(,"initial_generation":)";
#endif

struct MockConfiguration {
  std::uint32_t work_iterations;
#if defined(AIVS_MOCK_ENABLE_INITIAL_GENERATION_TEST_SEAM)
  std::uint64_t initial_generation;
#endif
};

enum class EngineState : std::uint8_t {
  Initialized,
  ModelLoaded,
  Prepared,
};

bool reserved_are_zero(const std::uint64_t (&reserved)[2]) noexcept {
  return reserved[0] == 0U && reserved[1] == 0U;
}

bool bytes_are_addressable(const std::uint8_t* data, std::uint64_t size) noexcept {
  if ((data == nullptr) != (size == 0U) || size > UINTPTR_MAX) {
    return false;
  }
  if (size == 0U) {
    return true;
  }
  const auto start = reinterpret_cast<std::uintptr_t>(data);
  return start <= UINTPTR_MAX - static_cast<std::uintptr_t>(size);
}

bool valid_bytes_view(const aivs_bytes_view_t& view) noexcept {
  return view.struct_size >= sizeof(view) &&
         view.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION &&
         reserved_are_zero(view.reserved) && bytes_are_addressable(view.data, view.size_bytes);
}

bool valid_mutable_bytes_buffer(const aivs_mutable_bytes_buffer_t& buffer) noexcept {
  return buffer.struct_size >= sizeof(buffer) &&
         buffer.abi_version == AIVS_VOICE_ENGINE_ABI_V1_VERSION &&
         reserved_are_zero(buffer.reserved) &&
         bytes_are_addressable(buffer.data, buffer.capacity_bytes);
}

bool parse_canonical_decimal(
    std::string_view digits, std::uint64_t maximum, std::uint64_t& value) noexcept {
  value = 0U;
  if (digits.empty() || (digits.size() > 1U && digits.front() == '0')) {
    return false;
  }
  for (const char digit : digits) {
    if (digit < '0' || digit > '9') {
      return false;
    }
    const auto number = static_cast<std::uint64_t>(digit - '0');
    if (value > (maximum - number) / 10U) {
      return false;
    }
    value = value * 10U + number;
  }
  return true;
}

bool parse_configuration(
    const aivs_bytes_view_t& configuration, MockConfiguration& parsed) noexcept {
  parsed.work_iterations = 0U;
#if defined(AIVS_MOCK_ENABLE_INITIAL_GENERATION_TEST_SEAM)
  parsed.initial_generation = 0U;
#endif
  if (!valid_bytes_view(configuration) || configuration.size_bytes > 64U) {
    return false;
  }
  const auto text = std::string_view(
      reinterpret_cast<const char*>(configuration.data),
      static_cast<std::size_t>(configuration.size_bytes));
  if (!text.starts_with(kConfigurationPrefix) || text.back() != '}') {
    return false;
  }
  const auto body = text.substr(kConfigurationPrefix.size(), text.size() - kConfigurationPrefix.size() - 1U);
#if defined(AIVS_MOCK_ENABLE_INITIAL_GENERATION_TEST_SEAM)
  const auto generation_position = body.find(kInitialGenerationField);
  const auto work_digits = body.substr(0U, generation_position);
#else
  const auto work_digits = body;
#endif
  std::uint64_t work = 0U;
  if (!parse_canonical_decimal(work_digits, kMaximumWorkIterations, work)) {
    return false;
  }
  parsed.work_iterations = static_cast<std::uint32_t>(work);
#if defined(AIVS_MOCK_ENABLE_INITIAL_GENERATION_TEST_SEAM)
  if (generation_position == std::string_view::npos) {
    return true;
  }
  const auto generation_digits = body.substr(generation_position + kInitialGenerationField.size());
  return parse_canonical_decimal(generation_digits, UINT64_MAX, parsed.initial_generation);
#else
  return true;
#endif
}

bool view_equals(const aivs_bytes_view_t& view, std::string_view expected) noexcept {
  return view.size_bytes == expected.size() &&
         (expected.empty() || std::memcmp(view.data, expected.data(), expected.size()) == 0);
}

}  // namespace

struct aivs_voice_engine_handle {
  explicit aivs_voice_engine_handle(MockConfiguration configuration) noexcept
#if defined(AIVS_MOCK_ENABLE_INITIAL_GENERATION_TEST_SEAM)
      : work_iterations(configuration.work_iterations),
        stream_generation(configuration.initial_generation) {}
#else
      : work_iterations(configuration.work_iterations) {}
#endif

  EngineState state{EngineState::Initialized};
  std::uint32_t work_iterations{0U};
  std::uint32_t sample_rate_hz{0U};
  std::uint32_t channel_count{0U};
  std::uint64_t maximum_frame_count{0U};
  std::uint64_t stream_generation{0U};
  std::atomic<std::uint64_t> process_call_count{0U};
  std::atomic<std::uint64_t> input_frame_count{0U};
  std::atomic<std::uint64_t> output_frame_count{0U};
  std::atomic<std::uint64_t> process_error_count{0U};
  std::atomic<std::uint64_t> work_sink{0U};
};

static_assert(std::atomic<std::uint64_t>::is_always_lock_free);

namespace {

void initialize_result_failure(aivs_initialize_result_t* result) noexcept {
  if (result != nullptr && result->struct_size >=
                               offsetof(aivs_initialize_result_t, engine) + sizeof(result->engine)) {
    result->engine = nullptr;
  }
}

void prepare_result_failure(aivs_prepare_stream_result_t* result) noexcept {
  if (result == nullptr) {
    return;
  }
  if (result->struct_size >= offsetof(aivs_prepare_stream_result_t, algorithmic_latency_frames) +
                                 sizeof(result->algorithmic_latency_frames)) {
    result->algorithmic_latency_frames = 0U;
  }
  if (result->struct_size >= offsetof(aivs_prepare_stream_result_t, stream_generation) +
                                 sizeof(result->stream_generation)) {
    result->stream_generation = 0U;
  }
}

void reset_result_failure(aivs_reset_result_t* result) noexcept {
  if (result != nullptr &&
      result->struct_size >= offsetof(aivs_reset_result_t, stream_generation) +
                                 sizeof(result->stream_generation)) {
    result->stream_generation = 0U;
  }
}

void metrics_failure(aivs_engine_metrics_t* metrics) noexcept {
  if (metrics == nullptr) {
    return;
  }
  if (metrics->struct_size >= offsetof(aivs_engine_metrics_t, process_call_count) +
                                  sizeof(metrics->process_call_count)) {
    metrics->process_call_count = 0U;
  }
  if (metrics->struct_size >= offsetof(aivs_engine_metrics_t, input_frame_count) +
                                  sizeof(metrics->input_frame_count)) {
    metrics->input_frame_count = 0U;
  }
  if (metrics->struct_size >= offsetof(aivs_engine_metrics_t, output_frame_count) +
                                  sizeof(metrics->output_frame_count)) {
    metrics->output_frame_count = 0U;
  }
  if (metrics->struct_size >= offsetof(aivs_engine_metrics_t, process_error_count) +
                                  sizeof(metrics->process_error_count)) {
    metrics->process_error_count = 0U;
  }
}

void info_failure(aivs_engine_info_t* info, bool shortage = false) noexcept {
  if (info == nullptr) {
    return;
  }
  constexpr auto nested_count_end =
      offsetof(aivs_mutable_bytes_buffer_t, written_or_required_bytes) + sizeof(std::uint64_t);
  constexpr auto name_count_end =
      offsetof(aivs_engine_info_t, engine_name_utf8) + nested_count_end;
  constexpr auto version_count_end =
      offsetof(aivs_engine_info_t, engine_version_utf8) + nested_count_end;
  if (info->struct_size >= name_count_end &&
      info->engine_name_utf8.struct_size >= nested_count_end) {
    info->engine_name_utf8.written_or_required_bytes = shortage ? kEngineName.size() : 0U;
  }
  if (info->struct_size >= version_count_end &&
      info->engine_version_utf8.struct_size >= nested_count_end) {
    info->engine_version_utf8.written_or_required_bytes = shortage ? kEngineVersion.size() : 0U;
  }
}

bool info_output_ranges_are_disjoint(const aivs_engine_info_t& info) noexcept {
  const auto name_start = reinterpret_cast<std::uintptr_t>(info.engine_name_utf8.data);
  const auto version_start = reinterpret_cast<std::uintptr_t>(info.engine_version_utf8.data);
  if ((info.engine_name_utf8.data != nullptr &&
       name_start > UINTPTR_MAX - kEngineName.size()) ||
      (info.engine_version_utf8.data != nullptr &&
       version_start > UINTPTR_MAX - kEngineVersion.size())) {
    return false;
  }
  if (info.engine_name_utf8.data == nullptr || info.engine_version_utf8.data == nullptr) {
    return true;
  }
  const auto name_end = name_start + kEngineName.size();
  const auto version_end = version_start + kEngineVersion.size();
  return name_end <= version_start || version_end <= name_start;
}

std::uint32_t nested_process_capacity(
    const aivs_process_audio_result_t* result, std::uint32_t outer_capacity) noexcept {
  constexpr auto required =
      offsetof(aivs_process_audio_result_t, output) + sizeof(std::uint32_t);
  return result != nullptr && outer_capacity >= required ? result->output.struct_size : 0U;
}

aivs_error_code_t process_failure(
    aivs_voice_engine_handle_t* engine,
    aivs_process_audio_result_t* result,
    std::uint32_t outer_capacity,
    std::uint32_t nested_capacity,
    aivs_error_code_t error,
    std::uint64_t required_frames = 0U) noexcept {
  aivs_voice_engine_process_result_set_failure(
      result, outer_capacity, nested_capacity, error, required_frames);
  if (engine != nullptr) {
    engine->process_error_count.fetch_add(1U, std::memory_order_relaxed);
  }
  return error;
}

aivs_error_code_t AIVS_VOICE_ENGINE_CALL mock_initialize(
    const aivs_initialize_request_t* request, aivs_initialize_result_t* result) noexcept {
  initialize_result_failure(result);
  if (request == nullptr || result == nullptr || request->struct_size < sizeof(*request) ||
      result->struct_size < sizeof(*result) ||
      request->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      result->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      !reserved_are_zero(request->reserved) || !reserved_are_zero(result->reserved)) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  MockConfiguration configuration{};
  if (!parse_configuration(request->configuration_utf8, configuration)) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  try {
    result->engine = new (std::nothrow) aivs_voice_engine_handle(configuration);
  } catch (...) {
    result->engine = nullptr;
  }
  return result->engine == nullptr ? AIVS_ERROR_INTERNAL_ERROR : AIVS_ERROR_SUCCESS;
}

aivs_error_code_t AIVS_VOICE_ENGINE_CALL mock_shutdown(
    aivs_voice_engine_handle_t* engine, const aivs_shutdown_request_t* request) noexcept {
  if (engine == nullptr || request == nullptr || request->struct_size < sizeof(*request) ||
      request->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      !reserved_are_zero(request->reserved)) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  delete engine;
  return AIVS_ERROR_SUCCESS;
}

aivs_error_code_t AIVS_VOICE_ENGINE_CALL mock_get_engine_info(
    aivs_voice_engine_handle_t* engine, aivs_engine_info_t* info) noexcept {
  info_failure(info);
  if (engine == nullptr || info == nullptr || info->struct_size < sizeof(*info) ||
      info->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      !reserved_are_zero(info->reserved) || !valid_mutable_bytes_buffer(info->engine_name_utf8) ||
      !valid_mutable_bytes_buffer(info->engine_version_utf8)) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  if (!info_output_ranges_are_disjoint(*info)) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  if (info->engine_name_utf8.capacity_bytes < kEngineName.size() ||
      info->engine_version_utf8.capacity_bytes < kEngineVersion.size()) {
    info_failure(info, true);
    return AIVS_ERROR_BUFFER_TOO_SMALL;
  }
  std::memcpy(info->engine_name_utf8.data, kEngineName.data(), kEngineName.size());
  std::memcpy(info->engine_version_utf8.data, kEngineVersion.data(), kEngineVersion.size());
  info->engine_name_utf8.written_or_required_bytes = kEngineName.size();
  info->engine_version_utf8.written_or_required_bytes = kEngineVersion.size();
  return AIVS_ERROR_SUCCESS;
}

aivs_error_code_t AIVS_VOICE_ENGINE_CALL mock_load_model(
    aivs_voice_engine_handle_t* engine, const aivs_load_model_request_t* request) noexcept {
  if (engine == nullptr || request == nullptr || request->struct_size < sizeof(*request) ||
      request->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      !reserved_are_zero(request->reserved) || !valid_bytes_view(request->model_id_utf8) ||
      !valid_bytes_view(request->model_data)) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  if (!view_equals(request->model_id_utf8, kModelIdentifier) || request->model_data.size_bytes != 0U) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  if (engine->state != EngineState::Initialized) {
    return AIVS_ERROR_INVALID_STATE;
  }
  engine->state = EngineState::ModelLoaded;
  return AIVS_ERROR_SUCCESS;
}

aivs_error_code_t AIVS_VOICE_ENGINE_CALL mock_prepare_stream(
    aivs_voice_engine_handle_t* engine,
    const aivs_prepare_stream_request_t* request,
    aivs_prepare_stream_result_t* result) noexcept {
  prepare_result_failure(result);
  if (engine == nullptr || request == nullptr || result == nullptr ||
      request->struct_size < sizeof(*request) || result->struct_size < sizeof(*result) ||
      request->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      result->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      !reserved_are_zero(request->reserved) || !reserved_are_zero(result->reserved)) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  if (request->format != AIVS_PCM_FORMAT_FLOAT32 ||
      request->layout != AIVS_PCM_LAYOUT_INTERLEAVED || request->sample_rate_hz < kMinimumSampleRate ||
      request->sample_rate_hz > kMaximumSampleRate || request->channel_count == 0U ||
      request->channel_count > kMaximumChannels || request->maximum_frame_count == 0U ||
      request->maximum_frame_count > kMaximumFrames || request->stream_id == 0U) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  if (engine->state != EngineState::ModelLoaded) {
    return AIVS_ERROR_INVALID_STATE;
  }
  std::uint64_t next_generation = 0U;
  const auto advanced =
      aivs_voice_engine_generation_advance(engine->stream_generation, &next_generation);
  if (advanced != AIVS_ERROR_SUCCESS) {
    return advanced;
  }
  engine->sample_rate_hz = request->sample_rate_hz;
  engine->channel_count = request->channel_count;
  engine->maximum_frame_count = request->maximum_frame_count;
  engine->stream_generation = next_generation;
  engine->state = EngineState::Prepared;
  result->algorithmic_latency_frames = 0U;
  result->stream_generation = next_generation;
  return AIVS_ERROR_SUCCESS;
}

aivs_error_code_t AIVS_VOICE_ENGINE_CALL mock_process_audio(
    aivs_voice_engine_handle_t* engine,
    const aivs_process_audio_request_t* request,
    aivs_process_audio_result_t* result) noexcept {
  const std::uint32_t outer_capacity = result == nullptr ? 0U : result->struct_size;
  const std::uint32_t nested_capacity = nested_process_capacity(result, outer_capacity);
  if (engine == nullptr || request == nullptr || result == nullptr ||
      request->struct_size < sizeof(*request) || result->struct_size < sizeof(*result) ||
      request->input.struct_size < sizeof(request->input) ||
      result->output.struct_size < sizeof(result->output) ||
      request->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      request->input.abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      result->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      result->output.abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      !reserved_are_zero(request->reserved) || !reserved_are_zero(request->input.reserved) ||
      !reserved_are_zero(result->reserved) || !reserved_are_zero(result->output.reserved)) {
    return process_failure(
        engine, result, outer_capacity, nested_capacity, AIVS_ERROR_INVALID_ARGUMENT);
  }
  if (request->input.format != AIVS_PCM_FORMAT_FLOAT32 ||
      request->input.layout != AIVS_PCM_LAYOUT_INTERLEAVED ||
      request->input.sample_rate_hz < kMinimumSampleRate ||
      request->input.sample_rate_hz > kMaximumSampleRate || request->input.channel_count == 0U ||
      request->input.channel_count > kMaximumChannels) {
    return process_failure(
        engine, result, outer_capacity, nested_capacity, AIVS_ERROR_INVALID_ARGUMENT);
  }
  std::uint64_t byte_count = 0U;
  if (aivs_voice_engine_pcm_byte_count_is_addressable(
          request->input.frame_count, request->input.channel_count, &byte_count) != AIVS_TRUE ||
      aivs_voice_engine_pcm_pointers_match_counts(
          request->input.samples,
          request->input.frame_count,
          result->output.samples,
          result->output.frame_capacity) != AIVS_TRUE ||
      aivs_voice_engine_pcm_ranges_are_compatible(
          request->input.samples, result->output.samples, byte_count) != AIVS_TRUE) {
    return process_failure(
        engine, result, outer_capacity, nested_capacity, AIVS_ERROR_INVALID_ARGUMENT);
  }
  if (engine->state != EngineState::Prepared ||
      request->stream_generation != engine->stream_generation ||
      request->input.sample_rate_hz != engine->sample_rate_hz ||
      request->input.channel_count != engine->channel_count ||
      request->input.frame_count > engine->maximum_frame_count) {
    return process_failure(engine, result, outer_capacity, nested_capacity, AIVS_ERROR_INVALID_STATE);
  }
  if (aivs_voice_engine_frame_capacity_is_sufficient(
          request->input.frame_count, result->output.frame_capacity) != AIVS_TRUE) {
    return process_failure(
        engine,
        result,
        outer_capacity,
        nested_capacity,
        AIVS_ERROR_BUFFER_TOO_SMALL,
        request->input.frame_count);
  }

  const auto sample_count = request->input.frame_count * request->input.channel_count;
  std::uint64_t work = UINT64_C(0xCBF29CE484222325) ^ sample_count;
  for (std::uint32_t iteration = 0U; iteration < engine->work_iterations; ++iteration) {
    work ^= static_cast<std::uint64_t>(iteration) + UINT64_C(0x9E3779B97F4A7C15);
    work = std::rotl(work, 13);
    work *= UINT64_C(0x100000001B3);
  }
  engine->work_sink.fetch_xor(work, std::memory_order_relaxed);

  for (std::uint64_t index = 0U; index < sample_count; ++index) {
    const auto bits = std::bit_cast<std::uint32_t>(request->input.samples[index]);
    result->output.samples[index] = std::bit_cast<float>(bits ^ UINT32_C(0x80000000));
  }
  result->output.sample_rate_hz = request->input.sample_rate_hz;
  result->output.channel_count = request->input.channel_count;
  result->output.format = request->input.format;
  result->output.layout = request->input.layout;
  result->output.frames_written_or_required = request->input.frame_count;
  result->output.sequence = request->input.sequence;
  result->output.sample_time = request->input.sample_time;
  result->processed_frame_count = request->input.frame_count;
  result->stream_generation = engine->stream_generation;
  engine->process_call_count.fetch_add(1U, std::memory_order_relaxed);
  engine->input_frame_count.fetch_add(request->input.frame_count, std::memory_order_relaxed);
  engine->output_frame_count.fetch_add(request->input.frame_count, std::memory_order_relaxed);
  return AIVS_ERROR_SUCCESS;
}

aivs_error_code_t AIVS_VOICE_ENGINE_CALL mock_reset(
    aivs_voice_engine_handle_t* engine,
    const aivs_reset_request_t* request,
    aivs_reset_result_t* result) noexcept {
  reset_result_failure(result);
  if (engine == nullptr || request == nullptr || result == nullptr ||
      request->struct_size < sizeof(*request) || result->struct_size < sizeof(*result) ||
      request->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      result->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      !reserved_are_zero(request->reserved) || request->reserved_u32 != 0U ||
      !reserved_are_zero(result->reserved) ||
      (request->reason != AIVS_RESET_REASON_CALLER_REQUEST &&
       request->reason != AIVS_RESET_REASON_DISCONTINUITY &&
       request->reason != AIVS_RESET_REASON_RECOVERY)) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  if (engine->state != EngineState::Prepared) {
    return AIVS_ERROR_INVALID_STATE;
  }
  std::uint64_t next_generation = 0U;
  const auto advanced =
      aivs_voice_engine_generation_advance(engine->stream_generation, &next_generation);
  if (advanced != AIVS_ERROR_SUCCESS) {
    return advanced;
  }
  engine->stream_generation = next_generation;
  engine->state = EngineState::ModelLoaded;
  result->stream_generation = next_generation;
  return AIVS_ERROR_SUCCESS;
}

aivs_error_code_t AIVS_VOICE_ENGINE_CALL mock_get_metrics(
    aivs_voice_engine_handle_t* engine,
    const aivs_get_metrics_request_t* request,
    aivs_engine_metrics_t* metrics) noexcept {
  metrics_failure(metrics);
  if (engine == nullptr || request == nullptr || metrics == nullptr ||
      request->struct_size < sizeof(*request) || metrics->struct_size < sizeof(*metrics) ||
      request->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      metrics->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      !reserved_are_zero(request->reserved) || !reserved_are_zero(metrics->reserved)) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  metrics->process_call_count = engine->process_call_count.load(std::memory_order_relaxed);
  metrics->input_frame_count = engine->input_frame_count.load(std::memory_order_relaxed);
  metrics->output_frame_count = engine->output_frame_count.load(std::memory_order_relaxed);
  metrics->process_error_count = engine->process_error_count.load(std::memory_order_relaxed);
  return AIVS_ERROR_SUCCESS;
}

}  // namespace

extern "C" AIVS_VOICE_ENGINE_API aivs_error_code_t AIVS_VOICE_ENGINE_CALL
aivs_voice_engine_get_api(
    const aivs_voice_engine_factory_request_t* request,
    aivs_voice_engine_api_t* out_api,
    std::uint32_t out_api_capacity_bytes) {
  aivs_voice_engine_clear_factory_output(out_api, out_api_capacity_bytes);
  if (request == nullptr || out_api == nullptr || request->struct_size < sizeof(*request) ||
      request->abi_version != AIVS_VOICE_ENGINE_ABI_V1_VERSION ||
      !reserved_are_zero(request->reserved)) {
    return AIVS_ERROR_INVALID_ARGUMENT;
  }
  std::uint32_t selected_version = 0U;
  const auto selection = aivs_voice_engine_select_abi_version(
      request->minimum_abi_version,
      request->maximum_abi_version,
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      &selected_version);
  if (selection != AIVS_ERROR_SUCCESS) {
    return selection;
  }
  if (aivs_voice_engine_api_v1_capacity_is_sufficient(out_api_capacity_bytes) != AIVS_TRUE) {
    return AIVS_ERROR_BUFFER_TOO_SMALL;
  }
  out_api->struct_size = AIVS_VOICE_ENGINE_API_V1_SIZE;
  out_api->abi_version = selected_version;
  out_api->initialize = &mock_initialize;
  out_api->shutdown = &mock_shutdown;
  out_api->get_engine_info = &mock_get_engine_info;
  out_api->load_model = &mock_load_model;
  out_api->prepare_stream = &mock_prepare_stream;
  out_api->process_audio = &mock_process_audio;
  out_api->reset = &mock_reset;
  out_api->get_metrics = &mock_get_metrics;
  return AIVS_ERROR_SUCCESS;
}
