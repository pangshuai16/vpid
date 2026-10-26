#!/usr/bin/env python3
"""固定 PE TimeDateStamp 并重算 PE CheckSum，使"同源码同参数"的构建产出确定性哈希。

背景（实测）：MSVC link 默认把构建时刻写进 COFF TimeDateStamp（COFF 头 pe+8）。
本项目里该 4 字节值还被复制到 .data 一处，所以全文件有 2 处字节随构建时刻变化
→ 同源码同参数连编，尺寸完全一致但 sha256 每次都不同。

把它钉成固定常量后，同源码同参数即可得到完全一致的字节与哈希。
Loader 不校验可执行文件的时间戳，但会校验/使用 PE CheckSum；固定时间戳后必须重算。

用法:
    pin_timestamp.py <exe> [stamp_hex]
stamp_hex 缺省为 0x64B0D000（2022-01-01 UTC，固定常量）。
"""
import struct
import sys


def compute_pe_checksum(data, off_ck):
    """按 PE 规范计算文件 CheckSum（512 字节块 16-bit 累加，字段先清零）。"""
    data = bytearray(data)
    struct.pack_into("<I", data, off_ck, 0)
    total = 0
    for off in range(0, len(data), 512):
        block = data[off:off + 512]
        s = 0
        words = (len(block) // 2) * 2
        for i in range(0, words, 2):
            s += struct.unpack_from("<H", block, i)[0]
        if len(block) % 2:
            s += block[-1]
        while s > 0xFFFF:
            s = (s & 0xFFFF) + (s >> 16)
        total = (total + s) & 0xFFFFFFFF
    return total & 0xFFFF


def parse_pe_fields(data):
    """解析本项目发布的 PE32 常用字段，避免依赖第三方 PE 库。"""
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    opt_size = struct.unpack_from("<H", data, pe + 20)[0]
    opt = pe + 24
    magic = struct.unpack_from("<H", data, opt)[0]
    is_pe32 = magic == 0x10B
    data_dirs = opt + (112 if is_pe32 else 128)
    cert_rva, cert_size = struct.unpack_from("<II", data, data_dirs + 4 * 8)
    sections = []
    sec_off = opt + opt_size
    for i in range(struct.unpack_from("<H", data, pe + 6)[0]):
        b = sec_off + i * 40
        raw_ptr, raw_size = struct.unpack_from("<II", data, b + 20)
        vsize = struct.unpack_from("<I", data, b + 8)[0]
        chars = struct.unpack_from("<I", data, b + 36)[0]
        sections.append((data[b:b+8].rstrip(b"\x00").decode("ascii", "replace"), raw_ptr, raw_size, vsize, chars))
    return {
        "pe": pe, "opt": opt, "is_pe32": is_pe32, "cert_rva": cert_rva,
        "cert_size": cert_size, "sections": sections,
        "timestamp_off": pe + 8,
        "checksum_off": opt + (64 if is_pe32 else 80),
        "dll_chars_off": opt + (70 if is_pe32 else 72),
    }


def main():
    if len(sys.argv) < 2:
        sys.stderr.write("usage: pin_timestamp.py <exe> [stamp_hex]\n")
        return 2

    path = sys.argv[1]
    stamp = int(sys.argv[2], 16) if len(sys.argv) > 2 else 0x64B0D000

    data = bytearray(open(path, "rb").read())
    if len(data) < 64 or data[:2] != b"MZ":
        # 非 PE（如 zig 交叉编译的 Linux ELF）：时间戳由 ELF 构建可复现性保证，
        # 无需处理。返回 0 避免 POST_BUILD 失败。
        print("[pin_timestamp] not a PE file (ELF?), skip: %s" % path)
        return 0

    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if not data[pe:pe + 4] == b"PE\x00\x00" or struct.unpack_from("<H", data, pe + 24)[0] != 0x10B:
        sys.stderr.write("仅支持本项目发布的 PE32 产物，跳过: %s\n" % path)
        return 1
    fields = parse_pe_fields(data)
    off_ts = fields["timestamp_off"]
    off_ck = fields["checksum_off"]
    off_dll = fields["dll_chars_off"]
    # XP/旧系统兼容：强制去掉 HIGH_ENTROPY_VA，保留 DYNAMIC_BASE。
    dll_chars = struct.unpack_from("<H", data, off_dll)[0]
    if dll_chars & 0x20:
        struct.pack_into("<H", data, off_dll, dll_chars & ~0x20)
    orig_ts = struct.unpack_from("<I", data, off_ts)[0]
    orig = orig_ts.to_bytes(4, "little")
    new = stamp.to_bytes(4, "little")

    ck = compute_pe_checksum(data, off_ck)
    old_ck = struct.unpack_from("<I", data, off_ck)[0]

    if orig != new:
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

        ck = compute_pe_checksum(data, off_ck)
        struct.pack_into("<I", data, off_ck, ck)
        open(path, "wb").write(data)
        locs = ", ".join("0x%X" % p for p in positions)
        print("[pin_timestamp] %08x -> %08x，%d 处 (%s)" % (orig_ts, stamp, len(positions), locs))
        print("[pin_timestamp] CheckSum %08x -> %08x, DllCharacteristics -> %04x" % (old_ck, ck, struct.unpack_from("<H", data, off_dll)[0]))
        if len(positions) > 4:
            sys.stderr.write("警告：出现 %d 处，可能有巧合的 4 字节匹配" % len(positions), file=sys.stderr)
    elif old_ck != ck:
        struct.pack_into("<I", data, off_ck, ck)
        open(path, "wb").write(data)
        print("[pin_timestamp] CheckSum %08x -> %08x, DllCharacteristics -> %04x" % (old_ck, ck, struct.unpack_from("<H", data, off_dll)[0]))
    else:
        print("[pin_timestamp] 时间戳与 CheckSum 均已定值，no-op")
    return 0


if __name__ == "__main__":
    sys.exit(main())
