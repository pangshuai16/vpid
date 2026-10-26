#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
为 Linux ELF 生成"内嵌资源"用的 COFF 资源对象。

背景
----
swinx 的 resapi.cpp 在 Linux 上通过 dlsym 查找符号：
    _binary_soui_coff_o_start / _binary_soui_coff_o_end
若存在，则把二者之间的内存当作"COFF 资源对象"解析（WindResResourceParser）。
这就是官方 windres 机制：windres 把 .rc 编译成 COFF 对象 -> 汇编 .incbin 嵌入。

本机没有 MinGW windres，但 Windows SDK 的 rc.exe + MSVC 的 cvtres.exe
组合等价：rc.exe 把 .rc 编译为 .res（PE 资源二进制），cvtres.exe 把 .res
转换为带 .rsrc 段的 COFF 对象。COFF 对象再作为数据嵌入 ELF。

关键约束（来自 swinx/src/coffparser.cpp）
---------------------------------------
* ParseResourceDirectory 按资源里的**原始大小写**存储 type/name；
  而 FindResourceW 会把查询串 **转大写** 再查表。
  => 资源名/类型必须全部写成大写，才能匹配。
* SResProviderPE::MyFindResource 把字符串类型 "ICON"/"BITMAP"/"CURSOR"
  映射为数字类型 RT_GROUP_ICON/RT_BITMAP/RT_GROUP_CURSOR，
  查询时经 ResourceIdToString 变成 "#14"/"#2"/"#12"。
  => 图标必须用 rc 的 ICON 关键字声明（生成 RT_GROUP_ICON 数字类型）。

用法
----
  python tools/gen_linux_coff_res.py --out .build-release/soui_coff.obj
