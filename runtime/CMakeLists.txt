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

add_library(ai_voice_runtime_session STATIC session/session.cpp)
add_library(ai_voice::runtime_session ALIAS ai_voice_runtime_session)
target_include_directories(
  ai_voice_runtime_session
  PUBLIC
    "${CMAKE_CURRENT_SOURCE_DIR}/session/include"
)
target_link_libraries(ai_voice_runtime_session PUBLIC ai_voice::contracts)

add_library(ai_voice_runtime_loader STATIC loader/voice_engine_loader.cpp)
add_library(ai_voice::runtime_loader ALIAS ai_voice_runtime_loader)
target_include_directories(
  ai_voice_runtime_loader
  PUBLIC
    "${CMAKE_CURRENT_SOURCE_DIR}/loader/include"
)
target_link_libraries(ai_voice_runtime_loader PUBLIC ai_voice::voice_engine_abi)
if(CMAKE_DL_LIBS)
  target_link_libraries(ai_voice_runtime_loader PRIVATE ${CMAKE_DL_LIBS})
endif()

add_library(ai_voice_mock_pipeline STATIC pipeline/mock_pipeline.cpp)
add_library(ai_voice::mock_pipeline ALIAS ai_voice_mock_pipeline)
target_include_directories(
  ai_voice_mock_pipeline
  PUBLIC
    "${CMAKE_CURRENT_SOURCE_DIR}/pipeline/include"
)
target_link_libraries(
  ai_voice_mock_pipeline
  PUBLIC
    ai_voice::runtime_loader
    ai_voice::runtime_session
)

add_library(ai_voice_runtime_options STATIC process/runtime_options.cpp)
add_library(ai_voice::runtime_options ALIAS ai_voice_runtime_options)
target_include_directories(
  ai_voice_runtime_options
  PUBLIC
    "${CMAKE_CURRENT_SOURCE_DIR}/process/include"
)
target_link_libraries(ai_voice_runtime_options PUBLIC ai_voice::mock_pipeline)

add_library(ai_voice_runtime_stdio STATIC process/stdio_runtime.cpp)
add_library(ai_voice::runtime_stdio ALIAS ai_voice_runtime_stdio)
target_include_directories(
  ai_voice_runtime_stdio
  PUBLIC
    "${CMAKE_CURRENT_SOURCE_DIR}/process/include"
)
target_link_libraries(
  ai_voice_runtime_stdio
  PUBLIC ai_voice::runtime_session
  PRIVATE ai_voice::mock_pipeline ai_voice::runtime_options
)

add_executable(voice-runtime main.cpp)
target_link_libraries(voice-runtime PRIVATE ai_voice::runtime_stdio)
