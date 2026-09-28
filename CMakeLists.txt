cmake_minimum_required(VERSION 3.16)
project(vpid_viewer LANGUAGES C CXX)

# =====================================================================
# 项目结构（统一 UI = SOUI5，跨平台；平台差异收敛到扫描器 + 设备通知）
#   src/common/  跨平台常量
#   src/core/    跨平台业务核心：设备模型 / Scanner 抽象 / 比对 / 设备通知抽象
#                + 平台实现：windows_scanner / linux_scanner / windows_notifier / linux_notifier
#   src/ui/      ★ 跨平台 SOUI5 UI 层（MainDlg / main / uires XML）——唯一 UI 实现
#   src/platform/legacy/   旧的 GTK / MinGW-Win32 UI（回退保留，非默认）
# =====================================================================
set(VPID_UI_STYLE "SOUI" CACHE STRING "UI backend: SOUI (Windows+Linux) | GTK (Linux legacy) | WIN32 (MinGW legacy)")
message(STATUS "VPID_UI_STYLE=${VPID_UI_STYLE}")

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

# ---- 跨平台业务核心（与 UI 无关，可独立单元测试） ----
set(CORE_SOURCES
  src/core/device_info.cpp
  src/core/device_scanner.cpp
  src/core/device_comparer.cpp
)

# ---- SOUI UI 层源码（跨平台共享） ----
set(SOUI_UI_SOURCES
  src/ui/main.cpp
  src/ui/MainDlg.cpp
)

# ---- 公共 include ----
include_directories(${CMAKE_CURRENT_SOURCE_DIR}/src)

# =====================================================================
# SOUI5 公共配置（Windows / Linux 共用，减少重复与维护漂移）
# =====================================================================
macro(VPID_SET_SOUI_OPTIONS)
  set(SOUI_ENABLE_CORE_LIB ON)   # 核心库编译为静态库
  set(SOUI_ENABLE_COM_LIB ON)    # 组件编译为静态库
  set(SOUI_SHARED_CRT OFF)       # /MT 静态 CRT（XP 目标机无 VCRUNTIME）
  set(SOUI_XP_TOOLSET ON)        # 引入 _USING_V110_SDK71_，抑制新 SDK 头
  set(SOUI_USE_UNICODE ON)
  set(SOUI_WCHAR_AS_DEFAULT ON)  # 保持 wchar_t 为内置类型（否则 /Zc:wchar_t- 破坏 STL）
  set(SOUI_BUILD_RICHEDIT OFF)
  set(SOUI_BUILD_DEMOS OFF)
  set(SOUI_BUILD_GAMES OFF)
  set(SOUI_BUILD_TOOLS OFF)
  set(SOUI_BUILD_HTTPCLIENT OFF)
  set(SOUI_BUILD_WS OFF)
  set(SOUI_ENABLE_SVG OFF)       # 本项目不需要 SVG，减小依赖
  set(SOUI_ENABLE_ACC OFF)
  set(SOUI_ENABLE_SPY OFF)
endmacro()

# 注入 SOUI 公共头文件目录；可附加平台专属目录（如 swinx）
macro(VPID_ADD_SOUI_INCLUDES)
  include_directories(
    ${CMAKE_BINARY_DIR}/config
    ${SOUI_ROOT}/SOUI/include
    ${SOUI_ROOT}/utilities/include
    ${SOUI_ROOT}/components
    ${SOUI_ROOT}/controls.extend
    ${SOUI_ROOT}/third-part/nanosvg
    ${SOUI_ROOT}/third-part/stb
    ${SOUI_ROOT}/third-part/pbl_aupng/include
    ${ARGN}
  )
endmacro()

# 挂载 SOUI5 依赖子树（绕开 skia/lua/sqlite 等无关依赖）
macro(VPID_ADD_SOUI_SUBDIRS)
  set(SOUI_COM_DIR ${SOUI_ROOT}/components)
  set(SOUI_ROOT_DIR ${SOUI_ROOT})
  add_subdirectory(${SOUI_ROOT}/utilities)              # -> utilities4
  add_subdirectory(${SOUI_ROOT}/third-part/pbl_aupng)   # -> aupng
  add_subdirectory(${SOUI_ROOT}/components/render-gdi)        # -> render-gdi
  add_subdirectory(${SOUI_ROOT}/components/imgdecoder-stb)    # -> imgdecoder-stb
  add_subdirectory(${SOUI_ROOT}/SOUI)                        # -> soui4 核心库
