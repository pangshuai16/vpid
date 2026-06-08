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

/// USB 设备信息（简化结构，适配 nusb 0.2）
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct UsbDeviceInfo {
    /// 厂商 ID
    pub vendor_id: u16,
    /// 产品 ID
    pub product_id: u16,
    /// USB 设备类
    pub device_class: u8,
    /// 子类
    pub device_subclass: u8,
    /// 协议
    pub device_protocol: u8,
    /// 厂商名称
    pub manufacturer: Option<String>,
    /// 产品名称
    pub product: Option<String>,
    /// 序列号
    pub serial_number: Option<String>,
    /// 设备速度
    pub device_speed: DeviceSpeed,
}
