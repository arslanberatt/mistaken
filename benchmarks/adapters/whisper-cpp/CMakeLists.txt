# Spec 05 whisper.cpp adapter build. Pulls in the vendored, pinned
# whisper.cpp source tree (../../.vendor/whisper-cpp, Git-ignored, never
# committed and never modified) via add_subdirectory, then links this
# adapter against the `whisper` library target it exports.
cmake_minimum_required(VERSION 3.16)
project(mistaken_bench_whisper_cpp_adapter CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

set(WHISPER_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(WHISPER_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(WHISPER_BUILD_SERVER OFF CACHE BOOL "" FORCE)
set(WHISPER_CURL OFF CACHE BOOL "" FORCE)
set(WHISPER_SDL2 OFF CACHE BOOL "" FORCE)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
# Spec 05 measures the CPU execution path Mistaken would actually ship
# (section 4, "GPU/NPU acceleration claims"). Metal is also expensive to
# initialize (one-time embedded shader compilation per process, ~15-20s on
# this host) which is intolerable when a fresh adapter process is spawned
# per clip per repetition; BLAS/Accelerate stays enabled as an in-runtime
# CPU-path default.
set(GGML_METAL OFF CACHE BOOL "" FORCE)

set(WHISPER_CPP_VENDOR_DIR
    "${CMAKE_CURRENT_SOURCE_DIR}/../../.vendor/whisper-cpp"
    CACHE PATH "Pinned, Git-ignored whisper.cpp checkout (see build.md)")
if(NOT EXISTS "${WHISPER_CPP_VENDOR_DIR}/CMakeLists.txt")
  message(FATAL_ERROR
    "whisper.cpp not vendored at ${WHISPER_CPP_VENDOR_DIR}. "
    "Follow benchmarks/adapters/whisper-cpp/build.md step 1 first.")
endif()
add_subdirectory(${WHISPER_CPP_VENDOR_DIR} whisper-cpp-vendor-build)

add_executable(whisper-cpp-adapter src/main.cc src/json_lite.h src/wav_reader.h)
target_link_libraries(whisper-cpp-adapter PRIVATE whisper)
target_include_directories(whisper-cpp-adapter PRIVATE
  ${WHISPER_CPP_VENDOR_DIR}/include
  ${WHISPER_CPP_VENDOR_DIR}/ggml/include
)
set_target_properties(whisper-cpp-adapter PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/build"
)
