if(NOT DEFINED VOICE_RUNTIME OR NOT DEFINED MOCK_PLUGIN OR NOT DEFINED SMOKE_HELPER OR NOT DEFINED SMOKE_DIRECTORY)
  message(FATAL_ERROR "voice-runtime smoke arguments are required")
endif()

file(MAKE_DIRECTORY "${SMOKE_DIRECTORY}")
set(empty_input "${SMOKE_DIRECTORY}/empty-input.bin")
set(shutdown_input "${SMOKE_DIRECTORY}/shutdown-input.bin")
set(eof_output "${SMOKE_DIRECTORY}/eof-output.bin")
set(shutdown_output "${SMOKE_DIRECTORY}/shutdown-output.bin")
set(eof_stderr "${SMOKE_DIRECTORY}/eof-stderr.txt")
set(shutdown_stderr "${SMOKE_DIRECTORY}/shutdown-stderr.txt")
set(pipeline_input "${SMOKE_DIRECTORY}/pipeline-input.bin")
set(pipeline_output "${SMOKE_DIRECTORY}/pipeline-output.bin")
set(pipeline_stderr "${SMOKE_DIRECTORY}/pipeline-stderr.txt")
set(invalid_output "${SMOKE_DIRECTORY}/invalid-output.bin")
set(invalid_stderr "${SMOKE_DIRECTORY}/invalid-stderr.txt")
set(missing_output "${SMOKE_DIRECTORY}/missing-output.bin")
set(missing_stderr "${SMOKE_DIRECTORY}/missing-stderr.txt")
file(WRITE "${empty_input}" "")

execute_process(
  COMMAND "${VOICE_RUNTIME}"
  INPUT_FILE "${empty_input}"
  OUTPUT_FILE "${eof_output}"
  ERROR_FILE "${eof_stderr}"
  RESULT_VARIABLE eof_result
  TIMEOUT 5
)
if(NOT eof_result EQUAL 0)
  message(FATAL_ERROR "voice-runtime EOF smoke failed: ${eof_result}")
endif()
execute_process(
  COMMAND "${SMOKE_HELPER}" verify-eof "${eof_output}"
  RESULT_VARIABLE verify_eof_result
)
if(NOT verify_eof_result EQUAL 0)
  message(FATAL_ERROR "voice-runtime EOF output was not protocol-clean")
endif()
file(SIZE "${eof_stderr}" eof_stderr_size)
if(NOT eof_stderr_size EQUAL 0)
  message(FATAL_ERROR "voice-runtime emitted stderr during clean EOF")
endif()

execute_process(
  COMMAND "${SMOKE_HELPER}" write-shutdown "${shutdown_input}"
  RESULT_VARIABLE write_shutdown_result
)
if(NOT write_shutdown_result EQUAL 0)
  message(FATAL_ERROR "could not prepare shutdown frame")
endif()
execute_process(
  COMMAND "${VOICE_RUNTIME}"
  INPUT_FILE "${shutdown_input}"
  OUTPUT_FILE "${shutdown_output}"
  ERROR_FILE "${shutdown_stderr}"
  RESULT_VARIABLE shutdown_result
  TIMEOUT 5
)
if(NOT shutdown_result EQUAL 0)
  message(FATAL_ERROR "voice-runtime shutdown smoke failed: ${shutdown_result}")
endif()
execute_process(
  COMMAND "${SMOKE_HELPER}" verify-shutdown "${shutdown_output}"
  RESULT_VARIABLE verify_shutdown_result
)
if(NOT verify_shutdown_result EQUAL 0)
  message(FATAL_ERROR "voice-runtime shutdown output was not protocol-clean")
endif()
file(SIZE "${shutdown_stderr}" shutdown_stderr_size)
if(NOT shutdown_stderr_size EQUAL 0)
  message(FATAL_ERROR "voice-runtime emitted stderr during clean shutdown")
endif()

execute_process(
  COMMAND "${SMOKE_HELPER}" write-pipeline-shutdown "${pipeline_input}"
  RESULT_VARIABLE write_pipeline_result
)
if(NOT write_pipeline_result EQUAL 0)
  message(FATAL_ERROR "could not prepare mock pipeline frames")
endif()
execute_process(
  COMMAND "${VOICE_RUNTIME}" --plugin "${MOCK_PLUGIN}" --mock-work-iterations 0
  INPUT_FILE "${pipeline_input}"
  OUTPUT_FILE "${pipeline_output}"
  ERROR_FILE "${pipeline_stderr}"
  RESULT_VARIABLE pipeline_result
  TIMEOUT 5
)
if(NOT pipeline_result EQUAL 0)
  message(FATAL_ERROR "voice-runtime mock pipeline smoke failed: ${pipeline_result}")
endif()
execute_process(
  COMMAND "${SMOKE_HELPER}" verify-pipeline "${pipeline_output}"
  RESULT_VARIABLE verify_pipeline_result
)
if(NOT verify_pipeline_result EQUAL 0)
  message(FATAL_ERROR "voice-runtime mock pipeline output was not the fixed control summary")
endif()
file(SIZE "${pipeline_stderr}" pipeline_stderr_size)
if(NOT pipeline_stderr_size EQUAL 0)
  message(FATAL_ERROR "voice-runtime emitted stderr during clean mock pipeline shutdown")
endif()

execute_process(
  COMMAND "${VOICE_RUNTIME}" --unknown value
  INPUT_FILE "${empty_input}"
  OUTPUT_FILE "${invalid_output}"
  ERROR_FILE "${invalid_stderr}"
  RESULT_VARIABLE invalid_result
  TIMEOUT 5
)
if(NOT invalid_result EQUAL 4)
  message(FATAL_ERROR "voice-runtime malformed CLI exit was not 4: ${invalid_result}")
endif()
file(SIZE "${invalid_output}" invalid_output_size)
file(SIZE "${invalid_stderr}" invalid_stderr_size)
if(NOT invalid_output_size EQUAL 0 OR invalid_stderr_size EQUAL 0)
  message(FATAL_ERROR "voice-runtime malformed CLI contaminated stdout or omitted stderr")
endif()

get_filename_component(plugin_directory "${MOCK_PLUGIN}" DIRECTORY)
set(missing_plugin "${plugin_directory}/missing-aivs-plugin")
execute_process(
  COMMAND "${VOICE_RUNTIME}" --plugin "${missing_plugin}"
  INPUT_FILE "${empty_input}"
  OUTPUT_FILE "${missing_output}"
  ERROR_FILE "${missing_stderr}"
  RESULT_VARIABLE missing_result
  TIMEOUT 5
)
if(NOT missing_result EQUAL 4)
  message(FATAL_ERROR "voice-runtime missing plugin exit was not 4: ${missing_result}")
endif()
file(SIZE "${missing_output}" missing_output_size)
file(SIZE "${missing_stderr}" missing_stderr_size)
if(NOT missing_output_size EQUAL 0 OR missing_stderr_size EQUAL 0)
  message(FATAL_ERROR "voice-runtime setup failure contaminated stdout or omitted stderr")
endif()
