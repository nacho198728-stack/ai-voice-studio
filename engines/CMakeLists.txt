add_library(aivs_mock_voice_engine SHARED mock/mock_voice_engine.cpp)
target_link_libraries(aivs_mock_voice_engine PRIVATE ai_voice::voice_engine_abi)
# The Mock audio callback is a realtime boundary. Keep its complete dependency
# surface logging-free; the source contract test separately pins the exact hot
# path implementation and its relaxed atomic metrics.
get_target_property(AIVS_MOCK_ENGINE_LINK_LIBRARIES aivs_mock_voice_engine LINK_LIBRARIES)
if(NOT AIVS_MOCK_ENGINE_LINK_LIBRARIES STREQUAL "ai_voice::voice_engine_abi")
  message(FATAL_ERROR "Mock VoiceEngine dependency surface changed; logging is prohibited")
endif()
get_target_property(AIVS_VOICE_ENGINE_ABI_LINK_LIBRARIES ai_voice_engine_abi INTERFACE_LINK_LIBRARIES)
if(NOT AIVS_VOICE_ENGINE_ABI_LINK_LIBRARIES STREQUAL "ai_voice::contracts")
  message(FATAL_ERROR "Mock VoiceEngine ABI dependency surface changed; logging is prohibited")
endif()
target_compile_definitions(aivs_mock_voice_engine PRIVATE AIVS_VOICE_ENGINE_BUILD)
set_target_properties(
  aivs_mock_voice_engine
  PROPERTIES
    CXX_VISIBILITY_PRESET hidden
    VISIBILITY_INLINES_HIDDEN YES
)
