cmake_minimum_required(VERSION 3.20)
project(hermes-bridge LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(HERMES_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../vendor/hermes")
set(HERMES_BUILD_DIR "${HERMES_DIR}/build")

add_library(hermes_bridge STATIC src/bridge.cpp)

target_include_directories(hermes_bridge PUBLIC
  ${CMAKE_CURRENT_SOURCE_DIR}/include
  ${HERMES_DIR}/API
  ${HERMES_DIR}/API/hermes
  ${HERMES_DIR}/API/jsi
  ${HERMES_DIR}/public
  ${HERMES_DIR}/include
  ${HERMES_DIR}/external/llvh/include
  ${HERMES_BUILD_DIR}/external/llvh/include
  ${HERMES_BUILD_DIR}/include
)
