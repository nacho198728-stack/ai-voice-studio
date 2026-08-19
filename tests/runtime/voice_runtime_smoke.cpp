#ifdef NDEBUG
#undef NDEBUG
#endif

#include <cassert>
#include <cstdint>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

#include <ai_voice_contracts/generated_contracts.hpp>
#include <ai_voice_contracts/runtime_message.hpp>

namespace message = ai_voice::contracts::runtime_message;
using ai_voice::contracts::ErrorCode;

namespace {

std::vector<std::uint8_t> read_file(const std::string& path) {
  std::ifstream input(path, std::ios::binary);
  assert(input.good());
  return {std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>()};
}

std::vector<message::RuntimeMessage> decode_file(const std::string& path) {
  const auto bytes = read_file(path);
  message::Decoder decoder;
  const auto decoded = decoder.feed(bytes);
  assert(!decoded.error.has_value());
  assert(!decoder.finish().has_value());
  return decoded.messages;
}

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

void write_frames(const std::string& path, const std::vector<message::RuntimeMessage>& messages) {
  std::ofstream output(path, std::ios::binary | std::ios::trunc);
  assert(output.good());
  for (const auto& message : messages) {
    const auto encoded = message::encode(message);
    assert(!encoded.error.has_value());
    output.write(
        reinterpret_cast<const char*>(encoded.bytes.data()),
        static_cast<std::streamsize>(encoded.bytes.size()));
  }
  assert(output.good());
}

void write_shutdown(const std::string& path) {
  const message::RuntimeMessage request{
      message::MessageKind::Request,
      ai_voice::contracts::kIpcProtocolCurrentVersion,
      77U,
      message::Command::Shutdown,
      ErrorCode::Success,
      {},
  };
  const auto encoded = message::encode(request);
  assert(!encoded.error.has_value());
  std::ofstream output(path, std::ios::binary | std::ios::trunc);
  assert(output.good());
  output.write(
      reinterpret_cast<const char*>(encoded.bytes.data()),
      static_cast<std::streamsize>(encoded.bytes.size()));
  assert(output.good());
}

void write_pipeline_shutdown(const std::string& path) {
  write_frames(
      path,
      {
          {
              message::MessageKind::Request,
              ai_voice::contracts::kIpcProtocolCurrentVersion,
              76U,
              message::Command::RunMockPipeline,
              ErrorCode::Success,
              {},
          },
          {
              message::MessageKind::Request,
              ai_voice::contracts::kIpcProtocolCurrentVersion,
              77U,
              message::Command::Shutdown,
              ErrorCode::Success,
              {},
          },
      });
}

void verify_hello(const message::RuntimeMessage& hello) {
  assert(hello.kind == message::MessageKind::Hello);
  assert(hello.protocol_version == ai_voice::contracts::kIpcProtocolCurrentVersion);
  assert(hello.request_id == 0U);
  assert(hello.command == message::Command::None);
  assert(hello.error_code == ErrorCode::Success);
  const std::string payload(hello.payload.begin(), hello.payload.end());
  assert(payload.find("\"runtime_version\":\"0.0.0\"") != std::string::npos);
  assert(payload.find("\"protocol_version\":1") != std::string::npos);
  assert(payload.find("\"generation\":1") != std::string::npos);
  assert(payload.find("\"health\":\"starting\"") != std::string::npos);
}

void verify_pipeline(const std::vector<message::RuntimeMessage>& frames) {
  assert(frames.size() == 3U);
  verify_hello(frames[0]);
  const auto& pipeline = frames[1];
  assert(pipeline.kind == message::MessageKind::Response);
  assert(pipeline.request_id == 76U);
  assert(pipeline.command == message::Command::RunMockPipeline);
  assert(pipeline.error_code == ErrorCode::Success);
  assert(pipeline.payload.size() == 80U);
  assert(read_u32(pipeline.payload, 0U) == 1U);
  assert(read_u32(pipeline.payload, 4U) == 80U);
  assert(read_u32(pipeline.payload, 8U) == 128U);
  assert(read_u32(pipeline.payload, 12U) == 2U);
  assert(read_u64(pipeline.payload, 16U) == UINT64_C(0x3ECD5190F6F4F725));
  assert(read_u64(pipeline.payload, 32U) == 0U);
  assert(read_u64(pipeline.payload, 40U) == 1U);
  assert(read_u64(pipeline.payload, 48U) == 1U);
  assert(read_u64(pipeline.payload, 56U) == 128U);
  assert(read_u64(pipeline.payload, 64U) == 128U);
  assert(read_u64(pipeline.payload, 72U) == 0U);
  assert(frames[2] == message::RuntimeMessage({
                          message::MessageKind::Response,
                          ai_voice::contracts::kIpcProtocolCurrentVersion,
                          77U,
                          message::Command::Shutdown,
                          ErrorCode::Success,
                          {},
                      }));
}

}  // namespace

int main(int argc, char** argv) {
  assert(argc == 3);
  const std::string mode(argv[1]);
  const std::string path(argv[2]);
  if (mode == "write-shutdown") {
    write_shutdown(path);
    return 0;
  }
  if (mode == "write-pipeline-shutdown") {
    write_pipeline_shutdown(path);
    return 0;
  }

  const auto frames = decode_file(path);
  if (mode == "verify-eof") {
    assert(frames.size() == 1U);
    verify_hello(frames[0]);
    return 0;
  }
  if (mode == "verify-pipeline") {
    verify_pipeline(frames);
    return 0;
  }
  assert(mode == "verify-shutdown");
  assert(frames.size() == 2U);
  verify_hello(frames[0]);
  assert(frames[1] == message::RuntimeMessage({
                          message::MessageKind::Response,
                          ai_voice::contracts::kIpcProtocolCurrentVersion,
                          77U,
                          message::Command::Shutdown,
                          ErrorCode::Success,
                          {},
                      }));
}
