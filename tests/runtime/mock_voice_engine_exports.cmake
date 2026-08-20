if(DEFINED OUTPUT_FIXTURE)
  file(READ "${OUTPUT_FIXTURE}" raw_exports)
  set(inspect_result 0)
elseif(MSVC_PLATFORM)
  if(NOT DEFINED INSPECTOR OR NOT DEFINED LIBRARY)
    message(FATAL_ERROR "MSVC Mock VoiceEngine export inspection arguments are required")
  endif()
  execute_process(
    COMMAND "${INSPECTOR}" /NOLOGO /EXPORTS "${LIBRARY}"
    RESULT_VARIABLE inspect_result
    OUTPUT_VARIABLE raw_exports
    ERROR_VARIABLE inspect_error
  )
elseif(APPLE_PLATFORM)
  if(NOT DEFINED NM OR NOT DEFINED LIBRARY)
    message(FATAL_ERROR "Mock VoiceEngine export inspection arguments are required")
  endif()
  execute_process(
    COMMAND "${NM}" -gjU "${LIBRARY}"
    RESULT_VARIABLE inspect_result
    OUTPUT_VARIABLE exports
    ERROR_VARIABLE inspect_error
    OUTPUT_STRIP_TRAILING_WHITESPACE
  )
  set(expected "_aivs_voice_engine_get_api")
else()
  if(NOT DEFINED NM OR NOT DEFINED LIBRARY)
    message(FATAL_ERROR "Mock VoiceEngine export inspection arguments are required")
  endif()
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
  message(FATAL_ERROR
    "Could not inspect Mock VoiceEngine exports (result=${inspect_result}, "
    "inspector='${INSPECTOR}', library='${LIBRARY}'). "
    "stdout='${raw_exports}' stderr='${inspect_error}'"
  )
endif()
if(MSVC_PLATFORM)
  string(REPLACE "\r\n" "\n" raw_exports "${raw_exports}")
  string(REPLACE "\n" ";" export_lines "${raw_exports}")
  set(exports_list)
  foreach(line IN LISTS export_lines)
    if(line MATCHES "^[ \t]*[0-9]+[ \t]+[0-9A-Fa-f]+[ \t]+[0-9A-Fa-f]+[ \t]+([^ \t=]+)")
      list(APPEND exports_list "${CMAKE_MATCH_1}")
    endif()
  endforeach()
  list(JOIN exports_list ";" exports)
  set(expected "aivs_voice_engine_get_api")
endif()
if(NOT exports STREQUAL expected)
  message(FATAL_ERROR "Mock VoiceEngine exports must be exactly '${expected}', got '${exports}'")
endif()
