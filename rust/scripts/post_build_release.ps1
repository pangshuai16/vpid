# Post-build hardening: set PE checksum (editbin /RELEASE), then verify release artifacts.
# Usage: powershell -File post_build_release.ps1
# Refs: skill desktop-app-build ui_win32_rust.md section 9 (checksum + audit).
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot   # ...\rust
$exe = Join-Path $root "target\release\vpid-viewer.exe"
if (-not (Test-Path $exe)) { Write-Output "[FAIL] exe not found: $exe"; exit 1 }
Write-Output "exe: $exe"

# Locate newest editbin (VS2022 BuildTools MSVC). editbin /RELEASE must run from
# PowerShell (Git Bash converts /RELEASE to C:\Program Files\Git\RELEASE -> LNK1342).
$eb = (Get-ChildItem "C:\Program Files (x86)\Microsoft Visual Studio\2022\*\VC\Tools\MSVC\*\bin\Hostx64\x64\editbin.exe" `
      -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
if (-not $eb) {
  Write-Output "[WARN] editbin not found; checksum stays 0 (non-fatal, non-official-build signal)"
} else {
  Write-Output "editbin: $eb"
  & $eb /RELEASE $exe
  if ($LASTEXITCODE -ne 0) { Write-Output "[WARN] editbin /RELEASE failed (exit $LASTEXITCODE)" }
}

# Audit (export table / .rsrc / manifest / version / icon / checksum / subsystem)
& python (Join-Path $PSScriptRoot "verify_release.py")
exit $LASTEXITCODE
