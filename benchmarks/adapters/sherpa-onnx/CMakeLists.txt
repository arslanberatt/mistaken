# Spec 05 sherpa-onnx adapter build. Pulls in the vendored, pinned
# sherpa-onnx source tree (../../.vendor/sherpa-onnx, Git-ignored, never
# committed and never modified — see build.md) the same way sherpa-onnx's
# own c-api-examples/ do, then links this adapter against sherpa-onnx-c-api.
cmake_minimum_required(VERSION 3.16)
project(mistaken_bench_sherpa_onnx_adapter CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

set(SHERPA_ONNX_ENABLE_PYTHON OFF CACHE BOOL "" FORCE)
set(SHERPA_ONNX_ENABLE_TESTS OFF CACHE BOOL "" FORCE)
set(SHERPA_ONNX_ENABLE_PORTAUDIO OFF CACHE BOOL "" FORCE)
set(SHERPA_ONNX_ENABLE_JNI OFF CACHE BOOL "" FORCE)
set(SHERPA_ONNX_ENABLE_C_API ON CACHE BOOL "" FORCE)
set(SHERPA_ONNX_ENABLE_WEBSOCKET OFF CACHE BOOL "" FORCE)
set(SHERPA_ONNX_ENABLE_GPU OFF CACHE BOOL "" FORCE)
set(SHERPA_ONNX_ENABLE_TTS OFF CACHE BOOL "" FORCE)
set(SHERPA_ONNX_ENABLE_SPEAKER_DIARIZATION OFF CACHE BOOL "" FORCE)
set(SHERPA_ONNX_ENABLE_BINARY OFF CACHE BOOL "" FORCE)
set(SHERPA_ONNX_BUILD_C_API_EXAMPLES OFF CACHE BOOL "" FORCE)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)

set(SHERPA_ONNX_VENDOR_DIR
    "${CMAKE_CURRENT_SOURCE_DIR}/../../.vendor/sherpa-onnx"
    CACHE PATH "Pinned, Git-ignored sherpa-onnx checkout (see build.md)")
if(NOT EXISTS "${SHERPA_ONNX_VENDOR_DIR}/CMakeLists.txt")
  message(FATAL_ERROR
    "sherpa-onnx not vendored at ${SHERPA_ONNX_VENDOR_DIR}. "
    "Follow benchmarks/adapters/sherpa-onnx/build.md step 1 first.")
endif()
add_subdirectory(${SHERPA_ONNX_VENDOR_DIR} sherpa-onnx-vendor-build)

add_executable(sherpa-onnx-adapter src/main.cc src/json_lite.h)
target_link_libraries(sherpa-onnx-adapter PRIVATE sherpa-onnx-c-api)
set_target_properties(sherpa-onnx-adapter PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/build"
)
