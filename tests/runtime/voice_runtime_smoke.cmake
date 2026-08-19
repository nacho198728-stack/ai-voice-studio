if(NOT DEFINED VOICE_RUNTIME OR NOT DEFINED MOCK_PLUGIN OR
   NOT DEFINED UNSUPPORTED_PLUGIN OR NOT DEFINED INITIALIZE_FAILURE_PLUGIN OR
   NOT DEFINED SMOKE_HELPER OR NOT DEFINED SMOKE_DIRECTORY)
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
set(log_failure_output "${SMOKE_DIRECTORY}/log-failure-output.bin")
set(log_failure_stderr "${SMOKE_DIRECTORY}/log-failure-stderr.txt")
set(debug_bypass_output "${SMOKE_DIRECTORY}/debug-bypass-output.bin")
set(debug_bypass_stderr "${SMOKE_DIRECTORY}/debug-bypass-stderr.txt")
file(WRITE "${empty_input}" "")
set(log_directory "${SMOKE_DIRECTORY}/logs-日志")
file(MAKE_DIRECTORY "${log_directory}")

function(assert_structured_stderr path label)
  file(READ "${path}" content)
  string(REGEX MATCHALL "[^\r\n]+" records "${content}")
  list(LENGTH records record_count)
  if(record_count EQUAL 0)
    message(FATAL_ERROR "voice-runtime ${label} omitted structured stderr")
  endif()
  foreach(record IN LISTS records)
    if(NOT record MATCHES "^\\{\"timestamp\":\"[^\"]+Z\",\"component\":\"voice-runtime\",\"level\":\"(trace|debug|info|warn|error)\",\"message\":")
      message(FATAL_ERROR "voice-runtime ${label} stderr was not unified JSONL")
    endif()
  endforeach()
endfunction()

get_filename_component(mock_plugin_extension "${MOCK_PLUGIN}" EXT)
set(unicode_plugin_directory "${SMOKE_DIRECTORY}/Unicode-声音-路径")
set(unicode_plugin "${unicode_plugin_directory}/Mock-引擎${mock_plugin_extension}")
file(MAKE_DIRECTORY "${unicode_plugin_directory}")
file(COPY_FILE "${MOCK_PLUGIN}" "${unicode_plugin}" ONLY_IF_DIFFERENT)

