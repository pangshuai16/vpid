//! vpid — USB设备查看器 (Qt5.15 + qmetaobject-rs)
//!
//! 使用 qmetaobject-rs 将 Rust QObject 绑定到 QML，
//! 通过 nusb 枚举 USB 设备，在 Qt5 Quick 界面中展示。

/// USB 模块 — 设备枚举、数据模型、热插拔
pub mod usb;

/// UI 模块 — qmetaobject QObject 桥接
pub mod ui;
