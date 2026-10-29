//! vpid-viewer-linux —— Linux GTK3 原生 UI 入口。
//!
//! 用法：
//!   vpid-viewer-linux            正常启动（真实 USB 扫描）
//!   vpid-viewer-linux --demo     演示桩数据（验证渲染/交互，无真实设备也有画面）
//!   vpid-viewer-linux --scan     CLI 自检（打印设备列表后退出）
//!
//! 非 Linux 目标编译为空占位（打印错误后退出），保证 Windows `cargo build` 不因本 crate 失败。

#[cfg(target_os = "linux")]
mod app;
#[cfg(target_os = "linux")]
mod provider;

#[cfg(target_os = "linux")]
use std::sync::Arc;
#[cfg(target_os = "linux")]
use vpid_ui_core::app::AppState;
#[cfg(target_os = "linux")]
use vpid_ui_core::platform::{DeviceProvider, Platform};

fn main() {
    #[cfg(target_os = "linux")]
    {
        if try_cli_scan() {
            return;
        }
        let use_demo = std::env::args().any(|a| a == "--demo");
        let provider: Arc<dyn DeviceProvider> = if use_demo {
            Arc::new(provider::StubProvider)
        } else {
            Arc::new(provider::RealProvider::new())
        };
        let result = app::LinuxApp.run(AppState::default(), provider);
        if let Err(e) = result {
            eprintln!("vpid-viewer-linux 启动失败: {e}");
            std::process::exit(1);
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("vpid-viewer-linux 仅支持 Linux（GTK3）目标；当前平台不可运行。");
        std::process::exit(1);
    }
}

/// CLI 自检：`vpid-viewer-linux --scan`（走 vpid-core 扫描链路）。
#[cfg(target_os = "linux")]
fn try_cli_scan() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|a| a == "--scan" || a == "/scan") {
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
