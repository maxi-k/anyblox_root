cmake_minimum_required(VERSION 3.6 FATAL_ERROR)
project(rootbench LANGUAGES CXX VERSION 0.1.0)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Locate the ROOT package and define a number of useful targets and variables.
set(ROOT_REQUIRED_COMPONENTS
  RIO Net Hist Tree MathCore Graf Gpad TreePlayer
)
set(ROOT_COMPONENTS_WITH_PREFIX
  ROOT::RIO ROOT::Net ROOT::Hist ROOT::Tree ROOT::MathCore ROOT::Graf ROOT::Gpad ROOT::TreePlayer
)

find_package(ROOT REQUIRED COMPONENTS ${ROOT_REQUIRED_COMPONENTS} CONFIG)

# Create a shared library.
# Passing cmake targets such as `ROOT::RIO` as dependencies (rather than plain
# library names for example via ${ROOT_LIBRARIES}) ensures that properties such as required
# include directories and C++ standard are propagated to our libraries or executables.
# Note: To ensure compatibility with Cling, targets *must* be compiled using the
# same C++ standard as ROOT was compiled with.
add_library(rootbench SHARED lib/rootbench.cpp)
target_link_libraries(rootbench PUBLIC ${ROOT_COMPONENTS_WITH_PREFIX})
target_include_directories(rootbench PUBLIC ${ROOT_INCLUDE_DIRS} ${CMAKE_CURRENT_SOURCE_DIR}/include)

# Create the main program using the library.
add_executable(main main.cpp)
target_link_libraries(main rootbench)

# print root variables
message(STATUS "-- ROOT_VERSION         : ${ROOT_VERSION}")
message(STATUS "-- ROOT_INCLUDE_DIRS    : ${ROOT_INCLUDE_DIRS}")
message(STATUS "-- ROOT_LIBRARIES       : ${ROOT_LIBRARIES}")
message(STATUS "-- ROOT_CXX_FLAGS       : ${ROOT_CXX_FLAGS}")
message(STATUS "-- ROOT_CXX_DEFINITIONS : ${ROOT_CXX_DEFINITIONS}")
message(STATUS "-- ROOT_CXX_STANDARD    : ${ROOT_CXX_STANDARD}")
