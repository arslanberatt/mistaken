# Spec 05 remediation candidate-expansion adapter build. Links against
# the vendored, pinned, Git-ignored prebuilt libvosk (../../.vendor/vosk,
# see build.md) — Vosk ships no source-buildable release artifact the way
# sherpa-onnx/whisper.cpp do; Alpha Cephei distributes libvosk only as a
# prebuilt binary (PyPI wheel / NuGet / npm), the same way sherpa-onnx's
# own build already depends on a prebuilt ONNX Runtime static library
# fetched from a GitHub release. This library is never modified or
# rebuilt; it is read-only, protected third-party source per
# docs/context/ai-workflow-rules.md, "Protected Files".
cmake_minimum_required(VERSION 3.16)
project(mistaken_bench_vosk_adapter CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

set(VOSK_VENDOR_DIR
    "${CMAKE_CURRENT_SOURCE_DIR}/../../.vendor/vosk"
    CACHE PATH "Pinned, Git-ignored libvosk + vosk_api.h (see build.md)")
if(NOT EXISTS "${VOSK_VENDOR_DIR}/include/vosk_api.h")
  message(FATAL_ERROR
    "vosk_api.h not vendored at ${VOSK_VENDOR_DIR}/include. "
    "Follow benchmarks/adapters/vosk/build.md step 1 first.")
endif()

find_library(VOSK_LIBRARY
  NAMES vosk libvosk
  PATHS "${VOSK_VENDOR_DIR}/lib"
  NO_DEFAULT_PATH
)
if(NOT VOSK_LIBRARY)
  message(FATAL_ERROR
    "libvosk not vendored at ${VOSK_VENDOR_DIR}/lib. "
    "Follow benchmarks/adapters/vosk/build.md step 1 first.")
endif()

add_executable(vosk-adapter src/main.cc src/json_lite.h src/wav_reader.h)
target_include_directories(vosk-adapter PRIVATE "${VOSK_VENDOR_DIR}/include")
target_link_libraries(vosk-adapter PRIVATE "${VOSK_LIBRARY}")
set_target_properties(vosk-adapter PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/build"
)

# libvosk is a dynamic library (Alpha Cephei ships no static build), so the
# adapter binary needs an rpath that finds it next to the vendored copy at
# run time without hardcoding this workstation's absolute path anywhere
# else in the repository. The vendored .dylib's own install name is a
# bare "libvosk.dylib" (Alpha Cephei's own build, not @rpath-qualified),
# so macOS's dynamic linker will not consult LC_RPATH for it unless the
# *adapter binary's* load command is rewritten to reference
# "@rpath/libvosk.dylib" — done here with `install_name_tool -change` as
# a post-build step against this build's own output executable only; the
# vendored libvosk.dylib file itself is never touched.
if(APPLE)
  set_target_properties(vosk-adapter PROPERTIES
    BUILD_RPATH "${VOSK_VENDOR_DIR}/lib"
    INSTALL_RPATH "${VOSK_VENDOR_DIR}/lib"
  )
  add_custom_command(TARGET vosk-adapter POST_BUILD
    COMMAND install_name_tool -change libvosk.dylib @rpath/libvosk.dylib
            $<TARGET_FILE:vosk-adapter>
    COMMENT "Rewriting vosk-adapter's libvosk load command to @rpath (vendored .dylib untouched)"
  )
elseif(UNIX)
  set_target_properties(vosk-adapter PROPERTIES
    BUILD_RPATH "${VOSK_VENDOR_DIR}/lib"
    INSTALL_RPATH "${VOSK_VENDOR_DIR}/lib"
    BUILD_WITH_INSTALL_RPATH TRUE
  )
endif()
