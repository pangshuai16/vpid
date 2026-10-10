#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Fetch Linux sysroot shared libs (libxcb / libXau / libuuid) for zig cross-compile.
=================================================================================
Zig cross-compiles our code with glibc floor 2.17, but swinx links against
system libs: libxcb, libXau (xcb_auth), libuuid (objbase.cpp uuid_generate).
On Windows host there are no Linux .so files, so we extract them from Ubuntu
.deb packages into a minimal sysroot that the zig toolchain links against.

The resulting vpid_viewer dynamically links libxcb.so.1 / libXau.so.6 /
libuuid.so.1 at runtime, exactly like a native Linux build does (CI installs
libxcb1-dev etc.). Target machines already ship these base libraries.

Usage:
    python fetch_linux_sysroot.py
Output:
    <repo>/tools/linux-sysroot/lib/{libxcb.so,libxcb.so.1,libXau.so,...}

NOTE: writing a binary .so copy into tools/linux-sysroot is checked into git
so cross builds are reproducible offline. Re-run to refresh.
"""
import io
import lzma
import os
import subprocess
import sys
import tarfile
import urllib.request

UBUNTU = "http://archive.ubuntu.com/ubuntu"
POOL = f"{UBUNTU}/pool/main"

# (deb-url, tag)
PKGS = [
    (f"{POOL}/libx/libxcb/libxcb1_1.14-3ubuntu3_amd64.deb", "libxcb1"),
    (f"{POOL}/libx/libxcb/libxcb1-dev_1.14-3ubuntu3_amd64.deb", "libxcb1-dev"),
    (f"{POOL}/libx/libxau/libxau6_1.0.9-0ubuntu1_amd64.deb", "libxau6"),
    (f"{POOL}/libx/libxau/libxau-dev_1.0.9-0ubuntu1_amd64.deb", "libxau-dev"),
    (f"{POOL}/u/util-linux/libuuid1_2.37.2-4ubuntu3_amd64.deb", "libuuid1"),
]

# For each tag, which lib paths (in /usr/lib/x86_64-linux-gnu) to collect.
WANT = {
    "libxcb1": [("usr/lib/x86_64-linux-gnu/libxcb.so.1.1.0", "libxcb.so.1"),
                ("usr/lib/x86_64-linux-gnu/libxcb.so.1", "libxcb.so.1")],
    "libxcb1-dev": [("usr/lib/x86_64-linux-gnu/libxcb.so", "libxcb.so")],
    "libxau6": [("usr/lib/x86_64-linux-gnu/libXau.so.6.0.0", "libXau.so.6"),
                ("usr/lib/x86_64-linux-gnu/libXau.so.6", "libXau.so.6")],
    "libxau-dev": [("usr/lib/x86_64-linux-gnu/libXau.so", "libXau.so")],
    "libuuid1": [("usr/lib/x86_64-linux-gnu/libuuid.so.1.3.0", "libuuid.so.1"),
                 ("usr/lib/x86_64-linux-gnu/libuuid.so.1", "libuuid.so.1")],
}

CACHE = os.path.join(os.path.expanduser("~"), ".cache", "vpid-linux-sysroot")
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "linux-sysroot", "lib")


def fetch(url: str) -> bytes:
    os.makedirs(CACHE, exist_ok=True)
    name = url.rsplit("/", 1)[-1]
    cached = os.path.join(CACHE, name)
    if os.path.exists(cached):
        with open(cached, "rb") as f:
            return f.read()
    print(f"  downloading {name}", flush=True)
    with urllib.request.urlopen(url, timeout=120) as r:
        data = r.read()
    with open(cached, "wb") as f:
        f.write(data)
    return data


def extract_deb(data: bytes) -> dict:
    """Poor-man's ar reader -> {member: bytes}."""
    pos = 8
    members = {}
    while pos < len(data):
        if data[pos:pos + 2] == b"`\n":
            break
        try:
            name = data[pos:pos + 16].decode().strip()
            size = int(data[pos + 48:pos + 58].decode().strip())
        except Exception:
            break
        members[name] = data[pos + 60:pos + 60 + size]
        pos += 60 + size + (1 if (60 + size) % 2 else 0)
    return members


