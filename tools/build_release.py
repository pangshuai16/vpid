#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
vpid 统一 Release 构建脚本（替代 GitHub Actions）
=================================================
Windows 宿主上一次性产出全部平台产物，并做完整性验证。

产物输出（dist/）：
  vpid_viewer_windows_x86.exe   —— MSVC x86，XP 兼容，1.5MB 级（strip + MSVC 体积优化）
  vpid_viewer_linux_amd64       —— zig 交叉，glibc floor 2.17（CentOS7+/Ubuntu12.04+）
  vpid_viewer_linux_arm64       —— zig 交叉，aarch64，glibc floor 2.17

用法：
  python tools/build_release.py [--only windows|linux-x64|linux-arm64] [--no-verify] [--dist DIR]

依赖：
  - CMake + Ninja（VS2022 BuildTools 自带，或系统 PATH）
  - VS2022 BuildTools（MSVC x86 工具链，Windows 产物）
  - zig 0.17.0（在 PATH；Linux 交叉产物）
  - tools/linux-sysroot/{lib,lib-arm64}/ （已入库，无需联网）
"""
import argparse
import os
import shutil
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DIST = os.path.join(REPO, "dist")
BUILD = os.path.join(REPO, ".build-release")   # 脚本专用构建目录（不入库）

# 候选工具路径
CMAKE_CANDIDATES = [
    "cmake",
    r"C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe",
]
NINJA_CANDIDATES = [
    r"C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja/ninja.exe",
]
ZIG_CANDIDATES = ["zig", r"C:/tools/zig-0.17.0/zig-x86_64-windows-0.17.0/zig.exe"]

# pixman 交叉探测必需（try_run 在交叉编译下要求预置退出码）
PIXMAN_EXITCODES = [
    "-DTOOLCHAIN_SUPPORTS_ATTRIBUTE_CONSTRUCTOR_EXITCODE=0",
    "-DTOOLCHAIN_SUPPORTS_ATTRIBUTE_DESTRUCTOR_EXITCODE=0",
    "-DASM_LEADING_UNDERSCORE_EXITCODE=1",
]


def find_tool(cands, name):
    for c in cands:
        if os.path.isfile(c) or shutil.which(c):
            return c
    raise RuntimeError(f"找不到 {name}，请安装或加入 PATH（候选：{cands}）")


def run(cmd, **kw):
    print("+", " ".join(str(c) for c in cmd), flush=True)
    return subprocess.run([str(c) for c in cmd], cwd=REPO, **kw)


def build_windows_x86(cmake, ninja):
    vcvars = r"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
    if not os.path.exists(vcvars):
        raise RuntimeError(f"找不到 vcvarsall.bat: {vcvars}")
    bdir = os.path.join(BUILD, "win-x86")
    # 不主动清空构建目录：CMake 增量重配即可（app.rc/源码变化会触发重编译）。
    # 若曾用 VS 生成器构建过同一目录，需手动删除 .build-release/win-x86 后再跑
    # （生成器缓存不兼容；脚本内不做 rmtree，避免与安全删除策略冲突）。
    os.makedirs(bdir, exist_ok=True)
    # XP 兼容三层机制以 Ninja 生成器为前提：VS 生成器把 SDK kernel32.lib 硬编码进
    # AdditionalDependencies 末尾，YY-Thunks kernel32.Lib 无法排到它前面，_WeakAlias
    # 弱别名失效；Ninja 生成器的 LINK_LIBRARIES 顺序完全可控（见 XP_COMPATIBILITY.md 2.4）。
    # /MANIFEST:EMBED 在当前 CMake + Ninja 下已验证正常（RT_MANIFEST 正确嵌入）。
    # 注意：批处理必须纯 ASCII（无中文注释，避免 GBK 乱码被当成命令执行）。
    ninja_dir = os.path.dirname(os.path.abspath(ninja))
    bat = os.path.join(bdir, "build_win.bat")
    with open(bat, "w", encoding="ascii") as f:
        f.write(
            "@echo off\r\n"
            'call "{vcvars}" x86 >nul\r\n'
            "if errorlevel 1 exit /b 1\r\n"
            'set "PATH={ninja_dir};%PATH%"\r\n'
            '"{cmake}" -S "{repo}" -B "{bdir}" -G Ninja -DCMAKE_BUILD_TYPE=Release\r\n'
            "if errorlevel 1 exit /b 1\r\n"
            '"{cmake}" --build "{bdir}"\r\n'
            "if errorlevel 1 exit /b 1\r\n".format(
                vcvars=vcvars,
                ninja_dir=ninja_dir.replace("/", "\\"),
                cmake=cmake.replace("/", "\\"),
                repo=REPO.replace("/", "\\"),
                bdir=bdir.replace("/", "\\"),
            )
        )
    r = run(["cmd", "/c", bat])
    if r.returncode:
        raise RuntimeError("Windows MSVC 构建失败")
    exe = os.path.join(bdir, "vpid_viewer.exe")
    if not os.path.exists(exe):
        raise RuntimeError(f"Windows 产物缺失: {exe}")
    # XP 兼容硬门禁：零致命 Vista+ 导入（check_imports.py 发现致命 API 时退出码非 0）
    chk = os.path.join(REPO, "tools", "check_imports.py")
    if os.path.exists(chk):
        r = run([sys.executable, chk, exe])
        if r.returncode:
            raise RuntimeError("XP 导入检查失败：产物含致命 Vista+ API，拒绝发布")
    out = os.path.join(DIST, "vpid_viewer_windows_x86.exe")
    shutil.copy2(exe, out)
    print(f"  -> {out} ({os.path.getsize(out)} bytes)")
    return out


def build_linux_zig(cmake, zig, toolchain, bdir_name, out_name):
    export = {"PATH": os.path.dirname(zig) + os.pathsep + os.environ.get("PATH", ""),
              "ZIG_LOCAL_CACHE": r"C:/tools/zig-cache",
              "ZIG_GLOBAL_CACHE": r"C:/tools/zig-global-cache"}
    env = {**os.environ, **export}
    bdir = os.path.join(BUILD, bdir_name)
    os.makedirs(bdir, exist_ok=True)
    # 先生成"内嵌资源"用的 COFF 资源对象（rc.exe + cvtres.exe → 单段 .rsrc），
    # 由 CMake 用 .incbin 嵌入 ELF。→ 单二进制自包含，无需外置 uires/。
    coff = os.path.join(bdir, "soui_coff.obj")
    embed_c = os.path.join(bdir, "soui_coff_embed.c")
    r = run([sys.executable, os.path.join(REPO, "tools", "gen_linux_coff_res.py"),
             "--out", coff, "--c-out", embed_c])
    if r.returncode or not os.path.exists(embed_c):
        raise RuntimeError(f"{bdir_name} COFF 资源对象生成失败")
    r = run([cmake, "-S", ".", "-B", bdir, "-G", "Ninja",
             f"-DCMAKE_TOOLCHAIN_FILE={os.path.join(REPO, 'tools', toolchain)}",
             f"-DVPID_COFF_RES_C={embed_c.replace(os.sep, '/')}"] + PIXMAN_EXITCODES, env=env)
    if r.returncode:
        raise RuntimeError(f"{bdir_name} CMake configure 失败")
    r = run([cmake, "--build", bdir, "--config", "Release"], env=env)
    if r.returncode:
        raise RuntimeError(f"{bdir_name} 构建失败")
    elf = os.path.join(bdir, "bin", "vpid_viewer")
    if not os.path.exists(elf):
        raise RuntimeError(f"{bdir_name} 产物缺失: {elf}")
    out = os.path.join(DIST, out_name)
    shutil.copy2(elf, out)
    os.chmod(out, 0o755)
    # Linux 产物为单文件自包含 ELF（UI 资源已内嵌），可直接运行，不再打包 uires/。
    print(f"  -> {out} ({os.path.getsize(out)} bytes)")
    return out


def main():
    global DIST
    ap = argparse.ArgumentParser(description="vpid 统一 Release 构建")
    ap.add_argument("--only", choices=["windows", "linux-x64", "linux-arm64"], help="只构建指定平台")
    ap.add_argument("--no-verify", action="store_true", help="跳过产物验证")
    ap.add_argument("--dist", default=DIST, help="产物输出目录")
    args = ap.parse_args()

    DIST = os.path.abspath(args.dist)
    os.makedirs(DIST, exist_ok=True)
    os.makedirs(BUILD, exist_ok=True)

    cmake = find_tool(CMAKE_CANDIDATES, "CMake")
    ninja = find_tool(NINJA_CANDIDATES, "Ninja")
    zig = find_tool(ZIG_CANDIDATES, "zig")
    print(f"工具: cmake={cmake}\n      ninja={ninja}\n      zig={zig}")

    artifacts = []
    if args.only in (None, "windows"):
        artifacts.append(("windows_x86", build_windows_x86(cmake, ninja)))
    if args.only in (None, "linux-x64"):
        artifacts.append(("linux_amd64", build_linux_zig(cmake, zig, "zig-linux-x86_64.cmake",
                                                           "zig-x64", "vpid_viewer_linux_amd64")))
    if args.only in (None, "linux-arm64"):
        artifacts.append(("linux_arm64", build_linux_zig(cmake, zig, "zig-linux-aarch64.cmake",
                                                           "zig-arm64", "vpid_viewer_linux_arm64")))

    if not args.no_verify:
        verify = os.path.join(REPO, "tools", "verify_release.py")
        if os.path.exists(verify):
            r = run([sys.executable, verify, DIST])
            if r.returncode:
                raise RuntimeError("产物验证失败")

    print("\n=== Release 构建完成 ===")
    for name, path in artifacts:
        print(f"  {name}: {path} ({os.path.getsize(path)} bytes)")


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"\n[FATAL] {e}", file=sys.stderr)
        sys.exit(1)