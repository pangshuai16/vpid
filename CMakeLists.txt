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
    # 体积优化（纯编译期、非打包、无 packer 特征 → 360 风险为 0）：
    #   /Os    体积优先（替代 CMake 默认的 /O2）
    #   /Gy    函数级链接（配合 /OPT:REF 删掉未引用函数）
    #   /GR-   关 RTTI（实测 SOUI 与本项目均无 dynamic_cast/typeid）
    #   /EHsc- 关 C++ 异常（实测全库仅 souidlgs.h 有一个未调用的 ATLTRY 宏，无 throw）
    # 注意：每个 flag 必须单独一个 genex。塞在一个 genex 里会被
    # add_compile_options 按空格拆坏（/EHsc- 的尾部 - 会被切成 /EH>，
    # /Os 会被当成源文件路径 $<1:/Os）。
    add_compile_options("$<$<CONFIG:Release>:/Os>")
    add_compile_options("$<$<CONFIG:Release>:/Gy>")
    add_compile_options("$<$<CONFIG:Release>:/GR->")
    add_compile_options("$<$<CONFIG:Release>:/EHsc->")
    # /GL 程序级优化：让编译器生成 COMDAT 节，配合 /OPT:REF 做跨 TU 去死码
    # 注意：这只是 COMDAT 布局优化，不是 /LTCG 全链接优化（后者会做代码生成，风险更大）
    add_compile_options("$<$<CONFIG:Release>:/GL>")
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
    # /OPT:REF 死代码消除（配合 /Gy 函数级链接）：未引用的 SOUI 控件/函数整个删掉
    target_link_options(vpid_viewer PRIVATE
      "/SUBSYSTEM:WINDOWS,5.01"
      "$<$<CONFIG:Release>:/OPT:REF>"
    )
    # 高分屏清晰度（避免 Windows 把整窗位图拉伸放虚）：
    # 用 /MANIFESTINPUT 把我们的 DPI 感知清单"合并"进链接器自动生成的主清单，
    # 主清单里含 VC 运行库 side-by-side 依赖（Microsoft.VC*.CRT），必须保留，
    # 否则缺 CRT 依赖会 14001 启动失败。
    # 注意：不能在 app.rc 里用 RT_MANIFEST 内嵌（RC 未 include winuser.h 会把
    # RT_MANIFEST 当字符串类型名，加载器不识别）；也不可 /MANIFEST:NO 关掉主清单。
    target_link_options(vpid_viewer PRIVATE
      "/MANIFESTINPUT:${CMAKE_CURRENT_SOURCE_DIR}/platform/resources/app.manifest"
      "/MANIFEST:EMBED"
    )
    # XP 兼容：把 MSVC UCRT 对 Vista+ API（GetNumaHighestNodeNumber / GetNumaProcessorNumber）
    # 的导入引用，重定向到 MainDlg.cpp 中定义的本地 stub
    target_link_options(vpid_viewer PRIVATE
      "/ALTERNATENAME:__imp__GetNumaHighestNodeNumber@4=__imp_MyGetNumaHighestNodeNumber"
      "/ALTERNATENAME:__imp__GetNumaProcessorNumber@4=__imp_MyGetNumaProcessorNumber"
    )
  endif()

