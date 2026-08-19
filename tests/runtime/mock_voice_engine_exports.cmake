if(NOT DEFINED NM OR NOT DEFINED LIBRARY)
  message(FATAL_ERROR "Mock VoiceEngine export inspection arguments are required")
endif()

if(APPLE_PLATFORM)
  execute_process(
    COMMAND "${NM}" -gjU "${LIBRARY}"
    RESULT_VARIABLE inspect_result
    OUTPUT_VARIABLE exports
    ERROR_VARIABLE inspect_error
    OUTPUT_STRIP_TRAILING_WHITESPACE
  )
  set(expected "_aivs_voice_engine_get_api")
else()
  execute_process(
    COMMAND "${NM}" -D --defined-only --format=posix "${LIBRARY}"
    RESULT_VARIABLE inspect_result
    OUTPUT_VARIABLE raw_exports
    ERROR_VARIABLE inspect_error
    OUTPUT_STRIP_TRAILING_WHITESPACE
  )
  string(REGEX REPLACE "[ \t].*" "" exports "${raw_exports}")
  set(expected "aivs_voice_engine_get_api")
endif()

if(NOT inspect_result EQUAL 0)
  message(FATAL_ERROR "Could not inspect Mock VoiceEngine exports: ${inspect_error}")
endif()
if(NOT exports STREQUAL expected)
  message(FATAL_ERROR "Mock VoiceEngine exports must be exactly '${expected}', got '${exports}'")
endif()
