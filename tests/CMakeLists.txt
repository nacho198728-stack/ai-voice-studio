add_executable(aivs_contract_mapping_test contract/contracts_mapping_test.cpp)
target_link_libraries(aivs_contract_mapping_test PRIVATE ai_voice_contracts)

add_test(NAME aivs_contract_mapping_test COMMAND aivs_contract_mapping_test)

add_executable(aivs_runtime_message_cpp_test contract/runtime_message_cpp_test.cpp)
target_link_libraries(aivs_runtime_message_cpp_test PRIVATE ai_voice_contracts)
target_compile_definitions(
  aivs_runtime_message_cpp_test
  PRIVATE
    AIVS_RUNTIME_MESSAGE_FIXTURE_DIRECTORY="${PROJECT_SOURCE_DIR}/core/contracts/fixtures"
)
add_test(NAME aivs_runtime_message_cpp_test COMMAND aivs_runtime_message_cpp_test)

add_executable(aivs_voice_engine_c_abi_c_test contract/voice_engine_c_abi_c_test.c)
target_link_libraries(aivs_voice_engine_c_abi_c_test PRIVATE ai_voice::voice_engine_abi)
add_test(NAME aivs_voice_engine_c_abi_c_test COMMAND aivs_voice_engine_c_abi_c_test)

add_executable(aivs_voice_engine_c_abi_cpp_test contract/voice_engine_c_abi_cpp_test.cpp)
target_link_libraries(aivs_voice_engine_c_abi_cpp_test PRIVATE ai_voice::voice_engine_abi)
add_test(NAME aivs_voice_engine_c_abi_cpp_test COMMAND aivs_voice_engine_c_abi_cpp_test)

add_executable(aivs_mock_voice_engine_dynamic_test runtime/mock_voice_engine_dynamic_test.cpp)
target_link_libraries(aivs_mock_voice_engine_dynamic_test PRIVATE ai_voice::voice_engine_abi)
if(CMAKE_DL_LIBS)
  target_link_libraries(aivs_mock_voice_engine_dynamic_test PRIVATE ${CMAKE_DL_LIBS})
endif()
add_test(
  NAME aivs_mock_voice_engine_dynamic_test
  COMMAND aivs_mock_voice_engine_dynamic_test $<TARGET_FILE:aivs_mock_voice_engine>
)
if(UNIX)
  add_test(
    NAME aivs_mock_voice_engine_exports
    COMMAND
      ${CMAKE_COMMAND}
      -DNM=${CMAKE_NM}
      -DLIBRARY=$<TARGET_FILE:aivs_mock_voice_engine>
      -DAPPLE_PLATFORM=${APPLE}
      -P ${CMAKE_CURRENT_SOURCE_DIR}/runtime/mock_voice_engine_exports.cmake
  )
endif()

add_executable(aivs_voice_engine_loader_test runtime/voice_engine_loader_test.cpp)
target_link_libraries(aivs_voice_engine_loader_test PRIVATE ai_voice::runtime_loader)
add_test(
  NAME aivs_voice_engine_loader_test
  COMMAND
    aivs_voice_engine_loader_test
    $<TARGET_FILE:aivs_mock_voice_engine>
    $<TARGET_FILE:voice-runtime>
)

add_executable(aivs_mock_pipeline_test runtime/mock_pipeline_test.cpp)
target_link_libraries(aivs_mock_pipeline_test PRIVATE ai_voice::mock_pipeline)
add_test(
  NAME aivs_mock_pipeline_test
  COMMAND aivs_mock_pipeline_test $<TARGET_FILE:aivs_mock_voice_engine>
)

add_executable(aivs_runtime_options_test runtime/runtime_options_test.cpp)
target_link_libraries(aivs_runtime_options_test PRIVATE ai_voice::runtime_options)
add_test(NAME aivs_runtime_options_test COMMAND aivs_runtime_options_test)

add_executable(aivs_runtime_session_test runtime/runtime_session_test.cpp)
target_link_libraries(aivs_runtime_session_test PRIVATE ai_voice_runtime_session)
add_test(NAME aivs_runtime_session_test COMMAND aivs_runtime_session_test)

add_executable(aivs_stdio_runtime_test runtime/stdio_runtime_test.cpp)
target_link_libraries(aivs_stdio_runtime_test PRIVATE ai_voice_runtime_stdio)
add_test(NAME aivs_stdio_runtime_test COMMAND aivs_stdio_runtime_test)

add_executable(aivs_voice_runtime_smoke runtime/voice_runtime_smoke.cpp)
target_link_libraries(aivs_voice_runtime_smoke PRIVATE ai_voice_contracts)

add_test(
  NAME aivs_voice_runtime_smoke
  COMMAND
    ${CMAKE_COMMAND}
    -DVOICE_RUNTIME=$<TARGET_FILE:voice-runtime>
    -DMOCK_PLUGIN=$<TARGET_FILE:aivs_mock_voice_engine>
    -DSMOKE_HELPER=$<TARGET_FILE:aivs_voice_runtime_smoke>
    -DSMOKE_DIRECTORY=${CMAKE_CURRENT_BINARY_DIR}/voice-runtime-smoke
    -P ${CMAKE_CURRENT_SOURCE_DIR}/runtime/voice_runtime_smoke.cmake
)
