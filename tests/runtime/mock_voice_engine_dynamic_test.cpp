#ifdef NDEBUG
#undef NDEBUG
#endif

#include <array>
#include <bit>
#include <cassert>
#include <chrono>
#include <cstdio>
#include <cstdint>
#include <cstring>
#include <string>

#include <voice_engine.h>

#if defined(_WIN32)
#include <io.h>
#include <windows.h>
#else
#include <dlfcn.h>
#include <unistd.h>
#endif

namespace {

class StderrCapture final {
 public:
  StderrCapture() {
    file_ = std::tmpfile();
    assert(file_ != nullptr);
    assert(std::fflush(stderr) == 0);
#if defined(_WIN32)
    saved_ = ::_dup(::_fileno(stderr));
    assert(saved_ >= 0);
    assert(::_dup2(::_fileno(file_), ::_fileno(stderr)) == 0);
#else
    saved_ = ::dup(::fileno(stderr));
    assert(saved_ >= 0);
    assert(::dup2(::fileno(file_), ::fileno(stderr)) >= 0);
#endif
  }

  ~StderrCapture() {
    restore();
  }

  StderrCapture(const StderrCapture&) = delete;
  StderrCapture& operator=(const StderrCapture&) = delete;

  std::uint64_t finish() {
    assert(std::fflush(stderr) == 0);
    assert(std::fseek(file_, 0L, SEEK_END) == 0);
    const auto size = std::ftell(file_);
    assert(size >= 0L);
    restore();
    return static_cast<std::uint64_t>(size);
  }

 private:
  void restore() {
    if (saved_ < 0) {
      return;
    }
    assert(std::fflush(stderr) == 0);
#if defined(_WIN32)
    assert(::_dup2(saved_, ::_fileno(stderr)) == 0);
    assert(::_close(saved_) == 0);
#else
    assert(::dup2(saved_, ::fileno(stderr)) >= 0);
    assert(::close(saved_) == 0);
#endif
    saved_ = -1;
    assert(std::fclose(file_) == 0);
    file_ = nullptr;
  }

  std::FILE* file_{nullptr};
  int saved_{-1};
};

class DynamicLibrary {
 public:
  explicit DynamicLibrary(const char* path) {
#if defined(_WIN32)
    const int required = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, nullptr, 0);
    assert(required > 0);
    std::wstring wide(static_cast<std::size_t>(required), L'\0');
    assert(MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, wide.data(), required) > 0);
    handle_ = LoadLibraryW(wide.c_str());
#else
    handle_ = dlopen(path, RTLD_NOW | RTLD_LOCAL);
#endif
    assert(handle_ != nullptr);
  }

  ~DynamicLibrary() {
#if defined(_WIN32)
    assert(FreeLibrary(handle_) != 0);
#else
    assert(dlclose(handle_) == 0);
#endif
  }

  aivs_voice_engine_get_api_fn factory() const {
#if defined(_WIN32)
    const auto symbol = GetProcAddress(handle_, "aivs_voice_engine_get_api");
#else
    const auto symbol = dlsym(handle_, "aivs_voice_engine_get_api");
#endif
    assert(symbol != nullptr);
    aivs_voice_engine_get_api_fn factory{};
    static_assert(sizeof(factory) == sizeof(symbol));
    std::memcpy(&factory, &symbol, sizeof(factory));
    return factory;
  }

 private:
#if defined(_WIN32)
  HMODULE handle_{nullptr};
#else
  void* handle_{nullptr};
#endif
};

aivs_bytes_view_t bytes_view(const char* data, std::size_t size) {
  auto view = aivs_bytes_view_t AIVS_BYTES_VIEW_INIT;
  view.data = reinterpret_cast<const std::uint8_t*>(data);
  view.size_bytes = size;
  return view;
}

