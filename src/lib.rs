//! vpid - USB 设备查看器 (Qt6 + qmetaobject-rs)
//!
//! 使用 qmetaobject-rs 将 Rust QObject 暴露给 QML，
//! 通过 nusb 枚举 USB 设备，在 Qt6 Quick 界面中展示。

pub mod usb;
pub mod ui;
