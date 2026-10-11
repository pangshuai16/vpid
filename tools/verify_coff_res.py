#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
离线校验：镜像 swinx/src/coffparser.cpp 的解析逻辑，验证生成的 COFF 资源对象。

用途：本机没有 Linux 运行环境时，先确认"资源对象结构 + 大小写规则"与
swinx 解析器一致（尤其 FindResourceW 会对查询串做大写化）。

用法：
  python tools/verify_coff_res.py <coff.obj> [--expect TYPE:NAME ...]

返回码 0 = 校验通过。
"""
import argparse
import struct
import sys

SUPPORTED_MACHINES = {0x014c, 0x8664, 0x01c4, 0xaa64}


def parse_coff(blob):
    """返回 (dirBase, resources) —— resources: list of dict(type,name,lang,size,off)"""
    machine, nsects = struct.unpack_from("<HH", blob, 0)
    assert machine in SUPPORTED_MACHINES, f"COFF machine 非法: 0x{machine:04x}"
    assert nsects > 0, "NumberOfSections = 0"

    # 段表：COFF_FILE_HEADER(20B) + sect*40B
    dir_base = None
    for i in range(nsects):
        off = 20 + i * 40
        name = blob[off:off + 8].rstrip(b"\x00")
        raw_size, raw_ptr = struct.unpack_from("<II", blob, off + 16)
        # 镜像 swinx：memcmp(name, ".rsrc", 5)==0 前缀匹配，取第一个
        if name.startswith(b".rsrc") or name.startswith(b".rdata") or name.startswith(b".data"):
            dir_base = raw_ptr
            break
    assert dir_base is not None, "找不到 .rsrc/.rdata/.data 段"

    resources = []

    def parse_dir(dir_off, path, depth):
        if depth > 3:
            return
        chars, tds, maj, minv, n_named, n_id = struct.unpack_from("<IIHHHH", blob, dir_base + dir_off)
        total = n_named + n_id
        for i in range(total):
            e = dir_base + dir_off + 16 + i * 8
            name_field, off_field = struct.unpack_from("<II", blob, e)
            if name_field & 0x80000000:
                # 字符串名（UTF-16LE）
                so = dir_base + (name_field & 0x7FFFFFFF)
                (slen,) = struct.unpack_from("<H", blob, so)
                cur = blob[so + 2: so + 2 + slen * 2].decode("utf-16-le")
            else:
                cur = "#%u" % (name_field & 0xFFFF)
            newpath = path + [cur]
            if off_field & 0x80000000:
                parse_dir(off_field & 0x7FFFFFFF, newpath, depth + 1)
            else:
                de = dir_base + off_field
                data_rva, size, codepage, reserved = struct.unpack_from("<IIII", blob, de)
                resources.append({
                    "type": newpath[0] if len(newpath) >= 1 else "",
                    "name": newpath[1] if len(newpath) >= 2 else "",
                    "lang": newpath[2] if len(newpath) >= 3 else "#0",
                    "size": size,
                    "off": dir_base + data_rva,
                })

    parse_dir(0, [], 0)
    return dir_base, resources


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("obj")
    ap.add_argument("--expect", action="append", default=[],
                    help="期望存在的 TYPE:NAME（大小写不敏感），可多次")
    ap.add_argument("-q", "--quiet", action="store_true")
    args = ap.parse_args()

    blob = open(args.obj, "rb").read()
    dir_base, resources = parse_coff(blob)

    # 镜像 FindResourceW：查询串转大写；存储保持原样
    table = {}
    for r in resources:
        table.setdefault(r["type"], {}).setdefault(r["name"], []).append(r)

    if not args.quiet:
        print(f"COFF 对象 {len(blob)} 字节，dirBase=0x{dir_base:x}，"
              f"解析到 {len(resources)} 个资源")
        types = {}
        for r in resources:
            types.setdefault(r["type"], []).append(r["name"])
        for t, names in sorted(types.items()):
            print(f"  [{t}] {len(names)} 项: {', '.join(sorted(set(names)))}")

    fail = 0
    for exp in args.expect:
        t, _, n = exp.partition(":")
        tq, nq = t.upper(), n.upper()
        hit = table.get(tq, {}).get(nq)
        mark = "OK " if hit else "MISS"
        if not hit:
            fail += 1
        print(f"  [{mark}] {tq}:{nq}" + (f"  ({hit[0]['size']}B)" if hit else ""))

    if fail:
        print(f"FAIL: {fail} 个期望资源未找到", file=sys.stderr)
        return 1
    print("PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
