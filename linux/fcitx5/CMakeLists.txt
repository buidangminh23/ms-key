cmake_minimum_required(VERSION 3.12)
project(mskey-fcitx5 VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

include(GNUInstallDirs)
find_package(Fcitx5Core REQUIRED)
find_package(Threads REQUIRED)

get_filename_component(MSKEY_REPO_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
set(MSKEY_FFI_INCLUDE "${MSKEY_REPO_ROOT}/crates/mskey-ffi/include")

set(MSKEY_FFI_LIB "" CACHE FILEPATH "Path to mskey_ffi library")
if(NOT MSKEY_FFI_LIB)
    find_library(MSKEY_FFI_FOUND
        NAMES mskey_ffi libmskey_ffi.a
        PATHS
            "${MSKEY_REPO_ROOT}/target/release"
            "${MSKEY_REPO_ROOT}/target/debug"
            "${CMAKE_INSTALL_PREFIX}/lib"
            /usr/local/lib
            /usr/lib
    )
    if(MSKEY_FFI_FOUND)
        set(MSKEY_FFI_LIB "${MSKEY_FFI_FOUND}")
    endif()
endif()
if(NOT MSKEY_FFI_LIB)
    message(FATAL_ERROR "mskey_ffi library not found; build mskey-ffi or pass -DMSKEY_FFI_LIB=<path>")
endif()

set(MSKEY_ADDON_DIR "${CMAKE_INSTALL_LIBDIR}/fcitx5")
if(DEFINED FCITX_INSTALL_ADDONDIR)
    set(MSKEY_ADDON_DIR "${FCITX_INSTALL_ADDONDIR}")
endif()

set(MSKEY_DATA_DIR "${CMAKE_INSTALL_DATADIR}/fcitx5")
if(DEFINED FCITX_INSTALL_PKGDATADIR)
    set(MSKEY_DATA_DIR "${FCITX_INSTALL_PKGDATADIR}")
endif()

add_library(mskey MODULE
    src/mskey_fcitx5.cpp
)
set_target_properties(mskey PROPERTIES
    PREFIX ""
    OUTPUT_NAME "mskey"
)
target_include_directories(mskey PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/src"
    "${MSKEY_FFI_INCLUDE}"
)
target_link_libraries(mskey PRIVATE
    Fcitx5::Core
    "${MSKEY_FFI_LIB}"
    Threads::Threads
    ${CMAKE_DL_LIBS}
    m
)

configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/src/mskey-addon.conf.in"
    "${CMAKE_CURRENT_BINARY_DIR}/mskey-addon.conf"
    @ONLY
)
configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/src/mskey.conf.in"
    "${CMAKE_CURRENT_BINARY_DIR}/mskey.conf"
    @ONLY
)

install(TARGETS mskey DESTINATION "${MSKEY_ADDON_DIR}")
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/mskey-addon.conf"
    DESTINATION "${MSKEY_DATA_DIR}/addon"
    RENAME "mskey.conf"
)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/mskey.conf"
    DESTINATION "${MSKEY_DATA_DIR}/inputmethod"
)
