//! 发布硬化资源（winres）：VERSIONINFO + 清单 + 图标 + BuildDate。
//!
//! 参考 skill `desktop-app-build` 的 `references/ui_win32_rust.md` §9（Rust 发布硬化）：
//!   裸 `cargo build --release` 产物无版本信息/无清单/无图标/校验和为 0 —— 杀软与体验
//!   双重缺陷。本文件在编译期补齐。
//!
//! 稳健性：rc.exe 不在 PATH（常见于非 VS 开发者提示符）。本文件先 PATH 后 Windows Kits
//! 定位 SDK 根，显式 `set_toolkit_path`；全部找不到则打印 WARN 并跳过资源（不中断构建，
//! 保证任何环境都能编出来——只是少了资源）。
//!
//! 铁律（见 skill 9.2.1）：清单必须纯 ASCII（rc.exe 按系统 ACP 重编码非 ASCII 字节，
//! 会把 UTF-8 中文变成非法字节 → 整个 manifest 解析失败 → asInvoker/comctl v6 全失效）。
//! 故 manifest 与 description 用英文；中文只放 VERSIONINFO（UTF-16，无此问题）。

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");

    let Some(rc_bin) = locate_rc_bin() else {
        println!("cargo:warning=[build.rs] rc.exe 未找到（PATH + Windows Kits），跳过版本信息/清单/图标资源（优雅降级）");
        return;
    };

    // winres 内部以 `rc.exe` 为子进程，需在 PATH 里能找到其依赖（bin\x64 下有配套 DLL）
    prepend_path(&rc_bin);

    let mut res = winres::WindowsResource::new();
    // ⚠️ winres 源码：先试 `toolkit_path/rc.exe`，所以 toolkit_path 必须是 rc.exe 所在目录
    // （...\Windows Kits\10\bin\<ver>\x64），不是 SDK 根。
    res.set_toolkit_path(rc_bin.to_str().unwrap_or(""));

    let ver = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    res.set("FileDescription", "USB Device ID Viewer");
    res.set("ProductName", "USB Device ID Viewer");
    res.set("CompanyName", "USB Manager");
    res.set("LegalCopyright", "USB Manager");
    res.set("OriginalFilename", "vpid-viewer.exe");
    res.set("InternalName", "vpid-viewer");
    res.set("FileVersion", &ver);
    res.set("ProductVersion", &ver);
    res.set("BuildDate", &build_timestamp());
    res.set_manifest(MANIFEST);

    if let Some(icon) = icon_path() {
        res.set_icon(&icon);
    }

    if let Err(e) = res.compile() {
        println!("cargo:warning=[build.rs] winres 编译资源失败（非致命，产物无版本信息/图标）：{e}");
    }
}

/// 清单：asInvoker + comctl32 v6 + PerMonitorV2 DPI + supportedOS（Win7~10）。
/// 全部 ASCII；`dpiAwareness` 与代码侧 `SetProcessDpiAwarenessContext`（GetProcAddress
/// 动态解析，见 main.rs `enable_dpi_awareness`）双保险——老系统忽略清单元素，
/// 代码侧回退 SetProcessDPIAware。
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="vpid.viewer" type="win32"/>
  <description>USB Device ID Viewer</description>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0" processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}"/>
      <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}"/>
      <supportedOS Id="{35138b9a-5d96-4fbd-8e2d-a2440225f93a}"/>
    </application>
  </compatibility>
</assembly>"#;

/// 定位 rc.exe 所在目录（...\Windows Kits\10\bin\<ver>\x64）。
/// 策略：① PATH 里已有 rc.exe；② 扫 Windows Kits 取最大版本。
fn locate_rc_bin() -> Option<PathBuf> {
    // ① PATH
    if let Ok(out) = std::process::Command::new("where").arg("rc.exe").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = s.lines().next() {
                let rc = PathBuf::from(line.trim());
                if rc.exists() {
                    return rc.parent().map(|p| p.to_path_buf());
                }
            }
        }
    }
    // ② Windows Kits 扫描（取最大版本目录）
    let kits = r"C:\Program Files (x86)\Windows Kits\10\bin";
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(kits) {
        for e in rd.flatten() {
            let vdir = e.path();
            let c = vdir.join("x64").join("rc.exe");
            if c.exists() {
                cands.push(c);
            }
        }
    }
    let best = cands
        .into_iter()
        .max_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()))?;
    best.parent().map(|p| p.to_path_buf())
}

fn prepend_path(dir: &Path) {
    let cur = env::var("PATH").unwrap_or_default();
    let new = if cur.is_empty() {
        dir.to_string_lossy().into_owned()
    } else {
        format!("{};{}", dir.display(), cur)
    };
    // rust 1.80+ 环境变量写入为 unsafe（进程级，单线程 build.rs 内安全）
    unsafe { env::set_var("PATH", new) };
}

/// 主线图标：vpid-ui-win32 -> crates -> rust -> vpid -> assets/app-icon.ico
fn icon_path() -> Option<String> {
    let md = env::var("CARGO_MANIFEST_DIR").ok()?;
    let p = Path::new(&md).join("../../../assets/app-icon.ico");
    if p.exists() {
        println!("cargo:rerun-if-changed={}", p.display());
        Some(p.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// 构建时间（UTC+8），写入版本资源 BuildDate。
fn build_timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let t = secs + 8 * 3600; // UTC+8，无时区 API，按偏移处理
    let days = (t / 86400) as i64;
    let rem = t % 86400;
    let (y, m, d) = civil_from_days(days);
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}")
}

/// Howard Hinnant 的 civil_from_days（days since 1970-01-01 -> y/m/d），纯 std。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