elseif(UNIX AND NOT APPLE AND VPID_UI_STYLE STREQUAL "SOUI")
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

  # ★ Linux 必需：swinx 跨平台兼容层（模拟 Win32 API，并提供 cairo/xkbcommon 等内部库）
  #   必须在 SOUI 之前 add，否则 SOUI/CMakeLists 的 add_dependencies(soui4 swinx) 找不到 target。
  #   swinx 由 SOUI 顶层 CMakeLists 本应自动 add_subdirectory(swinx)，但本项目为精简依赖
  #   手动挂载 SOUI 子目录，故此处显式补齐。
  add_subdirectory(${SOUI_ROOT}/swinx)

  VPID_ADD_SOUI_SUBDIRS()

  # zig 交叉 / Clang 下禁用 PCH 后，soui4 依赖 PCH 隐式注入 souistd.h
  # （源文件直接使用 SNSBEGIN/SStringA 等，无显式 include）。
  # 用编译器 -include 等效替换（仅 soui4 target，不影响第三方 C 库）。
  if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    set(SOUI_STD_HEADER ${SOUI_ROOT}/SOUI/include/souistd.h)
    if(EXISTS ${SOUI_STD_HEADER})
      target_compile_options(soui4 PRIVATE "-include;${SOUI_STD_HEADER}")
    endif()
  endif()

  add_executable(vpid_viewer
    ${SOUI_UI_SOURCES}
    ${CORE_SOURCES}
    src/core/linux_scanner.cpp
    src/core/linux_notifier.cpp
  )
  target_include_directories(vpid_viewer PRIVATE src)
  # 组件裁剪（与 Windows 分支一致）：只启用 render-gdi + imgdecoder-stb，
  # 否则 com-cfg.h 的 SCreator 会引用 RENDER_SKIA/RESPROVIDER_7ZIP/ZIP/
  # TRANSLATOR/LOG4Z 等未链接组件 → 链接期 undefined reference
  target_compile_definitions(vpid_viewer PRIVATE "SCOM_MASK=(scom_mask_render_gdi|scom_mask_imgdecoder_stb)")

  # swinx + SOUI 组件静态库 + Linux 原生扫描(libusb vendored)：
  #   * libudev：linux_notifier.cpp 用 dlopen 动态加载（缺失降级轮询），无链接期依赖，不写死。
  #   * libusb-1.0：linux_scanner.cpp 真要用静态链接，vendored 到 third_party/libusb 从源码编，
  #                 规避交叉编译时 pkg-config 误用 host 系统 libusb（glibc 版本不匹配）。
  add_subdirectory(${CMAKE_CURRENT_SOURCE_DIR}/third_party/libusb)
  target_link_libraries(vpid_viewer PRIVATE
    soui4 utilities4 render-gdi imgdecoder-stb aupng
    libusb
    brotlicommon   # freetype→brotlidec 的 PRIVATE 依赖，静态链接不传递，需显式补
    dl pthread
  )

  # zig 交叉编译：系统库 libxcb/libXau/libuuid 不能 -L 解析（Windows 下 zig
  # 对 -L盘符路径 解析错误），直接传 sysroot 里的 .so 绝对路径。
  #     --allow-shlib-undefined：sysroot 的 .so 取自 Ubuntu 22.04，内部引用
  #     getrandom@GLIBC_2.25 等新符号，本机链接时目标 glibc 是 2.17 floor；
  #     运行时目标机用自己的系统库解析，链接期放行即可（不污染产物 glibc 版本）。
  if(DEFINED LINUX_SYSROOT_LIB AND EXISTS ${LINUX_SYSROOT_LIB}/libxcb.so)
    target_link_libraries(vpid_viewer PRIVATE
      ${LINUX_SYSROOT_LIB}/libxcb.so
      ${LINUX_SYSROOT_LIB}/libuuid.so
      ${LINUX_SYSROOT_LIB}/libXau.so
    )
    target_link_options(vpid_viewer PRIVATE -Wl,--allow-shlib-undefined)
  endif()

  # Release 体积：链接期 strip（zig cc -s 等价于 --strip-all，避免依赖
  # zig objcopy 工具在该机解压失败的问题；gcc/g++ 也支持 -s）
  if(CMAKE_BUILD_TYPE STREQUAL "Release")
    target_link_options(vpid_viewer PRIVATE -s)
  endif()

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

# ============================================================
# 可复现构建：固定 PE TimeDateStamp（同源码同参数 → 确定性哈希）
#   MSVC link 默认把构建时刻写进 COFF TimeDateStamp（pe+8）；本项目里该 4 字节
#   还被复制到 .data 一处，故每次构建有 2 处字节随时间变化 → 尺寸恒等但哈希不同。
#   钉成定值后即可复现。
# ============================================================
if(TARGET vpid_viewer)
  find_program(VPID_PYTHON_EXECUTABLE NAMES python3 python
    HINTS
      "C:/Users/Administrator/AppData/Local/Programs/Python/Python312-32"
      ${CMAKE_CURRENT_SOURCE_DIR}/tools
    PATH_SUFFIXES .
  )
  if(VPID_PYTHON_EXECUTABLE)
    add_custom_command(TARGET vpid_viewer POST_BUILD
      COMMAND ${VPID_PYTHON_EXECUTABLE} ${CMAKE_CURRENT_SOURCE_DIR}/tools/pin_timestamp.py
              $<TARGET_FILE:vpid_viewer>
      COMMENT "固定 PE 时间戳（可复现哈希）$<TARGET_FILE:vpid_viewer>"
    )
  else()
    message(STATUS "未找到 python，跳过时间戳固定（构建哈希将不可复现）")
  endif()
endif()