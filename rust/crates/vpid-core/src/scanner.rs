//! 扫描器抽象 + VID/PID/序列号提取 + 取消标志。
//!
//! 忠实移植自 `src/core/device_scanner.h` / `device_scanner.cpp`。
//! 取消标志为全局原子量（与 C++ `std::atomic<bool> g_scanCancel` 语义一致）：
//! 同一时刻至多一个扫描线程（UI 层 `scanning` 互斥），故全局量即可；
//! 静态存储期 → 生命周期长于任何窗口实例，无线程持它导致的 UAF（这正是
//! Rust 移植想根治的旧版"跨线程悬垂"风险类别）。

use std::sync::atomic::{AtomicBool, Ordering};

use crate::device::UsbDevice;

static G_SCAN_CANCEL: AtomicBool = AtomicBool::new(false);

/// 退出时调用：让在途扫描尽快自行返回（Windows 的 ExitProcess 会等所有线程，
/// 旧版因此必须显式取消，否则进程退不出）。
pub fn request_scan_cancel() {
    G_SCAN_CANCEL.store(true, Ordering::Relaxed);
}

pub fn scan_cancel_requested() -> bool {
    G_SCAN_CANCEL.load(Ordering::Relaxed)
}

/// 每轮新扫描开始前复位（对齐 `scanUsbDevices()` 首行）。
pub fn reset_scan_cancel() {
    G_SCAN_CANCEL.store(false, Ordering::Relaxed);
}

/// 从设备 ID 提取 VID/PID："USB\VID_8087&PID_0024\.." -> ("0x8087", "0x0024")。
/// 对齐 C++ `Scanner::extractVidPid`（正则 `VID_([0-9A-Fa-f]{4})` icase，取第一个匹配）。
pub fn extract_vid_pid(device_id: &str) -> (String, String) {
    (extract_hex(device_id, "VID_"), extract_hex(device_id, "PID_"))
}

/// 提取 `PREFIX` 后恰好 4 位十六进制（大小写不敏感，取首个匹配）。
fn extract_hex(s: &str, prefix: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let pl = prefix.to_ascii_lowercase();
    let mut search = 0usize;
    while search < lower.len() {
        match lower[search..].find(&pl) {
            Some(rel) => {
                let pos = search + rel;
                let start = pos + pl.len();
                if start + 4 <= s.len() {
                    let hex = &s[start..start + 4];
                    if hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return format!("0x{}", hex.to_uppercase());
                    }
                }
                search = pos + pl.len();
            }
            None => break,
        }
    }
    String::new()
}

/// 提取反斜杠分隔的第三段作为序列号（对齐 `extractSerialFromDeviceId`）。
pub fn extract_serial(device_id: &str) -> String {
    if device_id.is_empty() {
        return String::new();
    }
    device_id
        .split('\\')
        .nth(2)
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// 扫描器抽象基类（对齐 C++ `Scanner`）。
/// 未来 Linux/macOS 各自提供实现（`scanner_linux` / `scanner_macos`），
/// 通过 `new_scanner()` 按平台分派 —— 多平台扫描插槽。
pub trait Scanner {
    fn scan(&self) -> Vec<UsbDevice>;
}

/// 平台工厂：当前 Windows（`WindowsScanner`）。
pub fn new_scanner() -> Box<dyn Scanner> {
    #[cfg(target_os = "windows")]
    {
        Box::new(crate::scanner_win::WindowsScanner::default())
    }
    #[cfg(not(target_os = "windows"))]
    {
        // 非 Windows：平台扫描未实现（预留插槽）
        Box::new(UnsupportedScanner)
    }
}

/// 非 Windows 占位扫描器（预留插槽，返回空）。
#[cfg(not(target_os = "windows"))]
struct UnsupportedScanner;
#[cfg(not(target_os = "windows"))]
impl Scanner for UnsupportedScanner {
    fn scan(&self) -> Vec<UsbDevice> {
        Vec::new()
    }
}

/// 统一入口：复位取消标志 → 扫描（对齐 `scanUsbDevices()`）。
pub fn scan_usb_devices() -> Vec<UsbDevice> {
    reset_scan_cancel();
    new_scanner().scan()
}
