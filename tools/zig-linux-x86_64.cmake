# =====================================================================
# zig 交叉编译工具链：x86_64-linux-gnu（glibc floor 2.17）
# ---------------------------------------------------------------------
# 用法（配合 zig 0.16.0+，zig 需在 PATH）：
#   cmake -S . -B build-zig-x64 \
#         -DCMAKE_TOOLCHAIN_FILE=${PWD}/tools/zig-linux-x86_64.cmake \
#         -G Ninja
#   cmake --build build-zig-x64 --config Release
#
# glibc floor：-target x86_64-linux-gnu.2.17 表示产物兼容 glibc >= 2.17
# （CentOS 7 / RHEL 7 / Ubuntu 12.04+ 均满足）。
#
# 说明：
#   - CMAKE_C_COMPILER 用 "zig;cc;-target;..." 列表形式（CMake 会把分号列表当 argv），
#     zig cc 自身识别 -target 及后续参数。
#   - FIND_ROOT_PATH_MODE_* = ONLY：交叉编译禁止搜索 host 系统的头/库，
#     只允许 zig 自带 sysroot（glibc headers）与 vendored 源码。
#   - CMAKE_TRY_COMPILE_TARGET_TYPE=STATIC_LIBRARY：CMake 探测阶段只编不链，
#     避免在 host(Windows) 上尝试链接 Linux ELF。
#   - CMAKE_SYSTEM_NAME=Linux 使主 CMakeLists 的 UNIX 分支生效
#     （否则 Windows host 上 UNIX=FALSE，会走错构建路径）。
# =====================================================================
set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR x86_64)

set(ZIG_GLIBC_FLOOR "2.17" CACHE STRING "zig glibc floor (最低兼容版本)")
set(ZIG_TARGET "x86_64-linux-gnu.${ZIG_GLIBC_FLOOR}" CACHE STRING "zig target triple")

# 编译器：zig cc / zig c++，-target 直接内联进 argv（zig 官方推荐写法，
# 避免依赖 CMAKE_C_COMPILER_TARGET 在跨版本间的行为差异）。
set(CMAKE_C_COMPILER   "zig;cc;-target;${ZIG_TARGET}")
set(CMAKE_CXX_COMPILER "zig;c++;-target;${ZIG_TARGET}")

# archiver / ranlib：zig 自带（zig ar / zig ranlib），否则 CMake 找 host 的 ar
# （Windows 无 ar，CMAKE_AR-NOTFOUND 会直接失败）。
# 注意：不能用 "zig;ar" 分号列表——Windows 下 cmd.exe 会把 ';' 当命令分隔符拆坏
# （报 unknown command: ;ar）。用 tools/zig-ar.cmd / tools/zig-ranlib.cmd wrapper。
set(CMAKE_AR "${CMAKE_CURRENT_LIST_DIR}/zig-ar.cmd")
set(CMAKE_RANLIB "${CMAKE_CURRENT_LIST_DIR}/zig-ranlib.cmd")

# 交叉文件里让 CMake 用编译而非链接可执行测试程序（避免无谓的链接探测）
set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)

# 禁用 precompiled headers：SOUI 的 target_precompile_headers 在 Clang/zig 下
# 用 -Xclang -emit-pch 生成 .pch，zig lld 链接时把 .pch 当 .o 报 unknown file type。
# CMake 官方开关，target_precompile_headers 将静默无效（编译 PCH 的头文件
# 时多编一次，体积/速度影响可忽略）。
set(CMAKE_DISABLE_PRECOMPILE_HEADERS ON)

# SOUI 源码依赖 PCH 隐式注入 souistd.h（每个 .cpp 开头都没有 include 它，
# 直接使用 SNSBEGIN/SStringA 等）。禁用 PCH 后必须用编译器 -include 强制注入，
# 否则 soui4 编译报 unknown type name 'SNSBEGIN'。
# 注意：不加到 CMAKE_*_FLAGS（会波及第三方 C 库——souistd.h 是 C++ 头），
# 在 SOUI target 上用 COMPILE_OPTIONS 单独加（见主 CMakeLists 的补丁）。
set(SOUI_FORCE_INCLUDE "D:/code/vpid/third_party/soui/SOUI/include/souistd.h" CACHE STRING
    "souistd.h path for forced -include (SOUI PCH replacement)")

# 交叉编译：绝不找 host 系统的包
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)

# 交叉编译时用 stub pkg-config（tools/zig-pkg-config/pkg-config，恒返回"包不存在"）：
#   swinx/linux.cmake 与 fontconfig/CMakeLists.txt 里有 find_package(PkgConfig REQUIRED)，
#   host(Windows) 无 pkg-config 时 REQUIRED 会 FATAL；stub 让 find_package 成功，
#   而所有 pkg_check_modules() 干净降级（可选依赖如 ALSA 自动关闭）。
#   本项目 C 库依赖全部 vendored，无需系统 .pc 文件。
set(CMAKE_PROGRAM_PATH "${CMAKE_CURRENT_LIST_DIR}/zig-pkg-config")

# dbus 交叉编译：无法 autodetect session socket 目录，显式给默认值
# （dbus 运行时用 $XDG_RUNTIME_DIR / /tmp 计算，编译期只需占位路径）
set(DBUS_SESSION_SOCKET_DIR "/tmp" CACHE STRING "dbus session socket dir (cross-compile)")

# fontconfig 交叉误报修复：
#   check_function_exists() 在交叉编译时只编译不链接（STATIC_LIBRARY try_compile），
#   导致 getprogname（glibc 2.32+）/ getexecname（Solaris）被误报存在。
#   实际上 glibc 2.17 floor 没有这些函数，链接期会 undefined symbol。
#   预置 FALSE 强制 fcdefault.c 走 HAVE_READLINK 分支（Linux 正确实现）。
set(HAVE_GETPROGNAME FALSE CACHE BOOL "getprogname not in glibc 2.17 (cross)")
set(HAVE_GETEXECNAME FALSE CACHE BOOL "getexecname is Solaris-only (cross)")

# 静态链接 libgcc（zig 默认动态链接 libgcc_s，XP/旧发行版无 libgcc_s 时启动失败）
set(CMAKE_EXE_LINKER_FLAGS_INIT "-static-libgcc")

# Linux 系统库链接路径（libxcb / libXau / libuuid）：
#   swinx 链接 -lxcb -lXau -luuid，交叉编译（Windows host）无系统 .so。
#   tools/fetch_linux_sysroot.py 从 Ubuntu deb 提取了这些 .so 到
#   tools/linux-sysroot/lib。zig 的 clang driver 在 Windows 上对 `-L盘符路径`
#   （如 -LD:\...）解析错误（把 D: 当库名前缀），因此不用 -L，
#   改由主 CMakeLists 显式传绝对路径 .so + -Wl,--allow-shlib-undefined
#   （目标机运行时会用自己系统库解析符号，--allow-shlib-undefined 允许
#   Ubuntu 库里的 getrandom@GLIBC_2.25 等引用不被校验）。
set(LINUX_SYSROOT_LIB "${CMAKE_CURRENT_LIST_DIR}/linux-sysroot/lib")

# 禁止 CMake 自动加 BUILD_RPATH：链接了 build 目录外的 .so（linux-sysroot）时，
# CMake 默认会把该目录（Windows 绝对路径 D:/code/vpid/...）写进产物 RPATH，
# 污染 Linux 运行时的库搜索路径。目标机应使用自己的系统库目录。
set(CMAKE_SKIP_BUILD_RPATH ON)
set(CMAKE_SKIP_RPATH ON)