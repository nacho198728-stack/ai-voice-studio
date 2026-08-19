cmake_minimum_required(VERSION 3.25)

set(AIVS_VERSION_FILE "${CMAKE_CURRENT_SOURCE_DIR}/VERSION")
if(NOT EXISTS "${AIVS_VERSION_FILE}")
  message(FATAL_ERROR "Repository version file is missing: ${AIVS_VERSION_FILE}")
endif()

file(READ "${AIVS_VERSION_FILE}" AIVS_PROJECT_VERSION)
string(STRIP "${AIVS_PROJECT_VERSION}" AIVS_PROJECT_VERSION)
if(NOT AIVS_PROJECT_VERSION MATCHES "^[0-9]+\\.[0-9]+\\.[0-9]+$")
  message(FATAL_ERROR
    "VERSION must contain a major.minor.patch version; got '${AIVS_PROJECT_VERSION}'"
  )
endif()

project(
  AIVoiceStudio
  VERSION ${AIVS_PROJECT_VERSION}
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
include(FetchContent)

# Official spdlog v1.16.0 release commit, pinned by immutable commit ID and
# archive integrity. Never fall back to an ambient system package.
set(SPDLOG_BUILD_EXAMPLE OFF CACHE BOOL "" FORCE)
set(SPDLOG_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(SPDLOG_BUILD_BENCH OFF CACHE BOOL "" FORCE)
set(SPDLOG_INSTALL OFF CACHE BOOL "" FORCE)
FetchContent_Declare(
  spdlog
  URL https://github.com/gabime/spdlog/archive/486b55554f11c9cccc913e11a87085b2a91f706f.tar.gz
  URL_HASH SHA256=d2fef585c9879dd239dc498e2e8a1e22982b3ed67b2d14e78622b7ef25bdfdfa
  DOWNLOAD_EXTRACT_TIMESTAMP FALSE
)
FetchContent_MakeAvailable(spdlog)

# These ownership areas deliberately contain no targets yet. Future native
# targets attach here without changing the root build graph.
add_subdirectory(core)
add_subdirectory(runtime)
add_subdirectory(engines)
add_subdirectory(backend)

if(BUILD_TESTING)
  add_subdirectory(tests)
endif()
