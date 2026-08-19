# Cross-language contract headers are generated from canonical JSON by
# tools/scripts/contracts.mjs. Native consumers only include the checked-in
# output; configuring or building never mutates the source tree.
add_library(ai_voice_contracts INTERFACE)
add_library(ai_voice::contracts ALIAS ai_voice_contracts)

target_include_directories(
  ai_voice_contracts
  INTERFACE
    "${CMAKE_CURRENT_SOURCE_DIR}/contracts/include"
)
