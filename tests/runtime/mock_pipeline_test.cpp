#ifdef NDEBUG
#undef NDEBUG
#endif

#include <cassert>
#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <string>
#include <vector>

#include <ai_voice_runtime/mock_pipeline.hpp>

namespace runtime = ai_voice::runtime;
using ai_voice::contracts::ErrorCode;

namespace {

std::uint32_t read_u32(const std::vector<std::uint8_t>& bytes, std::size_t offset) {
  std::uint32_t value = 0U;
  for (std::size_t index = 0U; index < 4U; ++index) {
    value |= static_cast<std::uint32_t>(bytes[offset + index]) << (index * 8U);
  }
  return value;
}

std::uint64_t read_u64(const std::vector<std::uint8_t>& bytes, std::size_t offset) {
  std::uint64_t value = 0U;
  for (std::size_t index = 0U; index < 8U; ++index) {
    value |= static_cast<std::uint64_t>(bytes[offset + index]) << (index * 8U);
  }
  return value;
}

void deterministic_pipeline_returns_only_the_fixed_control_summary(
    const std::filesystem::path& plugin_path) {
  auto created = runtime::MockPipeline::create(plugin_path, 0U);
  assert(created.error_code == ErrorCode::Success);
  assert(created.pipeline);
  assert(created.diagnostic.empty());
  assert(created.pipeline->available());

  const auto first = created.pipeline->run({});
  assert(first.error_code == ErrorCode::Success);
  assert(first.payload.size() == 80U);
  assert(read_u32(first.payload, 0U) == 1U);
  assert(read_u32(first.payload, 4U) == 80U);
  assert(read_u32(first.payload, 8U) == 128U);
  assert(read_u32(first.payload, 12U) == 2U);
  assert(read_u64(first.payload, 16U) == UINT64_C(0x3ECD5190F6F4F725));
  assert(read_u64(first.payload, 32U) == 0U);
  assert(read_u64(first.payload, 40U) == 1U);
  assert(read_u64(first.payload, 48U) == 1U);
  assert(read_u64(first.payload, 56U) == 128U);
  assert(read_u64(first.payload, 64U) == 128U);
  assert(read_u64(first.payload, 72U) == 0U);

  const auto second = created.pipeline->run({});
  assert(second.error_code == ErrorCode::Success);
  assert(read_u64(second.payload, 16U) == UINT64_C(0x3ECD5190F6F4F725));
  assert(read_u64(second.payload, 40U) == 3U);
  assert(read_u64(second.payload, 48U) == 2U);
  assert(read_u64(second.payload, 56U) == 256U);
  assert(read_u64(second.payload, 64U) == 256U);

  const std::vector<std::uint8_t> invalid_request{0x00U};
  const auto invalid = created.pipeline->run(invalid_request);
  assert(invalid.error_code == ErrorCode::InvalidArgument);
  assert(std::string(invalid.payload.begin(), invalid.payload.end()) ==
         R"({"error":"invalid_mock_pipeline_request"})");
}

void work_iteration_bounds_are_rejected_before_loading(const std::filesystem::path& plugin_path) {
  auto rejected = runtime::MockPipeline::create(plugin_path, 1'000'001U);
  assert(rejected.error_code == ErrorCode::InvalidArgument);
  assert(!rejected.pipeline);
  assert(!rejected.diagnostic.empty());
}

}  // namespace

int main(int argc, char** argv) {
  assert(argc == 2);
  const auto plugin_path = std::filesystem::absolute(std::filesystem::path(argv[1]));
  deterministic_pipeline_returns_only_the_fixed_control_summary(plugin_path);
  work_iteration_bounds_are_rejected_before_loading(plugin_path);
}
