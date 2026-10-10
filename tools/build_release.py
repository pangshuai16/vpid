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
    os.makedirs(bdir, exist_ok=True)
    # 用 Visual Studio 生成器（CMake vs_link_exe wrapper 对 Ninja 生成器与
    # /MANIFEST:EMBED 不兼容——mt.exe 报"No manifest inputs"；VS 生成器原生支持）。
    # vcvarsall x86 注入 MSVC 环境后跑 cmake + msbuild。
    # 注意：批处理必须纯 ASCII（无中文注释，避免 GBK 乱码被当成命令执行）。
    bat = os.path.join(bdir, "build_win.bat")
    with open(bat, "w", encoding="ascii") as f:
        f.write(
            "@echo off\r\n"
            'call "{vcvars}" x86 >nul\r\n'
            "if errorlevel 1 exit /b 1\r\n"
            '"{cmake}" -S "{repo}" -B "{bdir}" -G "Visual Studio 17 2022" -A Win32 -DVPID_UI_STYLE=SOUI\r\n'
            "if errorlevel 1 exit /b 1\r\n"
            '"{cmake}" --build "{bdir}" --config Release\r\n'
            "if errorlevel 1 exit /b 1\r\n".format(
                vcvars=vcvars,
                cmake=cmake.replace("/", "\\"),
                repo=REPO.replace("/", "\\"),
                bdir=bdir.replace("/", "\\"),
            )
        )
    r = run(["cmd", "/c", bat])
    if r.returncode:
        raise RuntimeError("Windows MSVC 构建失败")
    exe = os.path.join(bdir, "Release", "vpid_viewer.exe")
    if not os.path.exists(exe):
        raise RuntimeError(f"Windows 产物缺失: {exe}")
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
    r = run([cmake, "-S", ".", "-B", bdir, "-G", "Ninja",
             f"-DCMAKE_TOOLCHAIN_FILE={os.path.join(REPO, 'tools', toolchain)}",
             "-DVPID_UI_STYLE=SOUI"] + PIXMAN_EXITCODES, env=env)
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
    # Linux 产物必须配套 UI 资源目录（ELF 无 PE 资源段，运行时从
    # <exe_dir>/uires/ 加载）。打包成 tar.gz 保持单文件发布体验：
    #   vpid_viewer_linux_amd64.tar.gz
    #   ├── vpid_viewer_linux_amd64      （ELF，可执行）
    #   └── uires/
    #       ├── uires.idx                 （应用资源索引）
    #       ├── uidef/ values/ xml/       （应用 XML）
    #       └── soui-sys-resource/        （系统皮肤，含其 uires.idx）
    pkg = package_linux_assets(out, bdir_name)
    print(f"  -> {out} ({os.path.getsize(out)} bytes)")
    print(f"  -> {pkg} ({os.path.getsize(pkg)} bytes)")
    return out


def package_linux_assets(elf_path, bdir_name):
    """把 ELF + uires 资源目录打包成 tar.gz（单文件发布）。"""
    import tarfile
    pkg = elf_path + ".tar.gz"
    staging = os.path.join(BUILD, bdir_name, "pkg")
    if os.path.exists(staging):
        shutil.rmtree(staging)
    os.makedirs(staging, exist_ok=True)
    # 1. ELF
    shutil.copy2(elf_path, os.path.join(staging, os.path.basename(elf_path)))
    # 2. 应用资源：src/ui/uires/ -> uires/
    res_src = os.path.join(REPO, "src", "ui", "uires")
    res_dst = os.path.join(staging, "uires")
    shutil.copytree(res_src, res_dst)
    # 3. 系统皮肤：官方 soui-sys-resource -> uires/soui-sys-resource
    #    （只复制运行时需要的 uires.idx + theme_sys_res/，跳过 .rc/.cpp 等构建文件）
    sys_src = os.path.join(REPO, "third_party", "soui", "soui-sys-resource")
    sys_dst = os.path.join(res_dst, "soui-sys-resource")
    if os.path.exists(sys_src):
        os.makedirs(sys_dst, exist_ok=True)
        shutil.copy2(os.path.join(sys_src, "uires.idx"), os.path.join(sys_dst, "uires.idx"))
        theme_src = os.path.join(sys_src, "theme_sys_res")
        theme_dst = os.path.join(sys_dst, "theme_sys_res")
        shutil.copytree(theme_src, theme_dst)
    else:
        # 回退：项目自身的 theme_sys_res（含 uires.idx 需自建）
        sys_src2 = os.path.join(REPO, "platform", "resources", "theme_sys_res")
        os.makedirs(sys_dst, exist_ok=True)
        for f in os.listdir(sys_src2):
            shutil.copy2(os.path.join(sys_src2, f), os.path.join(sys_dst, f))
        shutil.copy2(os.path.join(REPO, "src", "ui", "uires", "uires.idx"),
                     os.path.join(sys_dst, "uires.idx"))
    # 4. 打包（tar.gz，根目录含 ELF + uires/）
    with tarfile.open(pkg, "w:gz") as tf:
        tf.add(os.path.join(staging, os.path.basename(elf_path)),
               arcname=os.path.basename(elf_path))
        tf.add(res_dst, arcname="uires")
    shutil.rmtree(staging)
    return pkg


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