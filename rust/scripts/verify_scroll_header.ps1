# Sticky-header regression check: scroll the "all devices" table, then screenshot
# to confirm the header stays pinned at top while rows scroll underneath it.
# Usage: powershell -File verify_scroll_header.ps1   (real scan, 35 devices -> scrollable)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr wparam, IntPtr lparam);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
[W]::SetProcessDPIAware() | Out-Null
$exe = "D:\code\vpid\rust\target\release\vpid-viewer.exe"
$out = "D:\code\vpid\rust\shot_scroll.png"
# Real scan (no --demo): 35 devices, so the left table always overflows and scrolls.
$p = Start-Process -FilePath $exe -PassThru
$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 40; $i++) {
  Start-Sleep -Milliseconds 250
  $p.Refresh()
  if (-not $p.HasExited) { $hwnd = [IntPtr]$p.MainWindowHandle }
  if ($hwnd -ne [IntPtr]::Zero) { break }
}
if ($hwnd -eq [IntPtr]::Zero) {
  Write-Output "NO WINDOW"
  if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
  exit 1
}
[W]::ShowWindow($hwnd, 9) | Out-Null   # SW_RESTORE
[W]::SetForegroundWindow($hwnd) | Out-Null
[W]::SetWindowPos($hwnd, [IntPtr](-1), 0, 0, 0, 0, 0x0003) | Out-Null  # HWND_TOPMOST
Start-Sleep -Milliseconds 2500          # let the first scan finish and populate the list
# Inject wheel: each notch = -120 (scroll down one row). 60 notches clamps to max scroll,
# so the first visible row must be near the END of the 35-device list (proves rows scrolled).
$WM_MOUSEWHEEL = 0x020A
$wparam = [IntPtr]0xFF880000            # high word = -120 (down), low word = 0 (no keys)
for ($k = 0; $k -lt 60; $k++) {
  [W]::PostMessage($hwnd, $WM_MOUSEWHEEL, $wparam, [IntPtr]::Zero) | Out-Null
  Start-Sleep -Milliseconds 30
}
Start-Sleep -Milliseconds 500           # let it repaint
$r = New-Object W+RECT
[W]::GetWindowRect($hwnd, [ref]$r) | Out-Null
$w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
Write-Output "window rect: $($r.Left),$($r.Top) ${w}x${h}"
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "saved $out"
[W]::SetWindowPos($hwnd, [IntPtr](-2), 0, 0, 0, 0, 0x0003) | Out-Null  # HWND_NOTOPMOST
Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
