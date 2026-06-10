use serde::{Serialize, Serializer};

/// USB 设备速度
#[derive(Clone, Debug, PartialEq, Eq)]
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
    pub fn display_text(&self) -> &'static str {
        match self {
            DeviceSpeed::Low => "Low (1.5 Mbps)",
            DeviceSpeed::Full => "Full (12 Mbps)",
            DeviceSpeed::High => "High (480 Mbps)",
            DeviceSpeed::Super => "Super (5 Gbps)",
            DeviceSpeed::Unknown => "Unknown",
        }
    }
}

/// 自定义序列化，输出字符串而非 `{"Variant": null}`
impl Serialize for DeviceSpeed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.display_text())
    }
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
