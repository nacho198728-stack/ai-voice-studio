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

}  // namespace

int main(int argc, char** argv) {
  assert(argc == 3);
  const std::string mode(argv[1]);
  const std::string path(argv[2]);
  if (mode == "write-shutdown") {
    write_shutdown(path);
    return 0;
  }

  const auto frames = decode_file(path);
  if (mode == "verify-eof") {
    assert(frames.size() == 1U);
    verify_hello(frames[0]);
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
