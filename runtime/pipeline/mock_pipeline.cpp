#include <ai_voice_runtime/mock_pipeline.hpp>

#include <array>
#include <bit>
#include <charconv>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>
#include <system_error>
#include <utility>
#include <vector>

namespace ai_voice::runtime {
namespace {

using contracts::ErrorCode;

constexpr std::uint32_t kSampleRateHz = 48'000U;
constexpr std::uint64_t kStreamId = 1U;
constexpr std::array<std::uint32_t, 8> kInputBitPattern{
    0x00000000U,
    0x3E800000U,
    0xBE800000U,
    0x3F000000U,
    0xBF000000U,
    0x3F800000U,
    0xBF800000U,
    0x80000000U,
};

std::vector<std::uint8_t> error_payload(std::string_view message) {
  return {message.begin(), message.end()};
}

PipelineRunResult failure(ErrorCode error, std::string_view message) {
  return {error, error_payload(message)};
}

aivs_bytes_view_t bytes_view(std::string_view bytes) {
  auto view = aivs_bytes_view_t AIVS_BYTES_VIEW_INIT;
  view.data = bytes.empty() ? nullptr : reinterpret_cast<const std::uint8_t*>(bytes.data());
  view.size_bytes = bytes.size();
  return view;
}

void write_u32(std::vector<std::uint8_t>& bytes, std::uint32_t value) {
  for (std::size_t index = 0U; index < 4U; ++index) {
    bytes.push_back(static_cast<std::uint8_t>(value >> (index * 8U)));
  }
}

void write_u64(std::vector<std::uint8_t>& bytes, std::uint64_t value) {
  for (std::size_t index = 0U; index < 8U; ++index) {
    bytes.push_back(static_cast<std::uint8_t>(value >> (index * 8U)));
  }
}

std::uint64_t checksum(std::span<const float> samples) noexcept {
  std::uint64_t hash = UINT64_C(0xCBF29CE484222325);
  for (const float sample : samples) {
    const auto bits = std::bit_cast<std::uint32_t>(sample);
    for (std::size_t byte_index = 0U; byte_index < 4U; ++byte_index) {
      hash ^= static_cast<std::uint8_t>(bits >> (byte_index * 8U));
      hash *= UINT64_C(0x100000001B3);
    }
  }
  return hash;
}

std::vector<std::uint8_t> encode_summary(
    std::uint64_t output_checksum,
    std::uint64_t elapsed_microseconds,
    const aivs_prepare_stream_result_t& prepared,
    const aivs_engine_metrics_t& metrics) {
  std::vector<std::uint8_t> bytes;
  bytes.reserve(kMockPipelineResultSizeBytes);
  write_u32(bytes, kMockPipelineResultSchemaVersion);
  write_u32(bytes, kMockPipelineResultSizeBytes);
  write_u32(bytes, kMockPipelineFrames);
  write_u32(bytes, kMockPipelineChannels);
  write_u64(bytes, output_checksum);
  write_u64(bytes, elapsed_microseconds);
  write_u64(bytes, prepared.algorithmic_latency_frames);
  write_u64(bytes, prepared.stream_generation);
  write_u64(bytes, metrics.process_call_count);
  write_u64(bytes, metrics.input_frame_count);
  write_u64(bytes, metrics.output_frame_count);
  write_u64(bytes, metrics.process_error_count);
  return bytes;
}

}  // namespace

MockPipeline::MockPipeline(std::unique_ptr<VoiceEngineInstance> engine) noexcept
    : engine_(std::move(engine)) {}

MockPipelineCreateResult MockPipeline::create(
    const std::filesystem::path& plugin_path, std::uint32_t work_iterations) noexcept {
  if (work_iterations > kMockMaximumWorkIterations) {
    return {nullptr, ErrorCode::InvalidArgument, "Mock work iterations exceed the fixed limit"};
  }
  try {
    auto loaded = VoiceEngineModule::load(plugin_path);
    if (loaded.error_code != ErrorCode::Success) {
      return {nullptr, loaded.error_code, std::move(loaded.diagnostic)};
    }

    std::array<char, 64> configuration{};
    constexpr std::string_view prefix = R"({"work_iterations":)";
    std::copy(prefix.begin(), prefix.end(), configuration.begin());
    const auto [digits_end, conversion_error] = std::to_chars(
        configuration.data() + static_cast<std::ptrdiff_t>(prefix.size()),
        configuration.data() + static_cast<std::ptrdiff_t>(configuration.size() - 1U),
        work_iterations);
    if (conversion_error != std::errc{}) {
      return {nullptr, ErrorCode::InternalError, "Could not construct Mock configuration"};
    }
    *digits_end = '}';
    const auto configuration_size =
        static_cast<std::size_t>(digits_end + 1 - configuration.data());
    auto initialized = loaded.module->initialize(
        std::span<const std::uint8_t>(
            reinterpret_cast<const std::uint8_t*>(configuration.data()), configuration_size));
    if (initialized.error_code != ErrorCode::Success) {
      return {nullptr, initialized.error_code, "Mock VoiceEngine initialization failed"};
    }

    constexpr std::string_view model_identifier = "mock-v1";
    aivs_load_model_request_t load_request{
        sizeof(aivs_load_model_request_t),
        AIVS_VOICE_ENGINE_ABI_V1_VERSION,
        bytes_view(model_identifier),
        AIVS_BYTES_VIEW_INIT,
        {0U, 0U},
    };
    const auto model_status = initialized.instance->load_model(&load_request);
    if (model_status != ErrorCode::Success) {
      return {nullptr, model_status, "Mock VoiceEngine model simulation failed"};
    }
    return {
        std::unique_ptr<MockPipeline>(new MockPipeline(std::move(initialized.instance))),
        ErrorCode::Success,
        {},
    };
  } catch (...) {
    return {nullptr, ErrorCode::InternalError, "Mock pipeline setup failed unexpectedly"};
  }
}

bool MockPipeline::available() const noexcept {
  return engine_ != nullptr && engine_->is_live();
}

PipelineRunResult MockPipeline::run(
    std::span<const std::uint8_t> request_payload) noexcept {
  if (!request_payload.empty()) {
    return failure(ErrorCode::InvalidArgument, R"({"error":"invalid_mock_pipeline_request"})");
  }
  if (!available()) {
    return failure(ErrorCode::EngineUnavailable, R"({"error":"engine_unavailable"})");
  }

  try {
    auto prepare_request = aivs_prepare_stream_request_t AIVS_PREPARE_STREAM_REQUEST_INIT;
    prepare_request.sample_rate_hz = kSampleRateHz;
    prepare_request.channel_count = kMockPipelineChannels;
    prepare_request.maximum_frame_count = kMockPipelineFrames;
    prepare_request.stream_id = kStreamId;
    auto prepared = aivs_prepare_stream_result_t AIVS_PREPARE_STREAM_RESULT_INIT;
    const auto prepare_status = engine_->prepare_stream(&prepare_request, &prepared);
    if (prepare_status != ErrorCode::Success) {
      return failure(prepare_status, R"({"error":"mock_prepare_failed"})");
    }

    constexpr std::size_t sample_count = kMockPipelineFrames * kMockPipelineChannels;
    std::array<float, sample_count> input{};
    std::array<float, sample_count> output{};
    for (std::size_t index = 0U; index < input.size(); ++index) {
      input[index] = std::bit_cast<float>(kInputBitPattern[index % kInputBitPattern.size()]);
    }
    aivs_process_audio_request_t process_request{
        sizeof(aivs_process_audio_request_t),
        AIVS_VOICE_ENGINE_ABI_V1_VERSION,
        AIVS_PCM_BUFFER_INIT,
        prepared.stream_generation,
        {0U, 0U},
    };
    process_request.input.samples = input.data();
    process_request.input.sample_rate_hz = kSampleRateHz;
    process_request.input.channel_count = kMockPipelineChannels;
    process_request.input.frame_count = kMockPipelineFrames;
    process_request.input.sequence = prepared.stream_generation;
    process_request.input.sample_time = 0U;
    auto processed = aivs_process_audio_result_t AIVS_PROCESS_AUDIO_RESULT_INIT;
    processed.output.samples = output.data();
    processed.output.frame_capacity = kMockPipelineFrames;

    const auto started = std::chrono::steady_clock::now();
    const auto process_status = engine_->process_audio(&process_request, &processed);
    const auto finished = std::chrono::steady_clock::now();
    if (process_status != ErrorCode::Success) {
      aivs_reset_request_t cleanup_request{
          sizeof(aivs_reset_request_t),
          AIVS_VOICE_ENGINE_ABI_V1_VERSION,
          AIVS_RESET_REASON_RECOVERY,
          0U,
          {0U, 0U},
      };
      auto cleanup_result = aivs_reset_result_t AIVS_RESET_RESULT_INIT;
      static_cast<void>(engine_->reset(&cleanup_request, &cleanup_result));
      return failure(process_status, R"({"error":"mock_process_failed"})");
    }

    aivs_get_metrics_request_t metrics_request{
        sizeof(aivs_get_metrics_request_t), AIVS_VOICE_ENGINE_ABI_V1_VERSION, {0U, 0U}};
    aivs_engine_metrics_t metrics{
        sizeof(aivs_engine_metrics_t),
        AIVS_VOICE_ENGINE_ABI_V1_VERSION,
        0U,
        0U,
        0U,
        0U,
        {0U, 0U},
    };
    const auto metrics_status = engine_->get_metrics(&metrics_request, &metrics);

    aivs_reset_request_t reset_request{
        sizeof(aivs_reset_request_t),
        AIVS_VOICE_ENGINE_ABI_V1_VERSION,
        AIVS_RESET_REASON_CALLER_REQUEST,
        0U,
        {0U, 0U},
    };
    auto reset_result = aivs_reset_result_t AIVS_RESET_RESULT_INIT;
    const auto reset_status = engine_->reset(&reset_request, &reset_result);
    if (metrics_status != ErrorCode::Success) {
      return failure(metrics_status, R"({"error":"mock_metrics_failed"})");
    }
    if (reset_status != ErrorCode::Success) {
      return failure(reset_status, R"({"error":"mock_reset_failed"})");
    }

    const auto elapsed = std::chrono::duration_cast<std::chrono::microseconds>(finished - started);
    return {
        ErrorCode::Success,
        encode_summary(
            checksum(output), static_cast<std::uint64_t>(elapsed.count()), prepared, metrics),
    };
  } catch (...) {
    return failure(ErrorCode::InternalError, R"({"error":"mock_pipeline_internal"})");
  }
}

}  // namespace ai_voice::runtime
