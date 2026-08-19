# The stable C plugin ABI has no implementation in Phase 0.5 Task 6. This
# interface target owns its public include path and canonical C error/version
# mapping without introducing a runtime, loader, or engine implementation.
add_library(ai_voice_engine_abi INTERFACE)
add_library(ai_voice::voice_engine_abi ALIAS ai_voice_engine_abi)

target_include_directories(
  ai_voice_engine_abi
  INTERFACE
    "${CMAKE_CURRENT_SOURCE_DIR}/api"
)
target_link_libraries(ai_voice_engine_abi INTERFACE ai_voice::contracts)