"""
import argparse
import os
import re
import shutil
import struct
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# 资源来源（.rc2 里 DEFINE_UIRES 是权威清单，随之自动同步）
APP_RC2 = os.path.join(REPO, "src", "ui", "uires.rc2")
SYS_RC2 = os.path.join(REPO, "platform", "resources", "theme_sys_res.rc2")
# 窗口图标（rc ICON 关键字 -> RT_GROUP_ICON 数字类型，名字 APP_ICON）
APP_ICON = os.path.join(REPO, "assets", "app-icon.ico")

RC_EXE_CANDIDATES = [
    r"C:/Program Files (x86)/Windows Kits/10/bin/10.0.26100.0/x64/rc.exe",
]
CVTRES_CANDIDATES = [
    r"C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64/cvtres.exe",
]

DEFINE_RE = re.compile(r'DEFINE_UIRES\s*\(\s*([^,]+?)\s*,\s*([^,]+?)\s*,\s*"([^"]+)"\s*\)')


def find_tool(cands, name, glob_root=None):
    for c in cands:
        if os.path.isfile(c):
            return c
    if glob_root:
        import glob
        for pat in glob_root:
            hits = sorted(glob.glob(pat))
            if hits:
                return hits[-1]
    raise RuntimeError(f"找不到 {name}（候选：{cands} / {glob_root}）")


def parse_rc2(path):
    """从 .rc2 提取 (name, type, abspath) 列表，路径相对该 rc2 所在目录。"""
    base = os.path.dirname(path)
    out = []
    with open(path, "r", encoding="utf-8-sig") as f:
        text = f.read()
    for m in DEFINE_RE.finditer(text):
        name, rtype, rel = m.group(1).strip(), m.group(2).strip(), m.group(3).strip()
        ap = os.path.normpath(os.path.join(base, rel.replace("\\", "/")))
        out.append((name, rtype, ap))
    return out


def build_rc(entries, out_rc):
    lines = [
        "// 自动生成：vpid Linux 内嵌资源清单（请勿手改，见 tools/gen_linux_coff_res.py）",
        "// 资源名/类型统一大写：swinx 查询时会转大写后匹配。",
        "",
    ]
    missing = []
    for name, rtype, ap in entries:
        if not os.path.isfile(ap):
            missing.append(ap)
            continue
        # 路径统一用正斜杠（rc.exe 接受），转义双引号
        p = ap.replace("\\", "/").replace('"', '\\"')
        lines.append(f'{name.upper()} {rtype.upper()} "{p}"')
    with open(out_rc, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    return missing


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True, help="输出 COFF 对象路径 (.obj)")
    ap.add_argument("--c-out", default=None,
                    help="同时输出内嵌用 C 源码（.incbin 内联汇编），供 CMake 直接编译")
    ap.add_argument("--workdir", default=None, help="中间文件目录（默认 out 同目录）")
    ap.add_argument("--keep", action="store_true", help="保留中间 .rc/.res")
    args = ap.parse_args()

    out_obj = os.path.abspath(args.out)
    workdir = os.path.abspath(args.workdir or os.path.dirname(out_obj))
    os.makedirs(workdir, exist_ok=True)
    rc_path = os.path.join(workdir, "vpid_linux_res.rc")
    res_path = os.path.join(workdir, "vpid_linux_res.res")

    rc_exe = find_tool(
        RC_EXE_CANDIDATES, "rc.exe",
        glob_root=[r"C:/Program Files (x86)/Windows Kits/10/bin/*/x64/rc.exe"],
    )
    cvtres = find_tool(
        CVTRES_CANDIDATES, "cvtres.exe",
        glob_root=[
            r"C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/VC/Tools/MSVC/*/bin/Hostx64/x64/cvtres.exe",
            r"C:/Program Files/Microsoft Visual Studio/2022/*/VC/Tools/MSVC/*/bin/Hostx64/x64/cvtres.exe",
        ],
    )

    entries = parse_rc2(SYS_RC2) + parse_rc2(APP_RC2)
    # 图标：用 ICON 关键字（RT_GROUP_ICON 数字类型 14），名字 APP_ICON
    if os.path.isfile(APP_ICON):
        entries.append(("APP_ICON", "ICON", APP_ICON))
    else:
        print(f"[warn] 图标不存在，跳过: {APP_ICON}", file=sys.stderr)

    missing = build_rc(entries, rc_path)
    if missing:
        for m in missing:
            print(f"[warn] 资源文件缺失（跳过）: {m}", file=sys.stderr)

    # rc.exe: .rc -> .res
    r = subprocess.run([rc_exe, "/nologo", "/fo", res_path, rc_path],
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0 or not os.path.exists(res_path):
        print("rc.exe 输出:", r.stdout, r.stderr, file=sys.stderr)
        raise RuntimeError("rc.exe 编译失败")
    if r.stdout.strip() or r.stderr.strip():
        print("rc.exe:", (r.stdout + r.stderr).strip())

    # cvtres.exe: .res -> COFF .obj（带 .rsrc 段）
    r = subprocess.run([cvtres, "/nologo", "/machine:x64", f"/out:{out_obj}", res_path],
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0 or not os.path.exists(out_obj):
        print("cvtres 输出:", r.stdout, r.stderr, file=sys.stderr)
        raise RuntimeError("cvtres 失败")
    if r.stdout.strip() or r.stderr.strip():
        print("cvtres:", (r.stdout + r.stderr).strip())

    # cvtres 输出的是"未链接"对象：.rsrc$01（目录）里指向 .rsrc$02（数据）的
    # DataRVA 字段全部是 0，靠 IMAGE_REL_AMD64_ADDR32NB 重定位在链接期修正。
    # 我们要把它当作纯数据嵌入 ELF（不经过链接器），故需离线应用重定位：
    #   DataRVA = len(.rsrc$01) + 目标符号(.rsrc$02 内偏移)
    # 再把两段合并为单个 .rsrc 段，包成一个最小 COFF 对象。
    raw_obj = out_obj + ".cvtres"
    shutil.move(out_obj, raw_obj)
    flatten_cvtres_object(raw_obj, out_obj)
    if not args.keep:
        for p in (res_path, raw_obj):
            if os.path.exists(p):
                os.remove(p)

    if args.c_out:
        write_embed_c(os.path.abspath(args.c_out), out_obj)

    print(f"[ok] COFF 资源对象: {out_obj} ({os.path.getsize(out_obj)} bytes), "
          f"资源条目 {len(entries)}")
    return 0


def write_embed_c(c_path, obj_path):
    """生成把 COFF 对象 .incbin 进 ELF 的 C 源码。

    用"文件级 __asm__"而不是 .S 文件：走标准 C 编译器（zig cc），
    避免 CMake ASM 语言在交叉工具链下把裸 `zig` 当汇编器调用（缺 `cc` 子命令）。
    """
    inc = obj_path.replace("\\", "/")
    src = (
        "/* 自动生成（tools/gen_linux_coff_res.py）：把 COFF 资源对象作为数据\n"
        "   嵌入 ELF。导出 swinx resapi 用 dlsym(RTLD_DEFAULT) 查找的符号\n"
        "   _binary_soui_coff_o_start/_end。 */\n"
        "__asm__(\n"
        "    \".section .rodata\\n\"\n"
        "    \".balign 8\\n\"\n"
        "    \".globl _binary_soui_coff_o_start\\n\"\n"
        "    \"_binary_soui_coff_o_start:\\n\"\n"
        "    \".incbin \\\"" + inc + "\\\"\\n\"\n"
        "    \".globl _binary_soui_coff_o_end\\n\"\n"
        "    \"_binary_soui_coff_o_end:\\n\"\n"
        "    \".balign 8\\n\"\n"
        ");\n"
    )
    with open(c_path, "w", encoding="utf-8") as f:
        f.write(src)


def _parse_sections(blob):
    machine, nsects, tsd, pts, nsym, optsz, chars = struct.unpack_from("<HHIIIHH", blob, 0)
    secs = []
    for i in range(nsects):
        off = 20 + i * 40
        name = blob[off:off + 8].rstrip(b"\x00").decode("latin1")
        vsize, vaddr, rawsize, rawptr, preloc, plinenum, nreloc, nlineno, ch = \
            struct.unpack_from("<IIIIIIHHI", blob, off + 8)
        secs.append(dict(name=name, rawsize=rawsize, rawptr=rawptr,
                         preloc=preloc, nreloc=nreloc))
    return dict(machine=machine, nsects=nsects, symtab=pts, nsyms=nsym, secs=secs)


def _sym_value(blob, info, idx):
    off = info["symtab"] + idx * 18
    val, secnum = struct.unpack_from("<iH", blob, off + 8)
    return val, secnum


def flatten_cvtres_object(src, dst):
    """把 cvtres 的多段未链接对象压成一个单段 .rsrc COFF 对象（含重定位修正）。"""
    blob = open(src, "rb").read()
    info = _parse_sections(blob)
    d1 = next((s for s in info["secs"] if s["name"].startswith(".rsrc") and s["nreloc"] > 0), None)
    d2 = next((s for s in info["secs"] if s["name"].startswith(".rsrc") and s["nreloc"] == 0), None)
    if d1 is None or d2 is None:
        raise RuntimeError(f"未找到 .rsrc$01/.rsrc$02 段（{src}）")

    d1_data = bytearray(blob[d1["rawptr"]:d1["rawptr"] + d1["rawsize"]])
    d2_data = blob[d2["rawptr"]:d2["rawptr"] + d2["rawsize"]]
    d2_rva = len(d1_data)  # .rsrc$02 紧邻 .rsrc$01 之后（4184 已 8 字节对齐）

    for k in range(d1["nreloc"]):
        field_off, sym_idx, rtype = struct.unpack_from("<IIH", blob, d1["preloc"] + k * 10)
        assert rtype == 0x0003, f"非预期重定位类型 0x{rtype:04x}"
        sym_val, sym_sec = _sym_value(blob, info, sym_idx)
        struct.pack_into("<I", d1_data, field_off, d2_rva + sym_val)

    merged = bytes(d1_data) + d2_data
    size = len(merged)

    # 最小 COFF 对象：20B 文件头 + 40B 段头（=60B 之后即段数据）
    hdr = struct.pack("<HHIIIHH", 0x8664, 1, 0, 0, 0, 0, 0)
    sec = struct.pack("<8sIIIIIIHHI", b".rsrc\x00\x00\x00", size, 0, size, 60,
                      0, 0, 0, 0, 0x40000040)  # INITIALIZED_DATA | READ
    with open(dst, "wb") as f:
        f.write(hdr + sec + merged)


if __name__ == "__main__":
    sys.exit(main())
