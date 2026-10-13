# -*- coding: utf-8 -*-
"""Build vpid with Ninja, using single cmd process with vcvarsall."""
import subprocess
import sys
import os

vs_path = r"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"
vcvars = os.path.join(vs_path, "VC", "Auxiliary", "Build", "vcvarsall.bat")
cmake_exe = os.path.join(vs_path, "Common7", "IDE", "CommonExtensions",
    "Microsoft", "CMake", "CMake", "bin", "cmake.exe")
ninja_dir = r"C:\Users\Administrator\AppData\Local\Programs\Python\Python313\Scripts"

# Build a batch script that runs everything in one process
batch = f"""@echo off
call "{vcvars}" x86
set PATH={ninja_dir};%PATH%
cd /d D:\\code\\vpid
if exist build-ninja rmdir /s /q build-ninja
"{cmake_exe}" -S . -B build-ninja -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded 2>&1
if errorlevel 1 exit /b 1
"{cmake_exe}" --build build-ninja --parallel 2>&1
exit /b %errorlevel%
"""

# Write batch file
batch_file = os.path.join(os.environ.get("TEMP", r"C:\Users\Administrator\AppData\Local\Temp"), "build_ninja_run.bat")
with open(batch_file, "w") as f:
    f.write(batch)

print(f"Running build batch: {batch_file}")
r = subprocess.run(
    ["cmd", "/c", batch_file],
    capture_output=True,
    timeout=300
)

# Decode output - cl.exe outputs in CP936/GBK
try:
    stdout = r.stdout.decode("utf-8", errors="replace")
except:
    stdout = r.stdout.decode("cp936", errors="replace")
try:
    stderr = r.stderr.decode("utf-8", errors="replace")
except:
    stderr = r.stderr.decode("cp936", errors="replace")

# Print only relevant lines
for line in stdout.splitlines():
    if any(k in line.lower() for k in ["error", "failed", "warning c", "=== ", "succeed", "linking", "build"]):
        print(line)

if stderr:
    for line in stderr.splitlines():
        if any(k in line.lower() for k in ["error", "failed"]):
            print("STDERR:", line)

print(f"\nExit code: {r.returncode}")

# Check output
exe = r"D:\code\vpid\build-ninja\vpid_viewer.exe"
if os.path.exists(exe):
    size = os.path.getsize(exe)
    print(f"Output: {exe} ({size} bytes, {size/1024/1024:.2f} MB)")
