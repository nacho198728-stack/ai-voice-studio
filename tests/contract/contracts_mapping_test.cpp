#include <cassert>

#include "ai_voice_contracts/generated_contracts.hpp"

int main() {
  using namespace ai_voice::contracts;

  assert(kRuntimeVersion == "0.0.0");
  assert(kIpcProtocolCurrentVersion == 1U);
  assert(kIpcProtocolMinimumCompatibleVersion == 1U);
  assert(is_ipc_protocol_compatible(1U));
  assert(!is_ipc_protocol_compatible(0U));
  assert(!is_ipc_protocol_compatible(2U));
  assert(kVoiceEngineAbiCurrentVersion == 1U);
  assert(kVoiceEngineAbiMinimumCompatibleVersion == 1U);
  assert(kVoiceEngineAbiV1Version == 1U);
  assert(is_voice_engine_abi_compatible(1U));
  assert(!is_voice_engine_abi_compatible(0U));

  assert(error_code_value(ErrorCode::Success) == 0);
  assert(error_code_value(ErrorCode::UnsupportedProtocolVersion) == 1000);
  assert(error_code_value(ErrorCode::MalformedFrame) == 1100);
  assert(error_code_value(ErrorCode::RuntimeUnavailable) == 1200);
  assert(error_code_value(ErrorCode::EngineUnavailable) == 1300);
  assert(error_code_value(ErrorCode::InternalError) == 1900);
  assert(error_code_from_value(1101) == ErrorCode::FrameTooLarge);
  assert(!error_code_from_value(42).has_value());
  assert(error_code_category(ErrorCode::MalformedFrame) == ErrorCategory::Framing);
  assert(error_code_category(ErrorCode::InternalError) == ErrorCategory::Internal);
}
