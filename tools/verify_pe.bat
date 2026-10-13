@echo off
set VSPATH=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools
call "%VSPATH%\VC\Auxiliary\Build\vcvarsall.bat" x86 >nul 2>&1
echo === PE Headers: subsystem + machine ===
dumpbin /headers "D:\code\vpid\build-ninja\vpid_viewer.exe" 2>&1 | findstr /i "subsystem machine " | head -6
echo.
echo === Verify no Vista+ APIs in FULL import table ===
dumpbin /imports "D:\code\vpid\build-ninja\vpid_viewer.exe" 2>&1 | findstr /i "GetFileSizeEx SetFilePointerEx FlsAlloc FlsFree FlsGetValue FlsSetValue CompareStringEx FindFirstFileExW InitializeCriticalSectionEx AcquireSRWLock GetTickCount64 EncodePointer DecodePointer" && echo "!!! FOUND VISTA+ APIS !!!" || echo "OK: No Vista+ APIs in import table"
echo.
echo === Tls APIs (YY-Thunks Fls fallback evidence) ===
dumpbin /imports "D:\code\vpid\build-ninja\vpid_viewer.exe" 2>&1 | findstr /i "TlsAlloc TlsFree TlsGetValue TlsSetValue GetFileSize SetFilePointer" | head -8
echo.
echo === File info ===
dir "D:\code\vpid\build-ninja\vpid_viewer.exe" | findstr vpid
