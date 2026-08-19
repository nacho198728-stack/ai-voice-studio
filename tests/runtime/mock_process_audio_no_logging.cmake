if(NOT DEFINED SOURCE)
  message(FATAL_ERROR "Mock source path is required")
endif()

file(READ "${SOURCE}" source)
string(FIND "${source}" "mock_process_audio(" begin)
string(FIND "${source}" "mock_reset(" end)
if(begin EQUAL -1 OR end EQUAL -1 OR end LESS_EQUAL begin)
  message(FATAL_ERROR "Could not isolate Mock process_audio hot path")
endif()
math(EXPR length "${end} - ${begin}")
string(SUBSTRING "${source}" ${begin} ${length} process_audio)

string(SHA256 process_audio_sha256 "${process_audio}")
set(expected_sha256 "99d1886e51e45aca443dd4a8fde47c56c6d8df3b312e63c39d305eaac770c844")
if(NOT process_audio_sha256 STREQUAL expected_sha256)
  message(FATAL_ERROR
    "Mock process_audio changed from the reviewed logging-free hot path; "
    "expected ${expected_sha256}, got ${process_audio_sha256}"
  )
endif()

foreach(required IN ITEMS
    "process_call_count.fetch_add(1U, std::memory_order_relaxed)"
    "input_frame_count.fetch_add(request->input.frame_count, std::memory_order_relaxed)"
    "output_frame_count.fetch_add(request->input.frame_count, std::memory_order_relaxed)"
)
  string(FIND "${process_audio}" "${required}" found)
  if(found EQUAL -1)
    message(FATAL_ERROR "Mock process_audio lost atomic-only metric update: ${required}")
  endif()
endforeach()