endmacro()

if(WIN32 AND VPID_UI_STYLE STREQUAL "SOUI")
  # ===================== Windows: MSVC + SOUI5（静态库, 32 位, XP 兼容） =====================
  enable_language(RC)
  set(SOUI_ROOT ${CMAKE_CURRENT_SOURCE_DIR}/third_party/soui)
  VPID_SET_SOUI_OPTIONS()

  # 全局使用静态运行时（/MT）
  set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")

  # UTF-8 源码（SOUI 与本项目均含中文）
  if(MSVC)
    add_compile_options(/utf-8)
  endif()

  # 生成 SOUI 编译期配置（souistd.h 依赖 <config.h>）
  configure_file(${SOUI_ROOT}/config/config.h.in ${CMAKE_BINARY_DIR}/config/config.h @ONLY)
  configure_file(${SOUI_ROOT}/config/build.cfg.in ${CMAKE_BINARY_DIR}/config/build.cfg @ONLY)

  # SOUI 头文件路径（必须在 add_subdirectory 之前注入）
  VPID_ADD_SOUI_INCLUDES()

  # 复用 SOUI 官方编译器/链接器统一设置
  include(${SOUI_ROOT}/__cmake/internal_utils.cmake)
  config_compiler_and_linker()

  VPID_ADD_SOUI_SUBDIRS()

  add_executable(vpid_viewer WIN32
    ${SOUI_UI_SOURCES}           # src/ui：跨平台 SOUI UI 层
    platform/resources/app.rc
    ${CORE_SOURCES}
    src/core/windows_scanner.cpp
    src/core/windows_notifier.cpp
  )
  target_include_directories(vpid_viewer PRIVATE src platform/resources)
  # RC 编译：相对路径（uires.rc2 / theme_sys_res.rc2）以资源目录与 ui 目录为基
  # OBJECT_DEPENDS：uires XML 变化时强制重编 app.rc（Ninja 不跟踪 RC 的 #include 依赖）
  file(GLOB VPID_UI_RES_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/src/ui/uires.rc2"
    "${CMAKE_CURRENT_SOURCE_DIR}/src/ui/uires/**"
  )
  set_source_files_properties(platform/resources/app.rc PROPERTIES
    LANGUAGE RC
    INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/platform/resources;${CMAKE_CURRENT_SOURCE_DIR}/src/ui"
    OBJECT_DEPENDS "${VPID_UI_RES_DEPENDS}"
  )

  # 系统库：SetupAPI 设备枚举 + SOUI 基础库
  target_link_libraries(vpid_viewer PRIVATE
    soui4 utilities4 render-gdi imgdecoder-stb aupng
    setupapi advapi32
    imm32 msimg32 shlwapi version
    user32 gdi32
  )

  # XP 目标：子系统版本 5.1 + _USING_V110_SDK71_
  target_compile_definitions(vpid_viewer PRIVATE _WIN32_WINNT=0x0501 WINVER=0x0501 _WIN32_IE=0x0501)
  # 组件裁剪：只保留 render-gdi + imgdecoder-stb
  target_compile_definitions(vpid_viewer PRIVATE "SCOM_MASK=(scom_mask_render_gdi|scom_mask_imgdecoder_stb)")
  if(MSVC)
    target_link_options(vpid_viewer PRIVATE /SUBSYSTEM:WINDOWS,5.01)
    # XP 兼容：把 MSVC UCRT 对 Vista+ API（GetNumaHighestNodeNumber / GetNumaProcessorNumber）
    # 的导入引用，重定向到 MainDlg.cpp 中定义的本地 stub
    target_link_options(vpid_viewer PRIVATE
      "/ALTERNATENAME:__imp__GetNumaHighestNodeNumber@4=__imp_MyGetNumaHighestNodeNumber"
      "/ALTERNATENAME:__imp__GetNumaProcessorNumber@4=__imp_MyGetNumaProcessorNumber"
    )
  endif()

