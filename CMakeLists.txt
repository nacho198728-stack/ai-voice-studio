cmake_minimum_required(VERSION 3.25)

project(
  AIVoiceStudio
  VERSION 0.5.0
  LANGUAGES C CXX
)

# The future VoiceEngine plugin boundary remains consumable from pure C, while
# the isolated runtime and its implementations use C++20.
set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Keep artifacts independent of generator layout. $<CONFIG> evaluates to the
# active configuration for multi-config generators and remains predictable for
# the Ninja presets used in local and CI builds.
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin/$<CONFIG>")
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib/$<CONFIG>")
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib/$<CONFIG>")

include(CTest)

# These ownership areas deliberately contain no targets yet. Future native
# targets attach here without changing the root build graph.
add_subdirectory(core)
add_subdirectory(runtime)
add_subdirectory(engines)
add_subdirectory(backend)

if(BUILD_TESTING)
  add_subdirectory(tests)
endif()
