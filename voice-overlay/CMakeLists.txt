cmake_minimum_required(VERSION 3.16)
project(archtype-voice-overlay C)

find_package(PkgConfig REQUIRED)
pkg_check_modules(GTK4 REQUIRED IMPORTED_TARGET gtk4)
pkg_check_modules(LAYER REQUIRED IMPORTED_TARGET gtk4-layer-shell-0)

add_executable(archtype-voice-overlay archtype-voice-overlay.c)
target_link_libraries(archtype-voice-overlay PRIVATE PkgConfig::GTK4 PkgConfig::LAYER m)
target_compile_options(archtype-voice-overlay PRIVATE -Wall -Wextra)

install(TARGETS archtype-voice-overlay RUNTIME DESTINATION bin)
