cmake_minimum_required(VERSION 3.16)
project(youai-pipeline-step C CXX)

set(LLAMA_DIR "$ENV{HOME}/.youai/llama.cpp" CACHE PATH "llama.cpp source tree")
set(LLAMA_BUILD "${LLAMA_DIR}/build" CACHE PATH "llama.cpp build tree")

if(NOT EXISTS "${LLAMA_DIR}/include/llama.h")
  message(FATAL_ERROR "llama.cpp not found at ${LLAMA_DIR}. Run ./scripts/setup-llama.sh")
endif()

set(LLAMA_LIB "${LLAMA_BUILD}/bin/libllama.dylib")
set(GGML_LIB "${LLAMA_BUILD}/bin/libggml.dylib")
if(NOT EXISTS "${LLAMA_LIB}")
  set(LLAMA_LIB "${LLAMA_BUILD}/bin/libllama.so")
  set(GGML_LIB "${LLAMA_BUILD}/bin/libggml.so")
endif()
if(NOT EXISTS "${LLAMA_LIB}")
  message(FATAL_ERROR "libllama not found under ${LLAMA_BUILD}/bin — run ./scripts/setup-llama.sh")
endif()

add_executable(youai-pipeline-step youai-pipeline-step.cpp)
target_include_directories(youai-pipeline-step PRIVATE "${LLAMA_DIR}/include" "${LLAMA_DIR}/ggml/include")
target_link_libraries(youai-pipeline-step PRIVATE "${LLAMA_LIB}" "${GGML_LIB}")
set_target_properties(youai-pipeline-step PROPERTIES
  BUILD_RPATH "${LLAMA_BUILD}/bin"
  INSTALL_RPATH "${LLAMA_BUILD}/bin"
)
if(APPLE)
  target_link_libraries(youai-pipeline-step PRIVATE "-framework Accelerate" "-framework Metal" "-framework Foundation")
endif()