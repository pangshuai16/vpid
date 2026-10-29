//! vpid-core：Windows USB 设备扫描业务核心。
//!
//! 由 C++ `src/core/` 忠实移植，**行为语义逐条对齐**（见各模块注释）：
//! - `device`     设备模型 + 展示/剪贴板格式化（对齐 device_info.cpp）
//! - `scanner`    扫描器抽象 + VID/PID/序列号提取 + 取消标志（对齐 device_scanner.cpp）
//! - `scanner_win` Windows SetupAPI 主路径 + 注册表兜底（对齐 windows_scanner.cpp）
//! - `comparer`   新增/移除 diff（对齐 device_comparer.cpp）
//! - `notifier`   设备插拔通知（RegisterDeviceNotification，对齐 windows_notifier.cpp）
//!
//! 平台约束：当前仅 Windows。未来 Linux/macOS 的扫描/通知实现可各自新增
//! `scanner_linux` / `notifier_linux` 等模块并按平台选择（预留插槽，见 `scanner::Scanner`）。

pub mod comparer;
pub mod device;
pub mod notifier;
pub mod scanner;
pub mod scanner_win;

#[cfg(target_os = "windows")]
pub use scanner::scan_usb_devices;