execute_process(
  COMMAND "${VOICE_RUNTIME}"
    --log-directory "${log_directory}" --log-level debug --debug-enabled true --generation 1
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
assert_structured_stderr("${eof_stderr}" "clean EOF")

execute_process(
  COMMAND "${SMOKE_HELPER}" write-shutdown "${shutdown_input}"
  RESULT_VARIABLE write_shutdown_result
)
if(NOT write_shutdown_result EQUAL 0)
  message(FATAL_ERROR "could not prepare shutdown frame")
endif()
execute_process(
  COMMAND "${VOICE_RUNTIME}"
    --log-directory "${log_directory}" --log-level debug --debug-enabled true --generation 1
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
assert_structured_stderr("${shutdown_stderr}" "clean shutdown")

execute_process(
  COMMAND "${SMOKE_HELPER}" write-pipeline-shutdown "${pipeline_input}"
  RESULT_VARIABLE write_pipeline_result
)
if(NOT write_pipeline_result EQUAL 0)
  message(FATAL_ERROR "could not prepare mock pipeline frames")
endif()
execute_process(
  COMMAND "${VOICE_RUNTIME}" --plugin "${unicode_plugin}" --mock-work-iterations 0
    --log-directory "${log_directory}" --log-level debug --debug-enabled true --generation 1
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
assert_structured_stderr("${pipeline_stderr}" "mock pipeline shutdown")

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

set(debug_bypass_directory "${SMOKE_DIRECTORY}/debug-bypass-logs")
execute_process(
  COMMAND "${VOICE_RUNTIME}"
    --log-directory "${debug_bypass_directory}" --log-level debug
    --debug-enabled false --generation 77
  INPUT_FILE "${empty_input}"
  OUTPUT_FILE "${debug_bypass_output}"
  ERROR_FILE "${debug_bypass_stderr}"
  RESULT_VARIABLE debug_bypass_result
  TIMEOUT 5
)
if(NOT debug_bypass_result EQUAL 4)
  message(FATAL_ERROR "voice-runtime accepted debug logging without debug authority")
endif()
file(SIZE "${debug_bypass_output}" debug_bypass_output_size)
file(SIZE "${debug_bypass_stderr}" debug_bypass_stderr_size)
if(NOT debug_bypass_output_size EQUAL 0 OR debug_bypass_stderr_size EQUAL 0 OR
   EXISTS "${debug_bypass_directory}")
  message(FATAL_ERROR "voice-runtime debug bypass changed stdout or initialized logging")
endif()

get_filename_component(plugin_directory "${MOCK_PLUGIN}" DIRECTORY)
set(missing_plugin "${plugin_directory}/missing-aivs-plugin")
execute_process(
  COMMAND "${VOICE_RUNTIME}" --plugin "${missing_plugin}"
    --log-directory "${log_directory}" --log-level info --debug-enabled false --generation 1
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
assert_structured_stderr("${missing_stderr}" "missing plugin")

set(blocked_log_directory "${SMOKE_DIRECTORY}/blocked-log-directory")
file(WRITE "${blocked_log_directory}" "not a directory")
execute_process(
  COMMAND "${VOICE_RUNTIME}"
    --log-directory "${blocked_log_directory}" --log-level info --debug-enabled false --generation 1
  INPUT_FILE "${empty_input}"
  OUTPUT_FILE "${log_failure_output}"
  ERROR_FILE "${log_failure_stderr}"
  RESULT_VARIABLE log_failure_result
  TIMEOUT 5
)
if(NOT log_failure_result EQUAL 4)
  message(FATAL_ERROR "voice-runtime log initialization failure exit was not 4")
endif()
file(SIZE "${log_failure_output}" log_failure_output_size)
file(SIZE "${log_failure_stderr}" log_failure_stderr_size)
if(NOT log_failure_output_size EQUAL 0 OR log_failure_stderr_size EQUAL 0)
  message(FATAL_ERROR "voice-runtime log initialization failure corrupted stdout or lacked diagnostics")
endif()

function(assert_plugin_setup_failure name plugin)
  set(output "${SMOKE_DIRECTORY}/${name}-output.bin")
  set(stderr "${SMOKE_DIRECTORY}/${name}-stderr.txt")
  execute_process(
    COMMAND "${VOICE_RUNTIME}" --plugin "${plugin}"
      --log-directory "${log_directory}" --log-level info --debug-enabled false --generation 1
    INPUT_FILE "${empty_input}"
    OUTPUT_FILE "${output}"
    ERROR_FILE "${stderr}"
    RESULT_VARIABLE result
    TIMEOUT 5
  )
  if(NOT result EQUAL 4)
    message(FATAL_ERROR "voice-runtime ${name} setup exit was not 4: ${result}")
  endif()
  file(SIZE "${output}" output_size)
  file(SIZE "${stderr}" stderr_size)
  if(NOT output_size EQUAL 0 OR stderr_size EQUAL 0)
    message(FATAL_ERROR "voice-runtime ${name} setup failure violated stdout/stderr boundary")
  endif()
  assert_structured_stderr("${stderr}" "${name}")
endfunction()

assert_plugin_setup_failure("unsupported-factory" "${UNSUPPORTED_PLUGIN}")
assert_plugin_setup_failure("initialize-failure" "${INITIALIZE_FAILURE_PLUGIN}")

if(NOT EXISTS "${log_directory}/voice-runtime.jsonl")
  message(FATAL_ERROR "voice-runtime did not write the configured JSONL log file")
endif()
