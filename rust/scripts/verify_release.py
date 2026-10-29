#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""vpid-viewer 发布产物验证（Rust 发布硬化，参考 skill ui_win32_rust.md §9）。

用法:
    python verify_release.py [exe路径]

默认: rust/target/release/vpid-viewer.exe

检查项:
  1) 导出表为空（杀软敏感特征，正常 GUI 不应有导出表）
  2) .rsrc 段存在（VERSIONINFO + MANIFEST + ICON）
  3) 清单为纯 ASCII 且含 asInvoker/comctl v6（PerMonitorV2 亦应存在）
  4) 版本资源字段完整（FileDescription/ProductVersion/BuildDate）
  5) 图标已嵌入（RT_ICON + RT_GROUP_ICON）
  6) 校验和非零（editbin /RELEASE，未设则自动补）
  7) 子系统版本（现代 Windows >= 6.0）
"""
import os
import sys
import subprocess
import glob

# PE 数据目录索引（pefile 用数值，避免常量名差异）
DIR_EXPORT, DIR_RESOURCE = 0, 2
RT_MANIFEST, RT_VERSION, RT_ICON, RT_GROUP_ICON = 24, 16, 3, 14


def find_editbin():
    pats = [
        r"C:\Program Files (x86)\Microsoft Visual Studio\2022\*\VC\Tools\MSVC\*\bin\Hostx64\x64\editbin.exe",
        r"C:\Program Files\Microsoft Visual Studio\2022\*\VC\Tools\MSVC\*\bin\Hostx64\x64\editbin.exe",
    ]
    hits = []
    for p in pats:
        hits.extend(glob.glob(p))
    if not hits:
        return None
    # 按工具集版本目录名排序（14.3x > 14.2x），取最新
    return max(hits, key=lambda h: h.replace("\\", "/").split("/Tools/MSVC/")[-1].split("/")[0])


def resource_data(pe, type_id):
    """取指定资源类型的第一个数据的原始字节。资源树为 type -> id -> lang -> data。"""
    if pe.OPTIONAL_HEADER.DATA_DIRECTORY[DIR_RESOURCE].VirtualAddress == 0:
        return None

    def find(entries):
        for ent in entries:
            data = getattr(ent, "data", None)
            if data is not None:
                # data.struct = IMAGE_RESOURCE_DATA_ENTRY；原始内容在 OffsetToData/Size
                s = getattr(data, "struct", None)
                off = getattr(s, "OffsetToData", None)
                size = getattr(s, "Size", None)
                if off is not None and size is not None:
                    return pe.get_data(off, size)
                return None
            d = getattr(ent, "directory", None)
            if d is not None:
                r = find(d.entries)
                if r is not None:
                    return r
        return None

    for ent in pe.DIRECTORY_ENTRY_RESOURCE.entries:
        if ent.id == type_id and getattr(ent, "directory", None):
            return find(ent.directory.entries)
    return None


def main():
    exe = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        os.path.dirname(__file__), "..", "target", "release", "vpid-viewer.exe")
    exe = os.path.abspath(exe)
    if not os.path.exists(exe):
        print(f"[FAIL] exe 不存在: {exe}")
        return 1

    import pefile
    pe = pefile.PE(exe, fast_load=False)

    results = []

    def check(name, ok, detail=""):
        results.append((name, ok))
        print(f"  [{'OK ' if ok else 'FAIL'}] {name}" + (f"  {detail}" if detail else ""))

    print(f"== vpid-viewer 发布验证 ==\n  exe: {exe}\n  size: {os.path.getsize(exe)} bytes")

    # 1) 导出表（数据目录索引 0）
    no_export = pe.OPTIONAL_HEADER.DATA_DIRECTORY[DIR_EXPORT].VirtualAddress == 0
    check("导出表为空（杀软敏感）", no_export)

    # 2) .rsrc 段
    check(".rsrc 段存在", any(b".rsrc" in s.Name for s in pe.sections))

    # 3) 清单纯 ASCII + 关键元素
    man = resource_data(pe, RT_MANIFEST)
    if man is None:
        check("清单已嵌入", False)
        check("清单纯 ASCII", False)
    else:
        ok = b"asInvoker" in man and b"Common-Controls" in man and b"PerMonitorV2" in man
        check("清单已嵌入 (asInvoker/comctl/PerMonitorV2)", ok)
        check("清单纯 ASCII", all(b < 128 for b in man))

    # 4) 版本资源字段（pe.FileInfo = [ [StringFileInfo,...] ] 双层）
    ver_ok, builddate = False, ""
    try:
        for group in getattr(pe, "FileInfo", []):
            for item in group:
                for s in getattr(item, "StringTable", []) or []:
                    for k, v in s.entries.items():
                        if k == b"FileDescription":
                            ver_ok = v.decode("utf-8", "replace") == "USB Device ID Viewer"
                        if k == b"BuildDate":
                            builddate = v.decode("utf-8", "replace")
    except Exception:
        pass
    check("版本资源 FileDescription 正确", ver_ok, f"BuildDate={builddate}")

    # 5) 图标
    check("图标已嵌入 (RT_ICON+RT_GROUP_ICON)",
          resource_data(pe, RT_ICON) is not None and resource_data(pe, RT_GROUP_ICON) is not None)

    # 6) 校验和（未设则 editbin /RELEASE 补）
    cs = pe.OPTIONAL_HEADER.CheckSum
    if cs == 0:
        eb = find_editbin()
        if eb:
            print("  ... 校验和为 0，调用 editbin /RELEASE 设置")
            r = subprocess.run([eb, "/RELEASE", exe], capture_output=True, text=True,
                               errors="replace")
            out = (r.stdout or "").strip() or (r.stderr or "").strip()
            if out:
                print("  ...", out[:120])
            pe = pefile.PE(exe, fast_load=False)
            cs = pe.OPTIONAL_HEADER.CheckSum
        else:
            print("  [WARN] editbin 未找到，无法设校验和（仅非正规构建信号，不影响运行）")
    check("PE 校验和非零", cs != 0, f"CheckSum=0x{cs:X}")

    # 7) 子系统版本
    maj, mn = pe.OPTIONAL_HEADER.MajorSubsystemVersion, pe.OPTIONAL_HEADER.MinorSubsystemVersion
    check("子系统版本 >= 6.0", (maj, mn) >= (6, 0), f"{maj}.{mn}")

    dlls = sorted({e.dll.decode() for e in pe.DIRECTORY_ENTRY_IMPORT})
    print("  [INFO] 导入 DLL:", ", ".join(dlls))

    failed = [r for r in results if not r[1]]
    print(f"\n== {'PASS' if not failed else 'FAIL'} ({len(results) - len(failed)}/{len(results)} 项通过) ==")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
