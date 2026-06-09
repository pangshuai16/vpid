use std::sync::mpsc::Sender;
use std::thread;
use crate::usb::models::{self, UsbDeviceInfo};

/// 热插拔事件类型
pub enum HotplugEvent {
    /// 设备连接
    Connected(UsbDeviceInfo),
    /// 设备断开（可能缺少详细信息）
    Disconnected { vendor_id: u16, product_id: u16 },
    /// 设备变化（通用通知，nusb 某些平台只发送这个）
    Change,
}

/// 热插拔监听器
pub struct HotplugWatcher {
    _thread: thread::JoinHandle<()>,
}

impl HotplugWatcher {
    /// 创建新的热插拔监听器
    pub fn new(tx: Sender<HotplugEvent>) -> Self {
        let _thread = thread::spawn(move || {
            if let Ok(stream) = nusb::watch_devices() {
                for event in futures_lite::stream::block_on(stream) {
                    match event {
                        nusb::hotplug::HotplugEvent::Connected(info) => {
                            let device_info = UsbDeviceInfo {
                                vendor_id: info.vendor_id(),
                                product_id: info.product_id(),
                                device_class: info.class(),
                                device_subclass: info.subclass(),
                                device_protocol: info.protocol(),
                                device_speed: match info.speed() {
                                    Some(nusb::Speed::Low) => models::DeviceSpeed::Low,
                                    Some(nusb::Speed::Full) => models::DeviceSpeed::Full,
                                    Some(nusb::Speed::High) => models::DeviceSpeed::High,
                                    Some(nusb::Speed::Super) => models::DeviceSpeed::Super,
                                    _ => models::DeviceSpeed::Unknown,
                                },
                                manufacturer: info.manufacturer_string().map(|s| s.to_string()),
                                product: info.product_string().map(|s| s.to_string()),
                                serial_number: info.serial_number().map(|s| s.to_string()),
                                device_class_name: crate::usb::class_codes::usb_class_name(info.class()).to_string(),
                            };
                            let _ = tx.send(HotplugEvent::Connected(device_info));
                        }
                        nusb::hotplug::HotplugEvent::Disconnected(_device_id) => {
                            // nusb 的 Disconnected 可能带或不带详细信息
                            // DeviceId 没有 vendor_id/product_id 方法，发送通用信号
                            let _ = tx.send(HotplugEvent::Change);
                        }
                    }
                }
            } else {
                // 热插播 API 不可用，发送初始 Change 信号兜底
                let _ = tx.send(HotplugEvent::Change);
            }
        });

        HotplugWatcher { _thread }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // 需要实际 USB 设备
    fn test_hotplug_watcher() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let _watcher = HotplugWatcher::new(tx);
        
        // 等待一小段时间看是否有事件
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}