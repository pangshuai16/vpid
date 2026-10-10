# =====================================================================
# zig 交叉编译工具链：aarch64-linux-gnu（glibc floor 2.17）
# ---------------------------------------------------------------------
# 与 tools/zig-linux-x86_64.cmake 相同，仅 target 换为 aarch64。
# CMAKE_SYSTEM_NAME=Linux 使主 CMakeLists 的 UNIX 分支生效。
# 用法：
#   cmake -S . -B build-zig-arm64 \
#         -DCMAKE_TOOLCHAIN_FILE=${PWD}/tools/zig-linux-aarch64.cmake \
#         -DVPID_UI_STYLE=SOUI -G Ninja
#   cmake --build build-zig-arm64 --config Release
# =====================================================================
set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR aarch64)

set(ZIG_GLIBC_FLOOR "2.17" CACHE STRING "zig glibc floor (最低兼容版本)")
set(ZIG_TARGET "aarch64-linux-gnu.${ZIG_GLIBC_FLOOR}" CACHE STRING "zig target triple")

# 编译器：zig cc / zig c++，-target 直接内联进 argv（zig 官方推荐写法）
set(CMAKE_C_COMPILER   "zig;cc;-target;${ZIG_TARGET}")
set(CMAKE_CXX_COMPILER "zig;c++;-target;${ZIG_TARGET}")

# archiver / ranlib：zig 自带（zig ar / zig ranlib），否则 CMake 找 host 的 ar
# 注意：不能用 "zig;ar" 分号列表——Windows 下 cmd.exe 会把 ';' 当命令分隔符拆坏。
set(CMAKE_AR "${CMAKE_CURRENT_LIST_DIR}/zig-ar.cmd")
set(CMAKE_RANLIB "${CMAKE_CURRENT_LIST_DIR}/zig-ranlib.cmd")

# 交叉文件里让 CMake 用编译而非链接可执行测试程序
set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)

# 禁用 PCH（SOUI 的 target_precompile_headers 在 zig lld 下失败，见 x86_64 版注释）
set(CMAKE_DISABLE_PRECOMPILE_HEADERS ON)

# 交叉编译：绝不找 host 系统的包
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)

# 交叉编译时用 stub pkg-config（见 x86_64 版注释）
set(CMAKE_PROGRAM_PATH "${CMAKE_CURRENT_LIST_DIR}/zig-pkg-config")

# dbus 交叉编译：无法 autodetect session socket 目录，显式给默认值
set(DBUS_SESSION_SOCKET_DIR "/tmp" CACHE STRING "dbus session socket dir (cross-compile)")

# fontconfig 交叉误报修复（同 x86_64 版）：getprogname 是 glibc 2.32+，
# glibc 2.17 floor 下不存在，强制走 HAVE_READLINK 分支。
set(HAVE_GETPROGNAME FALSE CACHE BOOL "getprogname not in glibc 2.17 (cross)")
set(HAVE_GETEXECNAME FALSE CACHE BOOL "getexecname is Solaris-only (cross)")

# 静态链接 libgcc
set(CMAKE_EXE_LINKER_FLAGS_INIT "-static-libgcc")

# Linux 系统库链接路径（libxcb / libXau / libuuid），见 x86_64 版注释。
# arm64 用 lib-arm64（aarch64 版 .so，从 Ubuntu ports 提取），x64 用 lib。
set(LINUX_SYSROOT_LIB "${CMAKE_CURRENT_LIST_DIR}/linux-sysroot/lib-arm64")

# 禁止 CMake 自动加 BUILD_RPATH（见 x86_64 版注释）
set(CMAKE_SKIP_BUILD_RPATH ON)
set(CMAKE_SKIP_RPATH ON)