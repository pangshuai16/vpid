#!/usr/bin/env python3
"""固定 PE TimeDateStamp，使"同源码同参数"的构建产出确定性哈希。

背景（实测）：MSVC link 默认把构建时刻写进 COFF TimeDateStamp（COFF 头 pe+8）。
本项目里该 4 字节值还被复制到 .data 一处，所以全文件有 2 处字节随构建时刻变化
→ 同源码同参数连编，尺寸完全一致但 sha256 每次都不同。

把它钉成定值后，同源码同参数即可得到完全一致的字节与哈希。
Loader 不校验可执行文件的时间戳，置 0 安全。

用法:
    pin_timestamp.py <exe> [stamp_hex]
stamp_hex 缺省为 0（全零）。
"""
import struct
import sys


def main():
    if len(sys.argv) < 2:
        sys.stderr.write("usage: pin_timestamp.py <exe> [stamp_hex]\n")
        return 2

    path = sys.argv[1]
    stamp = int(sys.argv[2], 16) if len(sys.argv) > 2 else 0

    data = bytearray(open(path, "rb").read())
    if len(data) < 64 or data[:2] != b"MZ":
        # 非 PE（如 zig 交叉编译的 Linux ELF）：时间戳由 ELF 构建可复现性保证，
        # 无需处理。返回 0 避免 POST_BUILD 失败。
        print("[pin_timestamp] not a PE file (ELF?), skip: %s" % path)
        return 0

    pe = struct.unpack_from("<I", data, 0x3C)[0]
    off_ts = pe + 8  # COFF: Machine(2) + NumberOfSections(2) + TimeDateStamp(4)
    orig_ts = struct.unpack_from("<I", data, off_ts)[0]
    orig = orig_ts.to_bytes(4, "little")
    new = stamp.to_bytes(4, "little")

    if orig == new:
        print("[pin_timestamp] 时间戳已是定值 %08x，no-op" % orig_ts)
        return 0

    # 记录所有出现位置（正常情况下 2 处：COFF 头 + .data 内副本）
    positions = []
    i = 0
    while True:
        i = data.find(orig, i)
        if i < 0:
            break
        positions.append(i)
        i += 4

    if not positions:
        sys.stderr.write("未找到时间戳字节，跳过\n")
        return 1

    for pos in positions:
        data[pos:pos + 4] = new

    open(path, "wb").write(data)
    locs = ", ".join("0x%X" % p for p in positions)
    print("[pin_timestamp] %08x -> %08x，%d 处 (%s)" % (orig_ts, stamp, len(positions), locs))
    if len(positions) > 4:
        print("[pin_timestamp] 警告：出现 %d 处，可能有巧合的 4 字节匹配" % len(positions), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
