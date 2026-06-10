use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use serde::{Serialize, Serializer};

/// USB 设备速度
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceSpeed {
    /// Low (1.5 Mbps)
    Low,
    /// Full (12 Mbps)
    Full,
    /// High (480 Mbps)
    High,
    /// Super (5 Gbps)
    Super,
    /// Unknown speed
    Unknown,
}

impl DeviceSpeed {
    /// 返回人类可读的速度描述
    pub fn display_text(self) -> &'static str {
        match self {
            DeviceSpeed::Low => "Low (1.5 Mbps)",
            DeviceSpeed::Full => "Full (12 Mbps)",
            DeviceSpeed::High => "High (480 Mbps)",
            DeviceSpeed::Super => "Super (5 Gbps)",
            DeviceSpeed::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for DeviceSpeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_text())
    }
}

/// 自定义序列化，输出字符串而非 `{"Variant": null}`
impl Serialize for DeviceSpeed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.display_text())
    }
}

/// 设备信息唯一标识 (VID:PID + serial)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeviceKey {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
}

impl DeviceKey {
    pub fn from_device(info: &UsbDeviceInfo) -> Self {
        DeviceKey {
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            serial_number: info.serial_number.clone(),
        }
    }
}

/// USB 设备信息，用于序列化到 QML
#[derive(Serialize, Clone, Debug)]
pub struct UsbDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub device_speed: DeviceSpeed,
    /// 设备类别名称（从 class_codes 映射）
    pub device_class_name: String,
}

impl PartialEq for UsbDeviceInfo {
    fn eq(&self, other: &Self) -> bool {
        self.vendor_id == other.vendor_id
            && self.product_id == other.product_id
            && self.serial_number == other.serial_number
    }
}

impl Eq for UsbDeviceInfo {}

impl Hash for UsbDeviceInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.vendor_id.hash(state);
        self.product_id.hash(state);
        self.serial_number.hash(state);
    }
}

impl UsbDeviceInfo {
    /// 获取设备的唯一标识
    pub fn key(&self) -> DeviceKey {
        DeviceKey::from_device(self)
    }

    /// 返回用户可读的短描述
    pub fn short_description(&self) -> String {
        let product = self.product.as_deref().unwrap_or("Unknown Device");
        format!(
            "{product} [{:04x}:{:04x}]",
            self.vendor_id, self.product_id
        )
    }
}

/// 解析 VID:PID 字符串（如 "8087:0026"）
impl FromStr for DeviceKey {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err("expected VID:PID format");
        }
        let vendor_id = u16::from_str_radix(parts[0], 16).map_err(|_| "invalid VID")?;
        let product_id = u16::from_str_radix(parts[1], 16).map_err(|_| "invalid PID")?;
        Ok(DeviceKey {
            vendor_id,
            product_id,
            serial_number: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_equality() {
        let d1 = UsbDeviceInfo {
            vendor_id: 0x8087,
            product_id: 0x0026,
            device_class: 0x09,
            device_subclass: 0x00,
            device_protocol: 0x00,
            manufacturer: None,
            product: None,
            serial_number: None,
            device_speed: DeviceSpeed::High,
            device_class_name: "Hub".to_string(),
        };
        let d2 = d1.clone();
        assert_eq!(d1, d2);

        let mut set = std::collections::HashSet::new();
        set.insert(d1);
        assert!(set.contains(&d2));
    }

    #[test]
    fn test_device_key_from_str() {
        let key: DeviceKey = "8087:0026".parse().unwrap();
        assert_eq!(key.vendor_id, 0x8087);
        assert_eq!(key.product_id, 0x0026);
    }

    #[test]
    fn test_short_description() {
        let d = UsbDeviceInfo {
            vendor_id: 0x8087,
            product_id: 0x0026,
            device_class: 0,
            device_subclass: 0,
            device_protocol: 0,
            manufacturer: None,
            product: Some("USB Hub".into()),
            serial_number: None,
            device_speed: DeviceSpeed::High,
            device_class_name: "Hub".to_string(),
        };
        assert_eq!(d.short_description(), "USB Hub [8087:0026]");
    }
}