elseif(LINUX AND VPID_UI_STYLE STREQUAL "SOUI")
  # ===================== Linux: SOUI5 + swinx（xcb + Cairo） =====================
  # SOUI5 通过 swinx 在 Linux 上复现 Win32 API（官方支持的跨平台路径）。
  # 依赖：cairo / fontconfig / freetype / pixman / xcb / xkbcommon / dbus（由 swinx 提供或系统安装）
  set(SOUI_ROOT ${CMAKE_CURRENT_SOURCE_DIR}/third_party/soui)
  VPID_SET_SOUI_OPTIONS()

  configure_file(${SOUI_ROOT}/config/config.h.in ${CMAKE_BINARY_DIR}/config/config.h @ONLY)
  configure_file(${SOUI_ROOT}/config/build.cfg.in ${CMAKE_BINARY_DIR}/config/build.cfg @ONLY)

  # 头文件路径：公共 SOUI 目录 + swinx（Win32 API 兼容层）
  VPID_ADD_SOUI_INCLUDES(${SOUI_ROOT}/swinx/include)

  include(${SOUI_ROOT}/__cmake/internal_utils.cmake)
  include(${SOUI_ROOT}/__cmake/global.cmake)
  config_compiler_and_linker()

  VPID_ADD_SOUI_SUBDIRS()

  add_executable(vpid_viewer
    ${SOUI_UI_SOURCES}
    ${CORE_SOURCES}
    src/core/linux_scanner.cpp
    src/core/linux_notifier.cpp
  )
  target_include_directories(vpid_viewer PRIVATE src)

  # swinx + SOUI 组件静态库
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(LIBUDEV REQUIRED libudev)
  target_link_libraries(vpid_viewer PRIVATE
    soui4 utilities4 render-gdi imgdecoder-stb aupng
    ${LIBUDEV_LIBRARIES}
    dl pthread
  )
  target_compile_options(vpid_viewer PRIVATE ${LIBUDEV_CFLAGS_OTHER})

elseif(WIN32 AND MINGW AND VPID_UI_STYLE STREQUAL "WIN32")
  # ===================== Windows (MinGW) 回退：原生 Win32 UI =====================
  enable_language(RC)
  add_compile_options(-Os -fno-rtti -fno-exceptions -fno-asynchronous-unwind-tables -ffunction-sections -fdata-sections)
  add_executable(vpid_viewer WIN32
    src/platform/legacy/win32/main.cpp
    platform/resources/app.rc
    ${CORE_SOURCES}
    src/core/windows_scanner.cpp
  )
  target_include_directories(vpid_viewer PRIVATE src platform/resources)
  set_source_files_properties(platform/resources/app.rc PROPERTIES
    LANGUAGE RC
    INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/platform/resources"
  )
  target_link_libraries(vpid_viewer PRIVATE
    setupapi advapi32 comctl32
    user32 gdi32
  )
  target_link_options(vpid_viewer PRIVATE -municode -static -static-libgcc -static-libstdc++ -Wl,-s -Wl,--gc-sections -Wl,--strip-all -Wl,--dynamicbase -Wl,--nxcompat)
  target_compile_definitions(vpid_viewer PRIVATE _WIN32_WINNT=0x0501 WINVER=0x0501 _WIN32_IE=0x0501)

  find_program(UPX_EXECUTABLE upx)
  if(UPX_EXECUTABLE)
    add_custom_command(TARGET vpid_viewer POST_BUILD
      COMMAND ${UPX_EXECUTABLE} --best --force $<TARGET_FILE:vpid_viewer>
      COMMENT "UPX 压缩 $<TARGET_FILE:vpid_viewer>"
    )
  else()
    message(STATUS "UPX 未找到，跳过压缩（安装 UPX 后自动启用）")
  endif()

elseif(UNIX AND NOT APPLE AND VPID_UI_STYLE STREQUAL "GTK")
  # ===================== Linux (GTK) 回退：旧 UI =====================
  add_compile_options(-Os -fno-rtti -fno-exceptions -fno-asynchronous-unwind-tables -ffunction-sections -fdata-sections)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(GTK3 REQUIRED gtk+-3.0)
  pkg_check_modules(LIBUSB REQUIRED libusb-1.0)
  add_executable(vpid_viewer
    src/platform/legacy/gtk/main.cpp
    ${CORE_SOURCES}
    src/core/linux_scanner.cpp
  )
  target_include_directories(vpid_viewer PRIVATE src ${GTK3_INCLUDE_DIRS} ${LIBUSB_INCLUDE_DIRS})
  target_compile_options(vpid_viewer PRIVATE ${GTK3_CFLAGS_OTHER} ${LIBUSB_CFLAGS_OTHER})
  target_link_libraries(vpid_viewer PRIVATE ${GTK3_LIBRARIES} ${LIBUSB_LIBRARIES})

else()
  message(WARNING "当前平台/选项无可用构建路径；请用 -DVPID_UI_STYLE=SOUI（默认）")
endif()