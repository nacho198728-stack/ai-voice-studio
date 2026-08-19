#pragma once

#include <cstdint>
#include <filesystem>
#include <memory>
#include <string>

#include <ai_voice_contracts/generated_contracts.hpp>
#include <ai_voice_runtime/session.hpp>
#include <ai_voice_runtime/voice_engine_loader.hpp>

namespace ai_voice::runtime {

inline constexpr std::uint32_t kMockMaximumWorkIterations = 1'000'000U;
inline constexpr std::uint32_t kMockPipelineResultSchemaVersion = 1U;
inline constexpr std::uint32_t kMockPipelineResultSizeBytes = 80U;
inline constexpr std::uint32_t kMockPipelineFrames = 128U;
inline constexpr std::uint32_t kMockPipelineChannels = 2U;

class MockPipeline;

struct MockPipelineCreateResult {
  std::unique_ptr<MockPipeline> pipeline;
  contracts::ErrorCode error_code;
  std::string diagnostic;
};

class MockPipeline final : public PipelineService {
 public:
  static MockPipelineCreateResult create(
      const std::filesystem::path& plugin_path, std::uint32_t work_iterations) noexcept;

  MockPipeline(const MockPipeline&) = delete;
  MockPipeline& operator=(const MockPipeline&) = delete;
  ~MockPipeline() override = default;

  [[nodiscard]] bool available() const noexcept override;
  PipelineRunResult run(std::span<const std::uint8_t> request_payload) noexcept override;

 private:
  explicit MockPipeline(std::unique_ptr<VoiceEngineInstance> engine) noexcept;

  std::unique_ptr<VoiceEngineInstance> engine_;
};

}  // namespace ai_voice::runtime
