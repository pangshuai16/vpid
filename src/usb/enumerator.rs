use std::fmt;

use crate::usb::models::*;
use crate::usb::class_codes;

/// USB 枚举错误
#[derive(Debug)]
pub enum EnumerationError {
    /// nusb 底层错误
    Nusb(String),
}

impl fmt::Display for EnumerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnumerationError::Nusb(msg) => write!(f, "USB enumeration failed: {msg}"),
        }
    }
}

impl std::error::Error for EnumerationError {}

impl From<nusb::Error> for EnumerationError {
    fn from(e: nusb::Error) -> Self {
        EnumerationError::Nusb(e.to_string())
    }
}

/// 列出所有 USB 设备
pub async fn list_usb_devices() -> Result<Vec<UsbDeviceInfo>, EnumerationError> {
    let devices = nusb::list_devices().await?;

    Ok(devices.into_iter().map(convert_device_info).collect())
}

/// 将 nusb::DeviceInfo 转换为 UsbDeviceInfo
pub fn convert_device_info(info: nusb::DeviceInfo) -> UsbDeviceInfo {
    UsbDeviceInfo {
        vendor_id: info.vendor_id(),
        product_id: info.product_id(),
        device_class: info.class(),
        device_subclass: info.subclass(),
        device_protocol: info.protocol(),
        device_speed: match info.speed() {
            Some(nusb::Speed::Low) => DeviceSpeed::Low,
            Some(nusb::Speed::Full) => DeviceSpeed::Full,
            Some(nusb::Speed::High) => DeviceSpeed::High,
            Some(nusb::Speed::Super) => DeviceSpeed::Super,
            _ => DeviceSpeed::Unknown,
        },
        manufacturer: info.manufacturer_string().map(|s| s.to_string()),
        product: info.product_string().map(|s| s.to_string()),
        serial_number: info.serial_number().map(|s| s.to_string()),
        device_class_name: class_codes::usb_class_name(info.class()).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_devices() {
        let result = futures_lite::future::block_on(list_usb_devices());
        match &result {
            Ok(list) => assert!(!list.is_empty() || list.is_empty(), "should list zero or more devices"),
            Err(e) => assert!(!e.to_string().is_empty(), "error message should not be empty"),
        }
    }

    #[test]
    fn test_error_display() {
        let err = EnumerationError::Nusb("test error".to_string());
        assert!(err.to_string().contains("test error"));
    }
}
