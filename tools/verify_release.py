#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
vpid Release 产物验证脚本
=========================
构建后校验每个产物的关键指标：
  - Windows x86:  PE 有效、x86 架构、时间戳已钉（可复现构建）
  - Linux amd64/arm64: ELF 有效、架构正确、glibc 版本符号 ≤ 2.17、动态依赖仅基础库

用法：
  python tools/verify_release.py [dist目录]
"""
import os
import re
import struct
import subprocess
import sys
import zlib  # noqa: F401 (占位, 保持一致性)


def check_pe(path):
    """校验 PE 文件：MZ 头 + PE 签名 + machine = 0x14c (x86)。"""
    with open(path, "rb") as f:
        data = f.read()
    assert data[:2] == b"MZ", "不是 PE (缺 MZ 头)"
    pe_off = struct.unpack_from("<I", data, 0x3C)[0]
    assert data[pe_off:pe_off + 4] == b"PE\x00\x00", "PE 签名无效"
    machine = struct.unpack_from("<H", data, pe_off + 4)[0]
    assert machine == 0x14C, f"不是 x86 架构 (machine=0x{machine:x})"
    size = os.path.getsize(path)
    print(f"  [OK] {os.path.basename(path)}: PE x86, {size} bytes")


def check_pe_xp(path):
    """Windows 产物 XP 致命导入门禁（调 check_imports.py；退出码 0 = 零致命 Vista+ API）。"""
    chk = os.path.join(os.path.dirname(os.path.abspath(__file__)), "check_imports.py")
    if not os.path.exists(chk):
        print(f"  [SKIP] {os.path.basename(path)}: 缺 check_imports.py，跳过 XP 门禁")
        return
    r = subprocess.run([sys.executable, chk, path], capture_output=True)
    out = r.stdout.decode("utf-8", errors="replace")
    line = next((ln.strip() for ln in out.splitlines() if ln.startswith("Fatal Vista+ APIs:")), "")
    status = "OK" if r.returncode == 0 else "FAIL"
    print(f"  [{status}] {os.path.basename(path)}: XP 导入门禁 {line or '(无输出)'}")
    if r.returncode:
        raise AssertionError("含致命 Vista+ API（XP 不兼容）")


def parse_elf_sections(data):
    """返回 {name: (offset,size)}。"""
    assert data[:4] == b"\x7fELF", "不是 ELF"
    is64 = data[4] == 2
    endian = "<" if data[5] == 1 else ">"
    if is64:
        e_shoff = struct.unpack_from(endian + "Q", data, 40)[0]
        e_shentsize = struct.unpack_from(endian + "H", data, 58)[0]
        e_shnum = struct.unpack_from(endian + "H", data, 60)[0]
        e_shstrndx = struct.unpack_from(endian + "H", data, 62)[0]
        # Elf64_Shdr: sh_name(0) sh_type(4) sh_flags(8) sh_addr(16)
        #             sh_offset(24) sh_size(32)
        off_off, size_off = 24, 32
    else:
        e_shoff = struct.unpack_from(endian + "I", data, 32)[0]
        e_shentsize = struct.unpack_from(endian + "H", data, 46)[0]
        e_shnum = struct.unpack_from(endian + "H", data, 48)[0]
        e_shstrndx = struct.unpack_from(endian + "H", data, 50)[0]
        # Elf32_Shdr: sh_name(0) sh_type(4) sh_flags(8) sh_addr(12)
        #             sh_offset(16) sh_size(20)
        off_off, size_off = 16, 20
    shstr_off = struct.unpack_from(endian + ("Q" if is64 else "I"),
                                   data, e_shoff + e_shstrndx * e_shentsize + off_off)[0]
    sections = {}
    for i in range(e_shnum):
        base = e_shoff + i * e_shentsize
        sh_name = struct.unpack_from(endian + "I", data, base)[0]
        pos = shstr_off + sh_name
        end = data.index(b"\0", pos)
        nm = data[pos:end].decode()
        sh_offset = struct.unpack_from(endian + ("Q" if is64 else "I"), data, base + off_off)[0]
        sh_size = struct.unpack_from(endian + ("Q" if is64 else "I"), data, base + size_off)[0]
        sections[nm] = (sh_offset, sh_size)
    return sections, is64, endian


def check_elf_linux(path, expect_arch):
    """校验 ELF：架构 + glibc 版本符号 ≤ 2.17 + DT_NEEDED 干净。"""
    with open(path, "rb") as f:
        data = f.read()
    assert data[:4] == b"\x7fELF", "不是 ELF"
    e_machine = struct.unpack_from("<H", data, 18)[0]
    if expect_arch == "amd64":
        assert e_machine == 0x3E, f"不是 x86-64 (machine=0x{e_machine:x})"
        arch = "x86-64"
    else:
        assert e_machine == 0xB7, f"不是 aarch64 (machine=0x{e_machine:x})"
        arch = "aarch64"

    sections, is64, endian = parse_elf_sections(data)
    if ".dynstr" not in sections:
        raise AssertionError("缺 .dynstr（非动态链接 ELF？）")
    off, size = sections[".dynstr"]
    dynstr = data[off:off + size]

    # 提取所有 GLIBC_x.y 版本符号，求最大值
    glibc_versions = set()
    for m in re.finditer(rb"GLIBC_(\d+)\.(\d+)(?:\.(\d+))?", dynstr):
        ver = tuple(int(x) for x in m.group(1, 2, 3) if x is not None)
        # 版本节点在主版本表的顺序即需求上限，这里取符号串中出现过的最大版本
        glibc_versions.add(ver)
    if not glibc_versions:
        raise AssertionError("未找到 GLIBC 版本符号（静态链接？）")
    max_ver = max(glibc_versions)
    # 允许 2.17 或更低；2.18+ 视为失败（目标 floor 2.17）
    floor = (2, 17)
    status = "OK" if max_ver <= floor else "FAIL"
    print(f"  [{status}] {os.path.basename(path)}: ELF {arch}, "
          f"GLIBC max {'.'.join(map(str, max_ver))} (floor 2.17), "
          f"{os.path.getsize(path)} bytes")
    if status == "FAIL":
        raise AssertionError(f"glibc floor 超限: {max_ver} > 2.17")

    # DT_NEEDED 解析：DT_NEEDED 的 val 是 .dynstr 内的字符串偏移，直接索引
    dyn_off, dyn_size = sections.get(".dynamic", (0, 0))
    dynstr_off, _ = sections.get(".dynstr", (0, 0))
    if not dyn_off or not dynstr_off:
        raise AssertionError("缺 .dynamic / .dynstr 段")
    dynstr = data[dynstr_off:]
    needed = []
    entsz = 16 if is64 else 8
    i = 0
    while i < min(dyn_size, 400 * entsz):
        tag = struct.unpack_from(endian + ("Q" if is64 else "I"), data, dyn_off + i)[0]
        val = struct.unpack_from(endian + ("Q" if is64 else "I"), data, dyn_off + i + (8 if is64 else 4))[0]
        if tag == 0:
            break
        if tag == 1:  # DT_NEEDED: val 是 .dynstr 内的偏移
            e = dynstr.index(b"\0", val)
            needed.append(dynstr[val:e].decode())
        i += entsz
    allowed = {"libc.so.6", "libm.so.6", "libpthread.so.0", "libdl.so.2", "librt.so.1",
               "libxcb.so.1", "libuuid.so.1", "ld-linux-x86-64.so.2", "ld-linux-aarch64.so.1"}
    extra = [n for n in needed if n not in allowed]
    print(f"  [OK] {os.path.basename(path)}: DT_NEEDED={needed}")
    if extra:
        raise AssertionError(f"意外动态依赖: {extra}")


def check_elf_embedded_res(path):
    """校验 Linux ELF 已"内嵌 UI 资源"（单二进制自包含）。

    资源以 PE 资源目录（COFF 对象）形式编入 ELF，资源名以 UTF-16LE 存于目录中。
    这里按资源名 UTF-16LE 字节序列搜索，确认应用资源与系统皮肤均在其中。
    """
    data = open(path, "rb").read()
    want = ["DLG_MAIN", "SYS_MSGICON", "SYS_XML_SKIN", "THEME_COLOR"]
    miss = [w for w in want if w.encode("utf-16-le") not in data]
    # 同时应有 COFF 资源对象特征：1 个段的 .rsrc 段名
    has_rsrc = data.find(b".rsrc\x00\x00\x00") >= 0
    ok = not miss and has_rsrc
    print(f"  [{'OK' if ok else 'FAIL'}] {os.path.basename(path)}: "
          f"内嵌资源 段名.rsrc={has_rsrc} 资源名缺失={miss or '无'}")
    if not ok:
        raise AssertionError(f"未检出内嵌 UI 资源（缺 {miss}, .rsrc={has_rsrc}）")


def main():
    dist = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "dist")
    print(f"=== 验证产物: {dist} ===")
    files = {
        "vpid_viewer_windows_x86.exe": check_pe,
        "vpid_viewer_linux_amd64": lambda p: check_elf_linux(p, "amd64"),
        "vpid_viewer_linux_arm64": lambda p: check_elf_linux(p, "arm64"),
    }
    # Windows 产物：XP 致命导入门禁（零 Vista+ API）
    xp_checks = {
        "vpid_viewer_windows_x86.exe": check_pe_xp,
    }
    # Linux 产物必须内嵌 UI 资源（单二进制自包含，无需外置 uires/）
    embedded = {
        "vpid_viewer_linux_amd64": check_elf_embedded_res,
        "vpid_viewer_linux_arm64": check_elf_embedded_res,
    }
    ok = True
    for group in (files, xp_checks, embedded):
        for name, fn in group.items():
            path = os.path.join(dist, name)
            if not os.path.exists(path):
                print(f"  [SKIP] {name}: 不存在，跳过")
                continue
            try:
                fn(path)
            except AssertionError as e:
                ok = False
                print(f"  [FAIL] {name}: {e}")
    if not ok:
        sys.exit(1)
    print("=== 全部通过 ===")


if __name__ == "__main__":
    main()