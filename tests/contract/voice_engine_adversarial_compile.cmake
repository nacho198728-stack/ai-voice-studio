foreach(argument IN ITEMS COMPILER LANGUAGE SOURCE OBJECT VOICE_ENGINE_INCLUDE CONTRACTS_INCLUDE)
  if(NOT DEFINED ${argument})
    message(FATAL_ERROR "Missing adversarial compile argument: ${argument}")
  endif()
endforeach()

file(REMOVE "${OBJECT}")
if(MSVC_PLATFORM)
  if(LANGUAGE STREQUAL "C")
    set(standard /std:c17)
  elseif(LANGUAGE STREQUAL "CXX")
    set(standard /std:c++20 /permissive- /Zc:__cplusplus)
  else()
    message(FATAL_ERROR "Unsupported adversarial compile language: ${LANGUAGE}")
  endif()
  execute_process(
    COMMAND
      "${COMPILER}" /nologo /c ${standard} /W4 /WX
      "/I${VOICE_ENGINE_INCLUDE}" "/I${CONTRACTS_INCLUDE}"
      "${SOURCE}" "/Fo${OBJECT}"
    RESULT_VARIABLE compile_result
    OUTPUT_VARIABLE compile_stdout
    ERROR_VARIABLE compile_stderr
  )
else()
  if(LANGUAGE STREQUAL "C")
    set(standard -std=c17)
  elseif(LANGUAGE STREQUAL "CXX")
    set(standard -std=c++20)
  else()
    message(FATAL_ERROR "Unsupported adversarial compile language: ${LANGUAGE}")
  endif()
  execute_process(
    COMMAND
      "${COMPILER}" -c ${standard} -Wall -Wextra -Wpedantic -Werror
      "-I${VOICE_ENGINE_INCLUDE}" "-I${CONTRACTS_INCLUDE}"
      "${SOURCE}" -o "${OBJECT}"
    RESULT_VARIABLE compile_result
    OUTPUT_VARIABLE compile_stdout
    ERROR_VARIABLE compile_stderr
  )
endif()

if(compile_result EQUAL 0)
  file(REMOVE "${OBJECT}")
  message(FATAL_ERROR "VoiceEngine contract accepted unsigned aivs_error_code_t drift")
endif()

set(diagnostics "${compile_stdout}\n${compile_stderr}")
if(NOT diagnostics MATCHES "canonical error code type must be exactly int32_t")
  message(FATAL_ERROR "Adversarial compile failed for an unexpected reason:\n${diagnostics}")
endif()
file(REMOVE "${OBJECT}")
