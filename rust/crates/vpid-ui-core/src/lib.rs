//! vpid-ui-core：平台无关 UI 核心。
//!
//! 只包含 UI 状态机、视图模型、主题常量与平台抽象（`Platform` trait），
//! 不依赖任何窗口系统/平台 API —— 这是"多平台 UI 兼容插槽"的关键：
//! 所有平台后端（win32 / x11 / cocoa）共享本 crate 的状态机、视图模型与主题，
//! 仅各自实现渲染与输入（见 `platform::Platform`）。

pub mod app;
pub mod command;
pub mod model;
pub mod platform;
pub mod theme;
