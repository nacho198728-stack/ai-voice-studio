add_library(aivs_mock_voice_engine SHARED mock/mock_voice_engine.cpp)
target_link_libraries(aivs_mock_voice_engine PRIVATE ai_voice::voice_engine_abi)
target_compile_definitions(aivs_mock_voice_engine PRIVATE AIVS_VOICE_ENGINE_BUILD)
set_target_properties(
  aivs_mock_voice_engine
  PROPERTIES
    CXX_VISIBILITY_PRESET hidden
    VISIBILITY_INLINES_HIDDEN YES
)
