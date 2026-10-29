#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
vpid 纯 Rust 发布构建（Windows 主产物，v2.x 起为唯一主线）。

流程：
  1. cargo build --release        -> rust/target/release/vpid-viewer.exe
  2. 复制为 dist/vpid_viewer_windows_x64.exe
  3. editbin /RELEASE             -> 写 PE 校验和（Python subprocess 调 editbin 安全：
                                    Git Bash 才做 /RELEASE 路径转换，此处不经过 Git Bash）
  4. verify_release.py            -> 8 项发布审计（导出表空/.rsrc/清单/版本/图标/校验和/子系统）

用法：
  python rust/scripts/build_release.py                 # 完整：build+copy+editbin+verify
  python rust/scripts/build_release.py --no-build      # 仅复制+硬化+验证（用已有产物）
  python rust/scripts/build_release.py --out dist/vpid_viewer_windows_x64.exe

依赖：
  - Rust 工具链（cargo/rustc 1.85+）
  - VS2022 MSVC 的 editbin.exe（硬化；缺失则告警不失败）
  - python pefile（verify_release.py 依赖；缺失则验证告警）
"""
import argparse
import os
import shutil
import subprocess
import sys

SCRIPT = os.path.dirname(os.path.abspath(__file__))
RUST = os.path.dirname(SCRIPT)          # .../rust
REPO = os.path.dirname(RUST)            # .../vpid
DIST = os.path.join(REPO, "dist")
OUT_DEFAULT = os.path.join(DIST, "vpid_viewer_windows_x64.exe")

EDITBIN_GLOBS = [
    r"C:\Program Files (x86)\Microsoft Visual Studio\2022\*\VC\Tools\MSVC\*\bin\Hostx64\x64\editbin.exe",
    r"C:\Program Files\Microsoft Visual Studio\2022\*\VC\Tools\MSVC\*\bin\Hostx64\x64\editbin.exe",
]


def newest_editbin():
    import glob
    best = None
    for g in EDITBIN_GLOBS:
        for p in glob.glob(g):
            if best is None or os.path.getmtime(p) > os.path.getmtime(best):
                best = p
    return best


def run(cmd, cwd=None, **kw):
    print("+", " ".join(str(c) for c in cmd), flush=True)
    return subprocess.run([str(c) for c in cmd], cwd=cwd, **kw)


def main():
    ap = argparse.ArgumentParser(description="vpid 纯 Rust 发布构建（Windows x64）")
    ap.add_argument("--no-build", action="store_true", help="跳过 cargo build，仅复制+硬化+验证")
    ap.add_argument("--out", default=OUT_DEFAULT, help="产物输出路径")
    args = ap.parse_args()

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    src_exe = os.path.join(RUST, "target", "release", "vpid-viewer.exe")

    # 1. 构建
    if not args.no_build:
        r = run(["cargo", "build", "--release"], cwd=RUST)
        if r.returncode != 0:
            sys.exit(f"[FAIL] cargo build --release 退出码 {r.returncode}")
    if not os.path.exists(src_exe):
        sys.exit(f"[FAIL] 未找到产物 {src_exe}（先构建）")

    # 2. 复制为发布命名
    shutil.copy2(src_exe, args.out)
    print(f"[OK ] 复制 -> {args.out}  ({os.path.getsize(args.out)} bytes)")

    # 3. editbin /RELEASE（写 PE 校验和）
    eb = newest_editbin()
    if not eb:
        print("[WARN] editbin 未找到；校验和保持 0（非致命，非官方构建信号）")
    else:
        print(f"[OK ] editbin: {eb}")
        r = run([eb, "/RELEASE", args.out])
        if r.returncode != 0:
            print(f"[WARN] editbin /RELEASE 失败（退出码 {r.returncode}）")

    # 4. 8 项发布审计
    r = run([sys.executable, os.path.join(SCRIPT, "verify_release.py"), args.out])
    if r.returncode != 0:
        sys.exit("[FAIL] verify_release.py 审计未通过")
    print(f"\n== 发布完成: {args.out} ==")


if __name__ == "__main__":
    main()
