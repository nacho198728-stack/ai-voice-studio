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
set_tests_properties(aivs_runtime_message_cpp_test PROPERTIES TIMEOUT 20)

add_executable(aivs_voice_engine_c_abi_c_test contract/voice_engine_c_abi_c_test.c)
target_link_libraries(aivs_voice_engine_c_abi_c_test PRIVATE ai_voice::voice_engine_abi)
set_target_properties(
  aivs_voice_engine_c_abi_c_test
  PROPERTIES C_STANDARD 17 C_STANDARD_REQUIRED YES C_EXTENSIONS NO
)
add_test(NAME aivs_voice_engine_c_abi_c_test COMMAND aivs_voice_engine_c_abi_c_test)

add_executable(aivs_voice_engine_c_abi_cpp_test contract/voice_engine_c_abi_cpp_test.cpp)
target_link_libraries(aivs_voice_engine_c_abi_cpp_test PRIVATE ai_voice::voice_engine_abi)
set_target_properties(
  aivs_voice_engine_c_abi_cpp_test
  PROPERTIES CXX_STANDARD 20 CXX_STANDARD_REQUIRED YES CXX_EXTENSIONS NO
)
add_test(NAME aivs_voice_engine_c_abi_cpp_test COMMAND aivs_voice_engine_c_abi_cpp_test)

add_test(
  NAME aivs_voice_engine_c_abi_c_rejects_unsigned_error_alias
  COMMAND
    ${CMAKE_COMMAND}
    "-DCOMPILER=${CMAKE_C_COMPILER}"
    -DLANGUAGE=C
    "-DSOURCE=${CMAKE_CURRENT_SOURCE_DIR}/fixtures/voice_engine_unsigned_error_alias_test.c"
    "-DOBJECT=${CMAKE_CURRENT_BINARY_DIR}/voice-engine-unsigned-error-alias-c.obj"
    "-DVOICE_ENGINE_INCLUDE=${PROJECT_SOURCE_DIR}/runtime/api"
    "-DCONTRACTS_INCLUDE=${PROJECT_SOURCE_DIR}/core/contracts/include"
    "-DMSVC_PLATFORM=${MSVC}"
    -P ${CMAKE_CURRENT_SOURCE_DIR}/contract/voice_engine_adversarial_compile.cmake
)
add_test(
  NAME aivs_voice_engine_c_abi_cpp_rejects_unsigned_error_alias
  COMMAND
    ${CMAKE_COMMAND}
    "-DCOMPILER=${CMAKE_CXX_COMPILER}"
    -DLANGUAGE=CXX
    "-DSOURCE=${CMAKE_CURRENT_SOURCE_DIR}/fixtures/voice_engine_unsigned_error_alias_test.cpp"
    "-DOBJECT=${CMAKE_CURRENT_BINARY_DIR}/voice-engine-unsigned-error-alias-cpp.obj"
    "-DVOICE_ENGINE_INCLUDE=${PROJECT_SOURCE_DIR}/runtime/api"
    "-DCONTRACTS_INCLUDE=${PROJECT_SOURCE_DIR}/core/contracts/include"
    "-DMSVC_PLATFORM=${MSVC}"
    -P ${CMAKE_CURRENT_SOURCE_DIR}/contract/voice_engine_adversarial_compile.cmake
)

foreach(target IN ITEMS aivs_voice_engine_c_abi_c_test aivs_voice_engine_c_abi_cpp_test)
  if(MSVC)
    target_compile_options(${target} PRIVATE /W4 /WX)
  else()
    target_compile_options(${target} PRIVATE -Wall -Wextra -Wpedantic -Werror)
  endif()
endforeach()
if(MSVC)
  target_compile_options(
    aivs_voice_engine_c_abi_cpp_test
    PRIVATE /permissive- /Zc:__cplusplus
  )
endif()

add_executable(aivs_mock_voice_engine_dynamic_test runtime/mock_voice_engine_dynamic_test.cpp)
target_link_libraries(aivs_mock_voice_engine_dynamic_test PRIVATE ai_voice::voice_engine_abi)
if(CMAKE_DL_LIBS)
  target_link_libraries(aivs_mock_voice_engine_dynamic_test PRIVATE ${CMAKE_DL_LIBS})
endif()
add_test(
  NAME aivs_mock_voice_engine_dynamic_test
  COMMAND aivs_mock_voice_engine_dynamic_test $<TARGET_FILE:aivs_mock_voice_engine>
)