def untar(data: bytes) -> list:
    """Return [(path, bytes)] for all regular files."""
    files = []
    # data.tar may be zst / xz / gz
    for opener_name, opener in (
        ("zst", None),
        ("xz", lambda b: lzma.decompress(b)),
        ("gz", lambda b: __import__("gzip").decompress(b)),
    ):
        try:
            if opener_name == "zst":
                import zstandard
                raw = zstandard.ZstdDecompressor().decompress(data, max_output_size=1 << 30)
            else:
                raw = opener(data)
            tf = tarfile.open(fileobj=io.BytesIO(raw))
            for m in tf:
                if m.isfile():
                    files.append((m.name.lstrip("./"), tf.extractfile(m).read()))
            return files
        except Exception:
            continue
    raise RuntimeError("cannot unpack data.tar (xz/gz/zst all failed)")


def main():
    os.makedirs(OUT, exist_ok=True)
    collected = {}  # dest-name -> bytes
    # 每包要收集的 .so 路径（按文件做精确匹配）
    want_files = {
        "libxcb1": {
            "usr/lib/x86_64-linux-gnu/libxcb.so.1.1.0:libxcb.so.1",
            "usr/lib/x86_64-linux-gnu/libxcb.so.1:libxcb.so.1",
        },
        "libxcb1-dev": {"usr/lib/x86_64-linux-gnu/libxcb.so:libxcb.so"},
        "libxau6": {
            "usr/lib/x86_64-linux-gnu/libXau.so.6.0.0:libXau.so.6",
            "usr/lib/x86_64-linux-gnu/libXau.so.6:libXau.so.6",
        },
        "libxau-dev": {"usr/lib/x86_64-linux-gnu/libXau.so:libXau.so"},
        "libuuid1": {
            "usr/lib/x86_64-linux-gnu/libuuid.so.1.3.0:libuuid.so.1",
            "usr/lib/x86_64-linux-gnu/libuuid.so.1:libuuid.so.1",
        },
    }
    for url, tag in PKGS:
        deb = fetch(url)
        members = extract_deb(deb)
        data_key = next(k for k in members if k.startswith("data.tar"))
        files = untar(members[data_key])
        mapping = {spec.split(":")[0]: spec.split(":")[1]
                   for spec in want_files.get(tag, set())}
        for path, content in files:
            dest = mapping.get(path)
            if dest:
                collected[dest] = content
                print(f"  {tag}: {path} -> {dest} ({len(content)}B)")

    # symlink chain: libfoo.so -> libfoo.so.1 -> real file
    for name, content in collected.items():
        with open(os.path.join(OUT, name), "wb") as f:
            f.write(content)
        print(f"  wrote {OUT}/{name} ({len(content)}B)")

    # 创建 .so 链接（Windows 无管理员权限建不了 symlink，直接复制文件——
    # 链接器只需要该文件名存在，正式版/dev 包内容与运行时库一致）
    pairs = [("libxcb.so", "libxcb.so.1"), ("libXau.so", "libXau.so.6"),
             ("libuuid.so", "libuuid.so.1")]
    for link, target in pairs:
        lp = os.path.join(OUT, link)
        tp = os.path.join(OUT, target)
        if not os.path.exists(lp) and os.path.exists(tp):
            try:
                os.symlink(target, lp)
                print(f"  symlink {link} -> {target}")
            except OSError:
                import shutil
                shutil.copyfile(tp, lp)
                print(f"  copied {link} (= {target})")

    # sanity: is it a well-formed ELF shared object?
    for name in sorted(collected):
        p = os.path.join(OUT, name)
        with open(p, "rb") as f:
            magic = f.read(4)
        ok = magic == b"\x7fELF"
        print(f"  check {name}: ELF={ok}")
    print("DONE")


if __name__ == "__main__":
    sys.exit(main())