void complete_lifecycle_uses_all_eight_operations_through_the_dynamic_table(const char* path) {
  DynamicLibrary library(path);
  const auto factory = library.factory();

  auto factory_request = aivs_voice_engine_factory_request_t AIVS_VOICE_ENGINE_FACTORY_REQUEST_INIT;
  auto api = aivs_voice_engine_api_t AIVS_VOICE_ENGINE_API_OUTPUT_INIT;
  assert(factory(&factory_request, &api, sizeof(api)) == AIVS_ERROR_SUCCESS);
  assert(aivs_voice_engine_api_v1_is_complete(&api) == AIVS_TRUE);

  constexpr char configuration[] = R"({"work_iterations":0})";
  auto initialize_request = aivs_initialize_request_t AIVS_INITIALIZE_REQUEST_INIT;
  initialize_request.configuration_utf8 = bytes_view(configuration, sizeof(configuration) - 1U);
  auto initialize_result = aivs_initialize_result_t AIVS_INITIALIZE_RESULT_INIT;
  assert(api.initialize(&initialize_request, &initialize_result) == AIVS_ERROR_SUCCESS);
  assert(initialize_result.engine != nullptr);
  auto* engine = initialize_result.engine;

  std::array<std::uint8_t, 64> rejected_name{};
  auto malformed_info = aivs_engine_info_t AIVS_ENGINE_INFO_INIT;
  malformed_info.engine_name_utf8.data = rejected_name.data();
  malformed_info.engine_name_utf8.capacity_bytes = rejected_name.size();
  malformed_info.engine_name_utf8.written_or_required_bytes = 99U;
  malformed_info.engine_version_utf8.struct_size = sizeof(std::uint32_t) * 2U;
  assert(api.get_engine_info(engine, &malformed_info) == AIVS_ERROR_INVALID_ARGUMENT);
  assert(malformed_info.engine_name_utf8.written_or_required_bytes == 0U);

  std::array<std::uint8_t, 64> name{};
  std::array<std::uint8_t, 32> version{};
  auto info = aivs_engine_info_t AIVS_ENGINE_INFO_INIT;
  info.engine_name_utf8.data = name.data();
  info.engine_name_utf8.capacity_bytes = name.size();
  info.engine_version_utf8.data = version.data();
  info.engine_version_utf8.capacity_bytes = version.size();

  std::array<std::uint8_t, 64> short_name{};
  std::array<std::uint8_t, 32> short_version{};
  short_name[0] = 0xA5U;
  short_version[0] = 0x5AU;
  auto short_info = aivs_engine_info_t AIVS_ENGINE_INFO_INIT;
  short_info.engine_name_utf8.data = short_name.data();
  short_info.engine_name_utf8.capacity_bytes = 1U;
  short_info.engine_version_utf8.data = short_version.data();
  short_info.engine_version_utf8.capacity_bytes = 1U;
  assert(api.get_engine_info(engine, &short_info) == AIVS_ERROR_BUFFER_TOO_SMALL);
  assert(short_name[0] == 0xA5U);
  assert(short_version[0] == 0x5AU);
  assert(short_info.engine_name_utf8.written_or_required_bytes == 21U);
  assert(short_info.engine_version_utf8.written_or_required_bytes == 5U);

  assert(api.get_engine_info(engine, &info) == AIVS_ERROR_SUCCESS);
  assert(std::string(name.begin(), name.begin() + static_cast<std::ptrdiff_t>(
                                              info.engine_name_utf8.written_or_required_bytes)) ==
         "AIVS Mock VoiceEngine");
  assert(std::string(version.begin(), version.begin() + static_cast<std::ptrdiff_t>(
                                                    info.engine_version_utf8.written_or_required_bytes)) ==
         "1.0.0");

  constexpr char model_id[] = "mock-v1";
  aivs_load_model_request_t load_request{
      sizeof(aivs_load_model_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      bytes_view(model_id, sizeof(model_id) - 1U),
      AIVS_BYTES_VIEW_INIT,
      {0U, 0U},
  };
  assert(api.load_model(engine, &load_request) == AIVS_ERROR_SUCCESS);

  constexpr char wrong_model_id[] = "wrong-model";
  auto malformed_after_transition = load_request;
  malformed_after_transition.model_id_utf8 =
      bytes_view(wrong_model_id, sizeof(wrong_model_id) - 1U);
  assert(api.load_model(engine, &malformed_after_transition) == AIVS_ERROR_INVALID_ARGUMENT);

  auto prepare_request = aivs_prepare_stream_request_t AIVS_PREPARE_STREAM_REQUEST_INIT;
  prepare_request.sample_rate_hz = 48000U;
  prepare_request.channel_count = 2U;
  prepare_request.maximum_frame_count = 4U;
  prepare_request.stream_id = 9U;
  auto prepare_result = aivs_prepare_stream_result_t AIVS_PREPARE_STREAM_RESULT_INIT;
  prepare_result.algorithmic_latency_frames = 99U;
  prepare_result.stream_generation = 99U;
  const auto valid_sample_rate = prepare_request.sample_rate_hz;
  prepare_request.sample_rate_hz = 0U;
  assert(api.prepare_stream(engine, &prepare_request, &prepare_result) ==
         AIVS_ERROR_INVALID_ARGUMENT);
  assert(prepare_result.algorithmic_latency_frames == 0U);
  assert(prepare_result.stream_generation == 0U);
  prepare_request.sample_rate_hz = valid_sample_rate;
  assert(api.prepare_stream(engine, &prepare_request, &prepare_result) == AIVS_ERROR_SUCCESS);
  assert(prepare_result.algorithmic_latency_frames == 0U);
  assert(prepare_result.stream_generation == 1U);

  const std::array<std::uint32_t, 8> input_bits{
      0x00000000U,
      0x3F000000U,
      0xBF000000U,
      0x3F800000U,
      0xBF800000U,
      0x3E800000U,
      0xBE800000U,
      0x80000000U,
  };
  std::array<float, 8> input{};
  std::array<float, 8> output{};
  for (std::size_t index = 0U; index < input.size(); ++index) {
    input[index] = std::bit_cast<float>(input_bits[index]);
  }
  aivs_process_audio_request_t process_request{
      sizeof(aivs_process_audio_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      AIVS_PCM_BUFFER_INIT,
      prepare_result.stream_generation,
      {0U, 0U},
  };
  process_request.input.samples = input.data();
  process_request.input.sample_rate_hz = 48000U;
  process_request.input.channel_count = 2U;
  process_request.input.frame_count = 4U;
  process_request.input.sequence = 17U;
  process_request.input.sample_time = 23U;
  auto process_result = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
  process_result.output.samples = output.data();
  process_result.output.frame_capacity = 4U;

  std::array<float, 8> short_output{};
  short_output.fill(std::bit_cast<float>(UINT32_C(0x3F400000)));
  auto short_process_result = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
  short_process_result.output.samples = short_output.data();
  short_process_result.output.frame_capacity = 3U;
  short_process_result.output.sample_rate_hz = 123U;
  short_process_result.processed_frame_count = 99U;
  short_process_result.stream_generation = 99U;
  StderrCapture hot_path_logs;
  assert(api.process_audio(engine, &process_request, &short_process_result) ==
         AIVS_ERROR_BUFFER_TOO_SMALL);
  assert(short_process_result.output.frames_written_or_required == 4U);
  assert(short_process_result.processed_frame_count == 0U);
  assert(short_process_result.stream_generation == 0U);
  assert(short_process_result.output.sample_rate_hz == 123U);
  for (const float sample : short_output) {
    assert(std::bit_cast<std::uint32_t>(sample) == UINT32_C(0x3F400000));
  }

  assert(api.process_audio(engine, &process_request, &process_result) == AIVS_ERROR_SUCCESS);
  assert(process_result.processed_frame_count == 4U);
  assert(process_result.output.frames_written_or_required == 4U);
  assert(process_result.stream_generation == 1U);
  assert(process_result.output.sequence == 17U);
  assert(process_result.output.sample_time == 23U);
  for (std::size_t index = 0U; index < output.size(); ++index) {
    assert(std::bit_cast<std::uint32_t>(output[index]) == (input_bits[index] ^ 0x80000000U));
  }

  auto in_place = input;
  process_request.input.samples = in_place.data();
  auto in_place_result = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
  in_place_result.output.samples = in_place.data();
  in_place_result.output.frame_capacity = 4U;
  assert(api.process_audio(engine, &process_request, &in_place_result) == AIVS_ERROR_SUCCESS);
  for (std::size_t index = 0U; index < in_place.size(); ++index) {
    assert(std::bit_cast<std::uint32_t>(in_place[index]) == (input_bits[index] ^ 0x80000000U));
  }

  process_request.input.samples = input.data();
  auto stale_result = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
  stale_result.output.samples = output.data();
  stale_result.output.frame_capacity = 4U;
  process_request.stream_generation = prepare_result.stream_generation + 1U;
  assert(api.process_audio(engine, &process_request, &stale_result) == AIVS_ERROR_INVALID_STATE);
  assert(stale_result.processed_frame_count == 0U);
  assert(stale_result.stream_generation == 0U);
  process_request.stream_generation = prepare_result.stream_generation;

  std::array<float, 9> overlap{};
  process_request.input.samples = overlap.data();
  auto overlap_result = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
  overlap_result.output.samples = overlap.data() + 1;
  overlap_result.output.frame_capacity = 4U;
  assert(api.process_audio(engine, &process_request, &overlap_result) ==
         AIVS_ERROR_INVALID_ARGUMENT);
  process_request.input.samples = input.data();

  auto zero_request = process_request;
  zero_request.input.samples = nullptr;
  zero_request.input.frame_count = 0U;
  auto zero_result = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
  assert(api.process_audio(engine, &zero_request, &zero_result) == AIVS_ERROR_SUCCESS);
  assert(zero_result.processed_frame_count == 0U);
  assert(zero_result.output.frames_written_or_required == 0U);
  assert(hot_path_logs.finish() == 0U);

  aivs_get_metrics_request_t metrics_request{
      sizeof(aivs_get_metrics_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
  aivs_engine_metrics_t metrics{
      sizeof(aivs_engine_metrics_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, 0U, 0U, 0U, 0U, {0U, 0U}};
  assert(api.get_metrics(engine, &metrics_request, &metrics) == AIVS_ERROR_SUCCESS);
  assert(metrics.process_call_count == 3U);
  assert(metrics.input_frame_count == 8U);
  assert(metrics.output_frame_count == 8U);
  assert(metrics.process_error_count == 3U);

  aivs_reset_request_t reset_request{
      sizeof(aivs_reset_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      AIVS_RESET_REASON_CALLER_REQUEST,
      0U,
      {0U, 0U},
  };
  auto reset_result = aivs_reset_result_t AIVS_RESET_RESULT_INIT;
  assert(api.reset(engine, &reset_request, &reset_result) == AIVS_ERROR_SUCCESS);
  assert(reset_result.stream_generation == 2U);

  aivs_shutdown_request_t shutdown_request{
      sizeof(aivs_shutdown_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
  assert(api.shutdown(engine, &shutdown_request) == AIVS_ERROR_SUCCESS);
}

void configured_work_is_bounded_cpu_work_outside_the_runtime_clock(const char* path) {
  DynamicLibrary library(path);
  const auto factory = library.factory();
  auto factory_request = aivs_voice_engine_factory_request_t AIVS_VOICE_ENGINE_FACTORY_REQUEST_INIT;
  auto api = aivs_voice_engine_api_t AIVS_VOICE_ENGINE_API_OUTPUT_INIT;
  assert(factory(&factory_request, &api, sizeof(api)) == AIVS_ERROR_SUCCESS);

  constexpr char configuration[] = R"({"work_iterations":1000000})";
  auto initialize_request = aivs_initialize_request_t AIVS_INITIALIZE_REQUEST_INIT;
  initialize_request.configuration_utf8 = bytes_view(configuration, sizeof(configuration) - 1U);
  auto initialize_result = aivs_initialize_result_t AIVS_INITIALIZE_RESULT_INIT;
  assert(api.initialize(&initialize_request, &initialize_result) == AIVS_ERROR_SUCCESS);
  auto* engine = initialize_result.engine;

  constexpr char model_id[] = "mock-v1";
  aivs_load_model_request_t load_request{
      sizeof(aivs_load_model_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      bytes_view(model_id, sizeof(model_id) - 1U),
      AIVS_BYTES_VIEW_INIT,
      {0U, 0U},
  };
  assert(api.load_model(engine, &load_request) == AIVS_ERROR_SUCCESS);
  auto prepare_request = aivs_prepare_stream_request_t AIVS_PREPARE_STREAM_REQUEST_INIT;
  prepare_request.sample_rate_hz = 48000U;
  prepare_request.channel_count = 1U;
  prepare_request.maximum_frame_count = 1U;
  prepare_request.stream_id = 1U;
  auto prepared = aivs_prepare_stream_result_t AIVS_PREPARE_STREAM_RESULT_INIT;
  assert(api.prepare_stream(engine, &prepare_request, &prepared) == AIVS_ERROR_SUCCESS);

  float input = 0.5F;
  float output = 0.0F;
  aivs_process_audio_request_t process_request{
      sizeof(aivs_process_audio_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      AIVS_PCM_BUFFER_INIT,
      prepared.stream_generation,
      {0U, 0U},
  };
  process_request.input.samples = &input;
  process_request.input.sample_rate_hz = 48000U;
  process_request.input.channel_count = 1U;
  process_request.input.frame_count = 1U;
  auto processed = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
  processed.output.samples = &output;
  processed.output.frame_capacity = 1U;
  const auto started = std::chrono::steady_clock::now();
  assert(api.process_audio(engine, &process_request, &processed) == AIVS_ERROR_SUCCESS);
  const auto elapsed = std::chrono::steady_clock::now() - started;
  assert(elapsed >= std::chrono::microseconds(100));
  assert(std::bit_cast<std::uint32_t>(output) ==
         (std::bit_cast<std::uint32_t>(input) ^ UINT32_C(0x80000000)));

  aivs_shutdown_request_t shutdown_request{
      sizeof(aivs_shutdown_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
  assert(api.shutdown(engine, &shutdown_request) == AIVS_ERROR_SUCCESS);
}

}  // namespace

int main(int argc, char** argv) {
  assert(argc == 2);
  complete_lifecycle_uses_all_eight_operations_through_the_dynamic_table(argv[1]);
  configured_work_is_bounded_cpu_work_outside_the_runtime_clock(argv[1]);
}
