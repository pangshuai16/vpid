#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""验证 vpid-viewer 窗口类图标运行时加载（skill ui_win32_rust.md 9.2.2）。

用法: 先启动 vpid-viewer.exe --demo，再运行本脚本。
检查: FindWindowW("VPidMainWnd") 后 GetClassLongPtrW(-14 /* GCLP_HICON */) 非零。
"""
import ctypes
import time

u = ctypes.windll.user32
u.FindWindowW.restype = ctypes.c_void_p
u.FindWindowW.argtypes = [ctypes.c_wchar_p, ctypes.c_wchar_p]
u.GetClassLongPtrW.restype = ctypes.c_ssize_t
u.GetClassLongPtrW.argtypes = [ctypes.c_void_p, ctypes.c_int]

hwnd = 0
for _ in range(50):  # 轮询最多 5s 等窗口
    hwnd = u.FindWindowW("VPidMainWnd", None)
    if hwnd:
        break
    time.sleep(0.1)

if not hwnd:
    print("[FAIL] window not found (class VPidMainWnd)")
    raise SystemExit(1)

hicon = u.GetClassLongPtrW(hwnd, -14)  # GCLP_HICON
print(f"hwnd=0x{hwnd:X}  hIcon=0x{hicon:X}")
print("[OK] 窗口类图标已加载" if hicon else "[FAIL] 窗口类图标未加载（任务栏将显示默认图标）")
raise SystemExit(0 if hicon else 1)
