# =====================================================================
# libusb-1.0.27 vendored 静态库（仅 Linux 后端）
# ---------------------------------------------------------------------
# 来源：libusb/libusb v1.0.27 官方 tarball（autotools 分支，无 CMake）
# 裁剪：仅保留 Linux 后端 linux_usbfs.c + linux_netlink.c（sysfs/ioctl/netlink，
#        无 udev 依赖）；去掉 linux_udev.c（可选监控后端，本项目不需要）、
#        Windows/Android/其他平台后端与示例代码。
# 用途：vpid_viewer linux_scanner.cpp 需要 libusb-1.0 静态链接；
#       支持 zig 交叉编译（libusb 纯 C，可用 zig cc 从源码编译）。
#
# 兼容性说明（zig 交叉 + glibc floor 2.17）：
#   - linux_usbfs.c/linux_netlink.c 用到的符号（open/read/write/ioctl/poll/
#     sysioctl/mmap/realpath/opendir/readdir/dirent/strtol 等）全部是 glibc 2.17
#     时代就存在的，无版本提升风险。
# =====================================================================
cmake_minimum_required(VERSION 3.16)
project(libusb_vendor C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# 静态库（本项目所有依赖 libusb 的地方都静态链接）
set(BUILD_SHARED_LIBS OFF)

set(LIBUSB_SOURCES
  src/core.c
  src/descriptor.c
  src/hotplug.c
  src/io.c
  src/strerror.c
  src/sync.c
  src/os/linux_usbfs.c
  src/os/linux_netlink.c
  src/os/events_posix.c
  src/os/threads_posix.c
)

# 平台裁剪：仅 Linux（本项目只在 Linux 使用 libusb）
if(NOT CMAKE_SYSTEM_NAME MATCHES "Linux")
  message(WARNING "libusb vendor 仅支持 Linux，跳过构建")
  return()
endif()

# 生成 libusb 需要的 config.h（libusbi.h:23 强制 #include <config.h>）。
# 官方 autotools 由 configure 生成；我们 vendored 裁剪后手写精简版，
# 只定义本项目 Linux 构建需要的宏：
#   PLATFORM_POSIX           选择 events_posix.h / threads_posix.h 后端
#   HAVE_SYS_TIME_H          时间函数
#   HAVE_UNISTD_H / HAVE_STRING_H / HAVE_ERRNO_H / HAVE_SYS_TYPES_H /
#   HAVE_SYS_STAT_H / HAVE_FCNTL_H / HAVE_POLL_H / HAVE_SYS_IOCTL_H /
#   HAVE_SYS_SOCKET_H / HAVE_SYS_UIO_H / HAVE_INTTYPES_H / HAVE_STDINT_H
#   HAVE_CLOCK_GETTIME       linux_usbfs/io 计时
#   HAVE_EVENTFD / HAVE_TIMERFD  events_posix（glibc 2.8+/2.8+ 存在，2.17 floor OK）
#   NDEBUG                   断言关闭（已通过 compile def 注入，这里冗余无害）
set(LIBUSB_CONFIG_CONTENT
"#pragma once
#define _GNU_SOURCE 1
#define PLATFORM_POSIX 1
#define PRINTF_FORMAT(a, b) __attribute__((__format__(__printf__, a, b)))
#define DEFAULT_VISIBILITY __attribute__((visibility(\"default\")))
#define HAVE_SYS_TIME_H 1
#define HAVE_UNISTD_H 1
#define HAVE_STRING_H 1
#define HAVE_ERRNO_H 1
#define HAVE_SYS_TYPES_H 1
#define HAVE_SYS_STAT_H 1
#define HAVE_FCNTL_H 1
#define HAVE_POLL_H 1
#define HAVE_SYS_IOCTL_H 1
#define HAVE_SYS_SOCKET_H 1
#define HAVE_SYS_UIO_H 1
#define HAVE_INTTYPES_H 1
#define HAVE_STDINT_H 1
#define HAVE_CLOCK_GETTIME 1
#define HAVE_EVENTFD 1
#define HAVE_TIMERFD 1
")
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/config.h "${LIBUSB_CONFIG_CONTENT}")

add_library(libusb STATIC ${LIBUSB_SOURCES})
target_include_directories(libusb
  PUBLIC
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>
  PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_CURRENT_BINARY_DIR}   # config.h 生成在此
)
# 强制预包含本项目生成的 config.h：
#   libusbi.h:23 是 #include <config.h>（尖括号），而全局 include 里
#   build-zig-x64/config（SOUI 的 config.h）排得更靠前，`#include <config.h>`
#   会命中 SOUI 的 config.h 而不是我们的 → PLATFORM_POSIX 等宏全丢。
#   -include 直接在命令行注入，无搜索顺序问题（Clang/GCC 均支持）。
target_compile_options(libusb PRIVATE "-include;${CMAKE_CURRENT_BINARY_DIR}/config.h")
# POSIX 线程（libusb io 层用 pthread）
find_package(Threads REQUIRED)
target_link_libraries(libusb PRIVATE Threads::Threads)

# 兼容性 / 紧凑：
#   - 关闭 udev 后端（linux_udev.c 未编译，无需 libudev）
#   - 关闭内置日志输出，减小体积（libusb 日志是可裁剪的）
target_compile_definitions(libusb PRIVATE
  NDEBUG
  -DENABLE_LOGGING=0
)

# 与项目整体一致：体积优先 + 死代码消除
if(CMAKE_C_COMPILER_ID MATCHES "GNU|Clang")
  # zig cc 报告 Clang；gcc 原生编译也走这里
  target_compile_options(libusb PRIVATE -Os -fno-unwind-tables -ffunction-sections -fdata-sections)
  set_target_properties(libusb PROPERTIES
    POSITION_INDEPENDENT_CODE ON
    INTERFACE_POSITION_INDEPENDENT_CODE ON
  )
endif()

install(TARGETS libusb
  CONFIGURATIONS Release
  ARCHIVE DESTINATION lib/Release
)

# 供其他 target 引用
set(LIBUSB_TARGET libusb CACHE INTERNAL "libusb vendored target")