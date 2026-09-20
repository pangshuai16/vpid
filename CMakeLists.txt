cmake_minimum_required(VERSION 3.16)
project(vpid_viewer LANGUAGES CXX)

# 两套系统各自使用原生 UI，体积最小、兼容性最好：
#   - Windows  : 原生 Win32 API + Common Controls（无任何第三方 UI 框架）
#               MinGW-w64 (i686) 静态链接，产出 32 位自包含 exe，兼容 Windows XP
#   - Linux    : 原生 GTK3（glibc 2.28 兼容）
#   - core/    : 跨平台复用的 USB 枚举 / 比对逻辑（无 UI 框架依赖）
# 编译依赖：Linux 需 gtk3-devel + libusb1-devel（Win32 端仅系统库，无额外依赖）
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

add_compile_options(-Os -fno-rtti -fno-exceptions -fno-asynchronous-unwind-tables -ffunction-sections -fdata-sections)

# 跨平台复用层：设备信息/扫描抽象/比对
set(CORE_SOURCES
  src/core/device_info.cpp
  src/core/device_scanner.cpp
  src/core/device_comparer.cpp
)

# 平台原生 UI 入口 + 平台扫描器
if(WIN32)
  # RC 仅 Windows 需要；放在分支内启用，避免 Linux 阶段要求 CMAKE_RC_COMPILER
  enable_language(RC)
  add_executable(vpid_viewer WIN32
    platform/win32/main.cpp
    platform/resources/app.rc
    ${CORE_SOURCES}
    src/core/windows_scanner.cpp
  )
  target_include_directories(vpid_viewer PRIVATE src platform/resources)
  set_source_files_properties(platform/resources/app.rc PROPERTIES
    LANGUAGE RC
    INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/platform/resources"
  )
  # 仅系统库（MinGW 自带），无需 wx/libusb；-municode 让 CRT 以 wWinMain 为入口
  target_link_libraries(vpid_viewer PRIVATE
    setupapi   # SetupAPI 设备枚举
    advapi32   # 注册表兜底扫描
    comctl32   # Common Controls（ListView）
    user32 gdi32
  )
  # 完全静态链接（含 MinGW 运行库 libwinpthread/libgcc_s/libstdc++），
  # 产出零 DLL 依赖、可独立运行在裸机 XP 的 exe
  target_link_options(vpid_viewer PRIVATE -municode -static -static-libgcc -static-libstdc++ -Wl,-s -Wl,--gc-sections -Wl,--strip-all -Wl,--dynamicbase -Wl,--nxcompat)
  # XP 兼容：锁定 NT 5.1 API 集 + Win 层 API 集
  target_compile_definitions(vpid_viewer PRIVATE _WIN32_WINNT=0x0501 WINVER=0x0501 _WIN32_IE=0x0501)
else()
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(GTK3 REQUIRED gtk+-3.0)
  pkg_check_modules(LIBUSB REQUIRED libusb-1.0)
  add_executable(vpid_viewer
    platform/gtk/main.cpp
    ${CORE_SOURCES}
    src/core/linux_scanner.cpp
  )
  target_include_directories(vpid_viewer PRIVATE src platform/resources ${GTK3_INCLUDE_DIRS} ${LIBUSB_INCLUDE_DIRS})
  target_compile_options(vpid_viewer PRIVATE ${GTK3_CFLAGS_OTHER} ${LIBUSB_CFLAGS_OTHER})
  target_link_libraries(vpid_viewer PRIVATE ${GTK3_LIBRARIES} ${LIBUSB_LIBRARIES})
endif()