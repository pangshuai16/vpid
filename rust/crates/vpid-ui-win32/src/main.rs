//! vpid-viewer —— 纯 Rust Windows 原生 UI 入口。
//!
//! 用法：
//!   vpid-viewer            正常启动（真实 USB 扫描）
//!   vpid-viewer --demo     演示桩数据（验证渲染/交互，无真实设备也有画面）
//!
//! 说明：
//!   - UI/UX 完全沿用主线（布局/主题/交互 1:1 复刻）。
//!   - CLI 自检（--scan）保留：见下方 TryCliScan（复用 vpid-core 扫描链路）。
//!   - 平台后端由 `vpid-ui-core::platform::Platform` 抽象（当前 Win32）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod paint;
mod provider;
mod widgets;

use std::sync::Arc;

use vpid_ui_core::app::AppState;
use vpid_ui_core::platform::{DeviceProvider, Platform};

fn main() {
    // CLI 自检：--scan / /scan 打印设备列表后退出（不启动 GUI）
    if try_cli_scan() {
        return;
    }

    // DPI 感知（必须在任何窗口创建前设置）。
    // 官方方案：Per-Monitor V2（Win10 1703+）——多显示器各自缩放的最佳模式，
    // 拖动窗口到不同缩放的显示器会收到 WM_DPICHANGED，应用按新 DPI 重建字号/布局。
    // 不用静态调用：SetProcessDpiAwarenessContext 是 1703+ 才有的 user32 导出，
    // 直接 import 会在老系统（Win10 1607/8.1/7）因导入表缺符号而**加载失败**；
    // 用 GetProcAddress 动态解析 + 逐级回退，保证"用在不同电脑"都能启动且不模糊。
    unsafe { enable_dpi_awareness() };

    let use_demo = std::env::args().any(|a| a == "--demo");
    let provider: Arc<dyn DeviceProvider> = if use_demo {
        Arc::new(provider::StubProvider)
    } else {
        Arc::new(provider::RealProvider::new())
    };

    let result = app::Win32App.run(AppState::default(), provider);
    if let Err(e) = result {
        eprintln!("vpid-viewer 启动失败: {e}");
        std::process::exit(1);
    }
}

/// 按官方 Per-Monitor V2 模式设置进程 DPI 感知，带逐级回退。
/// - 首选 `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`（Win10 1703+）：每显示器独立缩放。
/// - 回退 `SetProcessDPIAware()`（Vista+）：系统 DPI 感知（至少不模糊，但多显示器统一用主屏 DPI）。
/// 函数经 `GetProcAddress` 动态解析，避免老系统导入表缺符号导致进程无法加载。
unsafe fn enable_dpi_awareness() {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2;

    let user32 = GetModuleHandleW(crate::paint::to_wide("user32.dll").as_ptr());
    if user32.is_null() {
        return;
    }

    // 1) Per-Monitor V2
    let p = GetProcAddress(user32, b"SetProcessDpiAwarenessContext\0".as_ptr() as *const u8);
    if let Some(addr) = p {
        // FARPROC 是 `fn() -> isize`，按目标签名 transmute（fn 指针大小一致）
        let f: unsafe extern "system" fn(*mut core::ffi::c_void) -> i32 =
            std::mem::transmute(addr);
        if f(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) != 0 {
            return;
        }
    }

    // 2) 兜底：系统 DPI 感知（Vista+）
    let p = GetProcAddress(user32, b"SetProcessDPIAware\0".as_ptr() as *const u8);
    if let Some(addr) = p {
        let f: unsafe extern "system" fn() -> i32 = std::mem::transmute(addr);
        let _ = f();
    }
}

/// CLI 自检：`vpid-viewer --scan`（对齐旧版 TryCliScan，走 vpid-core 扫描链路）。
fn try_cli_scan() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let want = args
        .iter()
        .any(|a| a == "--scan" || a == "/scan");
    if !want {
        return false;
    }
    let devs = vpid_core::scanner::scan_usb_devices();
    println!("vpid scan: {} device(s)", devs.len());
    for d in &devs {
        println!(
            "  VID:{} PID:{} serial:{} name:{}",
            d.formatted_vid(),
            d.formatted_pid(),
            d.serial,
            d.display_name()
        );
    }
    true
}