add_executable(
  aivs_mock_voice_engine_contract_matrix_test
  runtime/mock_voice_engine_contract_matrix_test.cpp
)
target_link_libraries(
  aivs_mock_voice_engine_contract_matrix_test
  PRIVATE ai_voice::voice_engine_abi
)
if(CMAKE_DL_LIBS)
  target_link_libraries(aivs_mock_voice_engine_contract_matrix_test PRIVATE ${CMAKE_DL_LIBS})
endif()
add_test(
  NAME aivs_mock_voice_engine_contract_matrix_test
  COMMAND
    aivs_mock_voice_engine_contract_matrix_test
    $<TARGET_FILE:aivs_mock_voice_engine>
    $<TARGET_FILE:aivs_fixture_mock_generation_exhaustion>
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
if(MSVC)
  set(AIVS_MOCK_EXPORT_INSPECTOR "${CMAKE_DUMPBIN}")
  if(NOT AIVS_MOCK_EXPORT_INSPECTOR)
    unset(AIVS_MOCK_EXPORT_INSPECTOR)
    find_program(AIVS_MOCK_EXPORT_INSPECTOR NAMES dumpbin REQUIRED)
  endif()
  add_test(
    NAME aivs_mock_voice_engine_exports
    COMMAND
      ${CMAKE_COMMAND}
      "-DINSPECTOR=${AIVS_MOCK_EXPORT_INSPECTOR}"
      "-DLIBRARY=$<TARGET_FILE:aivs_mock_voice_engine>"
      -DMSVC_PLATFORM=TRUE
      -P ${CMAKE_CURRENT_SOURCE_DIR}/runtime/mock_voice_engine_exports.cmake
  )
endif()

add_test(
  NAME aivs_mock_voice_engine_msvc_export_parser_accepts_factory
  COMMAND
    ${CMAKE_COMMAND}
    -DMSVC_PLATFORM=TRUE
    -DOUTPUT_FIXTURE=${CMAKE_CURRENT_SOURCE_DIR}/fixtures/msvc-exports-factory-only.txt
    -P ${CMAKE_CURRENT_SOURCE_DIR}/runtime/mock_voice_engine_exports.cmake
)
add_test(
  NAME aivs_mock_voice_engine_msvc_export_parser_rejects_extra
  COMMAND
    ${CMAKE_COMMAND}
    -DMSVC_PLATFORM=TRUE
    -DOUTPUT_FIXTURE=${CMAKE_CURRENT_SOURCE_DIR}/fixtures/msvc-exports-extra-symbol.txt
    -P ${CMAKE_CURRENT_SOURCE_DIR}/runtime/mock_voice_engine_exports.cmake
)
set_tests_properties(
  aivs_mock_voice_engine_msvc_export_parser_rejects_extra
  PROPERTIES WILL_FAIL TRUE
)

add_executable(aivs_voice_engine_loader_test runtime/voice_engine_loader_test.cpp)
find_package(Threads REQUIRED)
target_link_libraries(
  aivs_voice_engine_loader_test
  PRIVATE ai_voice::runtime_loader Threads::Threads
)
add_test(
  NAME aivs_voice_engine_loader_test
  COMMAND
    aivs_voice_engine_loader_test
    $<TARGET_FILE:aivs_mock_voice_engine>
    $<TARGET_FILE:aivs_fixture_missing_factory>
    $<TARGET_FILE:aivs_fixture_unsupported_factory>
    $<TARGET_FILE:aivs_fixture_incomplete_factory>
    $<TARGET_FILE:aivs_fixture_initialize_failure>
    $<TARGET_FILE:aivs_fixture_shutdown_failure>
    $<TARGET_FILE:aivs_fixture_blocking_metrics>
)

add_executable(aivs_mock_pipeline_test runtime/mock_pipeline_test.cpp)
target_link_libraries(aivs_mock_pipeline_test PRIVATE ai_voice::mock_pipeline)
add_test(
  NAME aivs_mock_pipeline_test
  COMMAND aivs_mock_pipeline_test $<TARGET_FILE:aivs_mock_voice_engine>
)

add_executable(
  aivs_mock_pipeline_benchmark_gate
  benchmark/mock_pipeline_benchmark_test.cpp
)
target_link_libraries(
  aivs_mock_pipeline_benchmark_gate
  PRIVATE ai_voice::mock_pipeline
)
add_test(
  NAME aivs_mock_pipeline_benchmark_gate
  COMMAND
    aivs_mock_pipeline_benchmark_gate
    $<TARGET_FILE:aivs_mock_voice_engine>
)
set_tests_properties(
  aivs_mock_pipeline_benchmark_gate
  PROPERTIES LABELS benchmark TIMEOUT 20
)

add_test(
  NAME aivs_mock_process_audio_no_logging
  COMMAND
    ${CMAKE_COMMAND}
    -DSOURCE=${PROJECT_SOURCE_DIR}/engines/mock/mock_voice_engine.cpp
    -P ${CMAKE_CURRENT_SOURCE_DIR}/runtime/mock_process_audio_no_logging.cmake
)

add_executable(aivs_runtime_options_test runtime/runtime_options_test.cpp)
target_link_libraries(aivs_runtime_options_test PRIVATE ai_voice::runtime_options)
add_test(NAME aivs_runtime_options_test COMMAND aivs_runtime_options_test)

add_executable(aivs_runtime_logging_test runtime/runtime_logging_test.cpp)
target_link_libraries(aivs_runtime_logging_test PRIVATE ai_voice::runtime_logging)
add_test(NAME aivs_runtime_logging_test COMMAND aivs_runtime_logging_test)
set_tests_properties(aivs_runtime_logging_test PROPERTIES TIMEOUT 20)

add_executable(aivs_runtime_late_log_failure_test runtime/runtime_late_log_failure_test.cpp)
target_link_libraries(
  aivs_runtime_late_log_failure_test
  PRIVATE ai_voice::contracts
)
add_test(
  NAME aivs_runtime_late_log_failure_test
  COMMAND aivs_runtime_late_log_failure_test $<TARGET_FILE:voice-runtime>
)
set_tests_properties(aivs_runtime_late_log_failure_test PROPERTIES SKIP_RETURN_CODE 77)

add_executable(aivs_runtime_session_test runtime/runtime_session_test.cpp)
target_link_libraries(aivs_runtime_session_test PRIVATE ai_voice_runtime_session)
add_test(NAME aivs_runtime_session_test COMMAND aivs_runtime_session_test)

add_executable(aivs_stdio_runtime_test runtime/stdio_runtime_test.cpp)
target_link_libraries(aivs_stdio_runtime_test PRIVATE ai_voice_runtime_stdio)
add_test(NAME aivs_stdio_runtime_test COMMAND aivs_stdio_runtime_test)
set_tests_properties(aivs_stdio_runtime_test PROPERTIES TIMEOUT 20)

add_executable(aivs_voice_runtime_smoke runtime/voice_runtime_smoke.cpp)
target_link_libraries(aivs_voice_runtime_smoke PRIVATE ai_voice_contracts)

add_executable(
  aivs_runtime_manager_child_fixture
  fixtures/runtime_manager_child_fixture.cpp
)
target_link_libraries(aivs_runtime_manager_child_fixture PRIVATE ai_voice_contracts)

add_test(
  NAME aivs_voice_runtime_smoke
  COMMAND
    ${CMAKE_COMMAND}
    -DVOICE_RUNTIME=$<TARGET_FILE:voice-runtime>
    -DMOCK_PLUGIN=$<TARGET_FILE:aivs_mock_voice_engine>
    -DUNSUPPORTED_PLUGIN=$<TARGET_FILE:aivs_fixture_unsupported_factory>
    -DINITIALIZE_FAILURE_PLUGIN=$<TARGET_FILE:aivs_fixture_initialize_failure>
    -DSMOKE_HELPER=$<TARGET_FILE:aivs_voice_runtime_smoke>
    -DSMOKE_DIRECTORY=${CMAKE_CURRENT_BINARY_DIR}/voice-runtime-smoke
    -P ${CMAKE_CURRENT_SOURCE_DIR}/runtime/voice_runtime_smoke.cmake
)

find_program(AIVS_CARGO_EXECUTABLE cargo REQUIRED)
add_test(
  NAME aivs_runtime_manager_child_test
  COMMAND
    ${CMAKE_COMMAND} -E env
    AIVS_RUNTIME_PATH=$<TARGET_FILE:voice-runtime>
    AIVS_MOCK_PLUGIN_PATH=$<TARGET_FILE:aivs_mock_voice_engine>
    CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/rust-target
    ${AIVS_CARGO_EXECUTABLE} test
    --locked
    -p ai-voice-runtime-host
    --test runtime_manager_process
    --
    --ignored
    --nocapture
    --test-threads=1
)
set_tests_properties(
  aivs_runtime_manager_child_test
  PROPERTIES
    WORKING_DIRECTORY ${PROJECT_SOURCE_DIR}
    TIMEOUT 60
)

add_test(
  NAME aivs_runtime_manager_fixture_test
  COMMAND
    ${CMAKE_COMMAND} -E env
    AIVS_RUNTIME_FIXTURE_PATH=$<TARGET_FILE:aivs_runtime_manager_child_fixture>
    AIVS_TEST_SECRET=must-not-inherit
    CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/rust-target
    ${AIVS_CARGO_EXECUTABLE} test
    --locked
    -p ai-voice-runtime-host
    --test runtime_manager_fixture
    --
    --ignored
    --nocapture
    --test-threads=1
)
set_tests_properties(
  aivs_runtime_manager_fixture_test
  PROPERTIES
    WORKING_DIRECTORY ${PROJECT_SOURCE_DIR}
    TIMEOUT 60
)

add_test(
  NAME aivs_runtime_manager_gate_test
  COMMAND
    ${CMAKE_COMMAND} -E env
    AIVS_RUNTIME_PATH=$<TARGET_FILE:voice-runtime>
    AIVS_MOCK_PLUGIN_PATH=$<TARGET_FILE:aivs_mock_voice_engine>
    AIVS_RUNTIME_FIXTURE_PATH=$<TARGET_FILE:aivs_runtime_manager_child_fixture>
    CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/rust-target
    ${AIVS_CARGO_EXECUTABLE} test
    --locked
    -p ai-voice-runtime-host
    --test runtime_manager_gate
    --
    --ignored
    --nocapture
    --test-threads=1
)
set_tests_properties(
  aivs_runtime_manager_gate_test
  PROPERTIES
    WORKING_DIRECTORY ${PROJECT_SOURCE_DIR}
    TIMEOUT 90
)

add_test(
  NAME aivs_desktop_exit_runtime_race_test
  COMMAND
    ${CMAKE_COMMAND} -E env
    AIVS_RUNTIME_FIXTURE_PATH=$<TARGET_FILE:aivs_runtime_manager_child_fixture>
    CARGO_TARGET_DIR=${CMAKE_BINARY_DIR}/rust-target
    ${AIVS_CARGO_EXECUTABLE} test
    --locked
    -p ai-voice-desktop
    --test exit_runtime_race
    --
    --ignored
    --nocapture
    --test-threads=1
)
set_tests_properties(
  aivs_desktop_exit_runtime_race_test
  PROPERTIES
    WORKING_DIRECTORY ${PROJECT_SOURCE_DIR}
    TIMEOUT 60
)

add_library(aivs_fixture_missing_factory SHARED fixtures/missing_factory.cpp)
set_target_properties(
  aivs_fixture_missing_factory
  PROPERTIES CXX_VISIBILITY_PRESET hidden VISIBILITY_INLINES_HIDDEN YES
)

add_library(
  aivs_fixture_mock_generation_exhaustion
  SHARED
  ../engines/mock/mock_voice_engine.cpp
)
target_link_libraries(
  aivs_fixture_mock_generation_exhaustion
  PRIVATE ai_voice::voice_engine_abi
)
target_compile_definitions(
  aivs_fixture_mock_generation_exhaustion
  PRIVATE
    AIVS_VOICE_ENGINE_BUILD
    AIVS_MOCK_ENABLE_INITIAL_GENERATION_TEST_SEAM
)
set_target_properties(
  aivs_fixture_mock_generation_exhaustion
  PROPERTIES CXX_VISIBILITY_PRESET hidden VISIBILITY_INLINES_HIDDEN YES
)

function(add_voice_engine_failure_fixture target mode)
  add_library(${target} SHARED fixtures/voice_engine_failure_fixture.cpp)
  target_link_libraries(${target} PRIVATE ai_voice::voice_engine_abi)
  target_compile_definitions(
    ${target}
    PRIVATE AIVS_VOICE_ENGINE_BUILD AIVS_FAILURE_FIXTURE_MODE=${mode}
  )
  set_target_properties(
    ${target}
    PROPERTIES CXX_VISIBILITY_PRESET hidden VISIBILITY_INLINES_HIDDEN YES
  )
endfunction()

add_voice_engine_failure_fixture(aivs_fixture_unsupported_factory 1)
add_voice_engine_failure_fixture(aivs_fixture_incomplete_factory 2)
add_voice_engine_failure_fixture(aivs_fixture_initialize_failure 3)
add_voice_engine_failure_fixture(aivs_fixture_shutdown_failure 4)
add_voice_engine_failure_fixture(aivs_fixture_blocking_metrics 5)
target_link_libraries(aivs_fixture_blocking_metrics PRIVATE Threads::Threads)
