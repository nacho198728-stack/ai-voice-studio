if(NOT DEFINED VOICE_RUNTIME OR NOT DEFINED SMOKE_HELPER OR NOT DEFINED SMOKE_DIRECTORY)
  message(FATAL_ERROR "voice-runtime smoke arguments are required")
endif()

file(MAKE_DIRECTORY "${SMOKE_DIRECTORY}")
set(empty_input "${SMOKE_DIRECTORY}/empty-input.bin")
set(shutdown_input "${SMOKE_DIRECTORY}/shutdown-input.bin")
set(eof_output "${SMOKE_DIRECTORY}/eof-output.bin")
set(shutdown_output "${SMOKE_DIRECTORY}/shutdown-output.bin")
set(eof_stderr "${SMOKE_DIRECTORY}/eof-stderr.txt")
set(shutdown_stderr "${SMOKE_DIRECTORY}/shutdown-stderr.txt")
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
