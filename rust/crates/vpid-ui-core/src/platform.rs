//! 多平台 UI 兼容插槽：平台无关的 UI 入口抽象。
//!
//! 当前仅 Win32 实现；Linux(X11/Wayland) / macOS(Cocoa) 未来各自实现本 trait，
//! 由 `vpid-ui-core` 的状态机 + 视图模型 + 主题驱动，渲染/输入由各后端负责。
//! 详见 `rust/docs/PLATFORM_EXTENSION.md`。

use std::sync::Arc;

use crate::app::AppState;

/// 平台运行错误。
pub type PlatformError = String;

/// 设备数据源（生产 = `vpid-core` 扫描器；测试/演示 = 桩数据）。
///
/// 所有调用发生在扫描工作线程；实现必须 `Send + Sync`。
pub trait DeviceProvider: Send + Sync {
    /// 执行一次完整扫描（阻塞；可被取消）。
    fn scan(&self) -> Vec<vpid_core::device::UsbDevice>;
    /// 请求取消在途扫描（退出用）。
    fn request_cancel(&self);
}

/// 平台抽象：一个平台后端 = 创建窗口 + 跑消息循环。
///
/// 实现方职责：
/// 1. 把用户输入翻译成 [`crate::command::Command`]；
/// 2. 把 `AppState::apply_scan` 产出的 [`crate::app::ViewUpdate`] 渲染到屏幕；
/// 3. 后台扫描线程用 `provider` 扫描 + `vpid_core::comparer::diff` 比对，
///    结果经平台消息回投 UI 线程（不得在扫描线程直接改 UI）。
pub trait Platform {
    /// 启动 UI。`state` 为初始状态，`provider` 为设备数据源。
    fn run(self, state: AppState, provider: Arc<dyn DeviceProvider>) -> Result<(), PlatformError>;
}
