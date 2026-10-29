//! vpid-core：USB 设备扫描业务核心。
//!
//! 从 C++ `src/core/` 忠实移植，**行为语义逐条对齐**（见各模块注释）：
//! - `device`     设备模型 + 展示/剪贴板格式化（对齐 device_info.cpp）
//! - `scanner`    扫描器抽象 + VID/PID/序列号提取 + 取消标志（对齐 device_scanner.cpp）
//! - `scanner_win` Windows SetupAPI 主路径 + 注册表兜底（对齐 windows_scanner.cpp）
//! - `scanner_linux` Linux sysfs 枚举（无 libudev 依赖，纯 std::fs）
//! - `comparer`   新增/移除 diff（对齐 device_comparer.cpp）
//! - `notifier`   设备插拔通知（RegisterDeviceNotification，对齐 windows_notifier.cpp）
//!
//! 平台约束：Windows 走 `scanner_win` + `notifier`；Linux 走 `scanner_linux`
//! （热插拔由上层轮询）。macOS 未来各自新增 `scanner_macos` 等模块并按平台选择。

pub mod comparer;
pub mod device;
#[cfg(target_os = "windows")]
pub mod notifier;
pub mod scanner;
#[cfg(target_os = "windows")]
pub mod scanner_win;
#[cfg(target_os = "linux")]
pub mod scanner_linux;

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub use scanner::scan_usb_devices;
