use serde::Serialize;

/// USB 设备速度
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub enum DeviceSpeed {
    Low,
    Full,
    High,
    Super,
    Unknown,
}

/// USB Descriptors
/// 设备信息结构体，用于序列化到 QML
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
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
