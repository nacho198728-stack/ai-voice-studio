#ifdef NDEBUG
#undef NDEBUG
#endif

#include <array>
#include <bit>
#include <cassert>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <memory>
#include <span>
#include <string_view>
#include <vector>

#include <ai_voice_runtime/mock_pipeline.hpp>
#include <ai_voice_runtime/voice_engine_loader.hpp>

namespace runtime = ai_voice::runtime;
namespace message = ai_voice::contracts::runtime_message;
using ai_voice::contracts::ErrorCode;

namespace {

constexpr std::array<std::uint32_t, 8> kInputBits{
    0x00000000U,
    0x3E800000U,
    0xBE800000U,
    0x3F000000U,
    0xBF000000U,
    0x3F800000U,
    0xBF800000U,
    0x80000000U,
};
constexpr std::array<std::uint32_t, 8> kExpectedOutputBits{
    0x80000000U,
    0xBE800000U,
    0x3E800000U,
    0xBF000000U,
    0x3F000000U,
    0xBF800000U,
    0x3F800000U,
    0x00000000U,
};
constexpr std::array<std::array<std::uint8_t, 4>, 8> kExpectedOutputBytes{{
    {0x00U, 0x00U, 0x00U, 0x80U},
    {0x00U, 0x00U, 0x80U, 0xBEU},
    {0x00U, 0x00U, 0x80U, 0x3EU},
    {0x00U, 0x00U, 0x00U, 0xBFU},
    {0x00U, 0x00U, 0x00U, 0x3FU},
    {0x00U, 0x00U, 0x80U, 0xBFU},
    {0x00U, 0x00U, 0x80U, 0x3FU},
    {0x00U, 0x00U, 0x00U, 0x00U},
}};
constexpr std::uint64_t kExpectedChecksum = UINT64_C(0x3ECD5190F6F4F725);
constexpr std::uint32_t kFrames = 128U;
constexpr std::uint32_t kChannels = 2U;
constexpr std::size_t kSampleCount = kFrames * kChannels;

std::uint32_t read_u32(std::span<const std::uint8_t> bytes, std::size_t offset) {
  std::uint32_t value = 0U;
  for (std::size_t index = 0U; index < 4U; ++index) {
    value |= static_cast<std::uint32_t>(bytes[offset + index]) << (index * 8U);
  }
  return value;
}

std::uint64_t read_u64(std::span<const std::uint8_t> bytes, std::size_t offset) {
  std::uint64_t value = 0U;
  for (std::size_t index = 0U; index < 8U; ++index) {
    value |= static_cast<std::uint64_t>(bytes[offset + index]) << (index * 8U);
  }
  return value;
}

std::uint64_t checksum(std::span<const float> samples) {
  std::uint64_t hash = UINT64_C(0xCBF29CE484222325);
  for (const float sample : samples) {
    const auto bits = std::bit_cast<std::uint32_t>(sample);
    for (std::size_t byte = 0U; byte < 4U; ++byte) {
      hash ^= static_cast<std::uint8_t>(bits >> (byte * 8U));
      hash *= UINT64_C(0x100000001B3);
    }
  }
  return hash;
}

aivs_bytes_view_t bytes_view(std::string_view bytes) {
  auto view = aivs_bytes_view_t AIVS_BYTES_VIEW_INIT;
  view.data = reinterpret_cast<const std::uint8_t*>(bytes.data());
  view.size_bytes = bytes.size();
  return view;
}

struct LoadedEngine {
  std::shared_ptr<runtime::VoiceEngineModule> module;
  std::unique_ptr<runtime::VoiceEngineInstance> instance;
};

LoadedEngine load_engine(
    const std::filesystem::path& plugin_path, std::string_view configuration) {
  auto loaded = runtime::VoiceEngineModule::load(plugin_path);
  assert(loaded.error_code == ErrorCode::Success);
  assert(loaded.module);
  assert(loaded.diagnostic.empty());
  auto initialized = loaded.module->initialize(std::span<const std::uint8_t>(
      reinterpret_cast<const std::uint8_t*>(configuration.data()), configuration.size()));
  assert(initialized.error_code == ErrorCode::Success);
  assert(initialized.instance);

  constexpr std::string_view model_identifier = "mock-v1";
  aivs_load_model_request_t load_request{
      sizeof(aivs_load_model_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      bytes_view(model_identifier),
      AIVS_BYTES_VIEW_INIT,
      {0U, 0U},
  };
  assert(initialized.instance->load_model(&load_request) == ErrorCode::Success);
  return {std::move(loaded.module), std::move(initialized.instance)};
}

aivs_prepare_stream_result_t prepare(runtime::VoiceEngineInstance& engine) {
  auto request = aivs_prepare_stream_request_t AIVS_PREPARE_STREAM_REQUEST_INIT;
  request.sample_rate_hz = 48'000U;
  request.channel_count = kChannels;
  request.maximum_frame_count = kFrames;
  request.stream_id = 17U;
  auto result = aivs_prepare_stream_result_t AIVS_PREPARE_STREAM_RESULT_INIT;
  assert(engine.prepare_stream(&request, &result) == ErrorCode::Success);
  assert(result.algorithmic_latency_frames == 0U);
  return result;
}

std::array<float, kSampleCount> fixed_input() {
  std::array<float, kSampleCount> input{};
  for (std::size_t index = 0U; index < input.size(); ++index) {
    input[index] = std::bit_cast<float>(kInputBits[index % kInputBits.size()]);
  }
  return input;
}

aivs_process_audio_request_t process_request(
    const std::array<float, kSampleCount>& input, std::uint64_t generation) {
  aivs_process_audio_request_t request{
      sizeof(aivs_process_audio_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      AIVS_PCM_BUFFER_INIT,
      generation,
      {0U, 0U},
  };
  request.input.samples = input.data();
  request.input.sample_rate_hz = 48'000U;
  request.input.channel_count = kChannels;
  request.input.frame_count = kFrames;
  request.input.sequence = 29U;
  request.input.sample_time = 31U;
  return request;
}

aivs_process_audio_result_t process_result(std::array<float, kSampleCount>& output) {
  auto result = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
  result.output.samples = output.data();
  result.output.frame_capacity = kFrames;
  return result;
}

aivs_engine_metrics_t metrics(runtime::VoiceEngineInstance& engine) {
  aivs_get_metrics_request_t request{
      sizeof(aivs_get_metrics_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
  aivs_engine_metrics_t result{
      sizeof(aivs_engine_metrics_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      0U,
      0U,
      0U,
      0U,
      {0U, 0U},
  };
  assert(engine.get_metrics(&request, &result) == ErrorCode::Success);
  return result;
}

void assert_fixed_output(const std::array<float, kSampleCount>& output) {
  static_assert(std::endian::native == std::endian::little);
  for (std::size_t index = 0U; index < output.size(); ++index) {
    const auto bits = std::bit_cast<std::uint32_t>(output[index]);
    assert(bits == kExpectedOutputBits[index % kExpectedOutputBits.size()]);
    const auto bytes = std::bit_cast<std::array<std::uint8_t, 4>>(output[index]);
    assert(bytes == kExpectedOutputBytes[index % kExpectedOutputBytes.size()]);
  }
  assert(checksum(output) == kExpectedChecksum);
}

void dynamic_c_abi_pipeline_pins_output_reset_and_exact_metrics(
    const std::filesystem::path& plugin_path) {
  auto loaded = load_engine(plugin_path, R"({"work_iterations":0})");
  const auto input = fixed_input();
  auto prepared = prepare(*loaded.instance);
  assert(prepared.stream_generation == 1U);
  auto before = metrics(*loaded.instance);
  assert(before.process_call_count == 0U);
  assert(before.input_frame_count == 0U);
  assert(before.output_frame_count == 0U);
  assert(before.process_error_count == 0U);

  std::array<float, kSampleCount> first_output{};
  auto request = process_request(input, prepared.stream_generation);
  auto result = process_result(first_output);
  assert(loaded.instance->process_audio(&request, &result) == ErrorCode::Success);
  assert(result.processed_frame_count == kFrames);
  assert(result.output.frames_written_or_required == kFrames);
  assert_fixed_output(first_output);

  std::array<float, kSampleCount> short_output{};
  auto short_result = process_result(short_output);
  short_result.output.frame_capacity = kFrames - 1U;
  assert(loaded.instance->process_audio(&request, &short_result) ==
         ErrorCode::BufferTooSmall);
  auto after_failure = metrics(*loaded.instance);
  assert(after_failure.process_call_count == 1U);
  assert(after_failure.input_frame_count == kFrames);
  assert(after_failure.output_frame_count == kFrames);
  assert(after_failure.process_error_count == 1U);

  aivs_reset_request_t reset_request{
      sizeof(aivs_reset_request_t),
      AIVS_VOICE_ENGINE_ABI_V1_VERSION,
      AIVS_RESET_REASON_CALLER_REQUEST,
      0U,
      {0U, 0U},
  };
  auto reset_result = aivs_reset_result_t AIVS_RESET_RESULT_INIT;
  assert(loaded.instance->reset(&reset_request, &reset_result) == ErrorCode::Success);
  assert(reset_result.stream_generation == 2U);
  auto stale_result = process_result(short_output);
  assert(loaded.instance->process_audio(&request, &stale_result) == ErrorCode::InvalidState);

  prepared = prepare(*loaded.instance);
  assert(prepared.stream_generation == 3U);
  request.stream_generation = prepared.stream_generation;
  std::array<float, kSampleCount> second_output{};
  result = process_result(second_output);
  assert(loaded.instance->process_audio(&request, &result) == ErrorCode::Success);
  assert_fixed_output(second_output);
  assert(first_output == second_output);

  const auto final_metrics = metrics(*loaded.instance);
  assert(final_metrics.process_call_count == 2U);
  assert(final_metrics.input_frame_count == 2U * kFrames);
  assert(final_metrics.output_frame_count == 2U * kFrames);
  assert(final_metrics.process_error_count == 2U);
}

void configured_work_has_a_conservative_lower_bound(
    const std::filesystem::path& plugin_path) {
  auto loaded = load_engine(plugin_path, R"({"work_iterations":1000000})");
  const auto prepared = prepare(*loaded.instance);
  const auto input = fixed_input();
  auto request = process_request(input, prepared.stream_generation);
  std::array<float, kSampleCount> output{};
  auto result = process_result(output);

  const auto started = std::chrono::steady_clock::now();
  for (std::size_t run = 0U; run < 3U; ++run) {
    assert(loaded.instance->process_audio(&request, &result) == ErrorCode::Success);
  }
  const auto elapsed = std::chrono::steady_clock::now() - started;
  assert(elapsed >= std::chrono::microseconds(150));
  assert_fixed_output(output);
  const auto final_metrics = metrics(*loaded.instance);
  assert(final_metrics.process_call_count == 3U);
  assert(final_metrics.input_frame_count == 3U * kFrames);
  assert(final_metrics.output_frame_count == 3U * kFrames);
  assert(final_metrics.process_error_count == 0U);
}

void runtime_session_returns_only_the_fixed_pipeline_summary(
    const std::filesystem::path& plugin_path) {
  auto created = runtime::MockPipeline::create(plugin_path, 0U);
  assert(created.error_code == ErrorCode::Success);
  assert(created.pipeline);
  runtime::Session session(41U, created.pipeline.get());
  assert(session.start().has_value());
  const message::RuntimeMessage request{
      message::MessageKind::Request,
      ai_voice::contracts::kIpcProtocolCurrentVersion,
      73U,
      message::Command::RunMockPipeline,
      ErrorCode::Success,
      {},
  };
  const auto dispatched = session.handle(request);
  assert(dispatched.disposition == runtime::DispatchDisposition::Respond);
  assert(dispatched.response.has_value());
  assert(dispatched.response->kind == message::MessageKind::Response);
  assert(dispatched.response->request_id == 73U);
  assert(dispatched.response->command == message::Command::RunMockPipeline);
  assert(dispatched.response->error_code == ErrorCode::Success);
  assert(dispatched.response->payload.size() == runtime::kMockPipelineResultSizeBytes);
  const std::span<const std::uint8_t> summary(dispatched.response->payload);
  assert(read_u32(summary, 0U) == 1U);
  assert(read_u32(summary, 4U) == 80U);
  assert(read_u32(summary, 8U) == kFrames);
  assert(read_u32(summary, 12U) == kChannels);
  assert(read_u64(summary, 16U) == kExpectedChecksum);
  assert(read_u64(summary, 32U) == 0U);
  assert(read_u64(summary, 40U) == 1U);
  assert(read_u64(summary, 48U) == 1U);
  assert(read_u64(summary, 56U) == kFrames);
  assert(read_u64(summary, 64U) == kFrames);
  assert(read_u64(summary, 72U) == 0U);
}

}  // namespace

int main(int argc, char** argv) {
  assert(argc == 2);
  const auto plugin_path = std::filesystem::absolute(std::filesystem::path(argv[1]));
  dynamic_c_abi_pipeline_pins_output_reset_and_exact_metrics(plugin_path);
  configured_work_has_a_conservative_lower_bound(plugin_path);
  runtime_session_returns_only_the_fixed_pipeline_summary(plugin_path);
}
