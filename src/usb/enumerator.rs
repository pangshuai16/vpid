use crate::usb::models::*;
use crate::usb::class_codes;

/// 列出所有 USB 设备
pub async fn list_usb_devices() -> Result<Vec<UsbDeviceInfo>, String> {
    let devices = nusb::list_devices()
        .await
        .map_err(|e| format!("nusb error: {}", e))?;

    Ok(devices
        .into_iter()
        .map(convert_device_info)
        .collect())
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
        // 至少应该能列出设备（可能为空）
        let result = futures_lite::future::block_on(list_usb_devices());
        match result {
            Ok(list) => println!("Found {} devices", list.len()),
            Err(e) => println!("Error listing devices: {}", e),
        }
        // 测试不失败，只是验证函数可调用
    }
}