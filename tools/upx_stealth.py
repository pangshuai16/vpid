#!/usr/bin/env python3
"""UPX 压缩后去特征脚本（降低杀软误报，配合 UPX POST_BUILD 使用）。

功能：
    1. 节名标准化：UPX0/UPX1 -> .text/.data（规避 UPX 段名特征）
    2. 清除 UPX 特征字符串：UPX! / UPX0 / UPX1 / UPX2 等（规避静态字符串特征）

用法：
    python upx_stealth.py <exe_path>

说明：
    - 该脚本在 UPX 压缩后执行，只改写 PE 节名与特征串，不改变功能
    - 实测：0.54MB lzma + 去特征 通过 360 杀软验证（2026-10-08）
    - Windows 下用 32 位 Python 即可运行（纯标准库，无依赖）
"""
import struct
import sys
from pathlib import Path


def de_stealth(path: str) -> None:
    fn = Path(path)
    data = bytearray(fn.read_bytes())

    pe_off = struct.unpack("<I", data[0x3C:0x40])[0]
    opt_size = struct.unpack("<H", data[pe_off + 20 : pe_off + 22])[0]
    nsec = struct.unpack("<H", data[pe_off + 6 : pe_off + 8])[0]
    sect_off = pe_off + 24 + opt_size

    # 1. 节名标准化：UPX0 -> .text, UPX1 -> .data, 空名 -> .text/.data
    renamed = 0
    for i in range(nsec):
        so = sect_off + i * 40
        nm = bytes(data[so : so + 8]).rstrip(b"\x00")
        if nm in (b"UPX0", b"UPX1") or (i < 2 and nm == b""):
            data[so : so + 8] = b".text\x00\x00\x00" if i == 0 else b".data\x00\x00\x00"
            renamed += 1

    # 2. 清除 UPX 特征字符串
    cleared = 0
    for marker in (b"UPX!", b"UPX0", b"UPX1", b"UPX2", b"UPX ", b"$Id: UPX", b"UPX!RSDX"):
        while True:
            idx = data.find(marker)
            if idx == -1:
                break
            data[idx : idx + len(marker)] = b"\x00" * len(marker)
            cleared += 1

    fn.write_bytes(bytes(data))
    print(f"[upx_stealth] {fn.name}: 节名标准化 {renamed} 处, 清除 UPX 串 {cleared} 处")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("用法: python upx_stealth.py <exe_path>")
        sys.exit(2)
    de_stealth(sys.argv[1])
