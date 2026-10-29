//! 设备模型（忠实移植自 `src/core/device_info.h` / `device_info.cpp`）。

/// USB 设备信息（字段与 C++ `USBDevice` 一一对应）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsbDevice {
    pub vid: String,
    pub pid: String,
    pub serial: String,
    pub name: String,
    pub manufacturer: String,
    pub location: String,
    pub driver: String,
    pub device_id: String,
    pub pnp_device_id: String,
    pub status: String,
    pub path: String,
}

impl UsbDevice {
    /// 唯一键 `(vid, pid, serial)`，用于比对/去重/哈希。
    /// 对齐 C++ `DeviceKey`（std::set 按 vid→pid→serial 字典序）。
    pub fn key(&self) -> (String, String, String) {
        (self.vid.clone(), self.pid.clone(), self.serial.clone())
    }

    /// 显示名称：name -> "manufacturer Device" -> "Unknown USB Device"。
    pub fn display_name(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        if !self.manufacturer.is_empty() {
            return format!("{} Device", self.manufacturer);
        }
        "Unknown USB Device".to_string()
    }

    /// 去 `0x/0X` 前缀并转大写（如 "0x8087" -> "8087"）。
    fn strip0x(s: &str) -> String {
        let mut out = s.to_string();
        if out.len() >= 2
            && out.starts_with('0')
            && (out.as_bytes()[1] == b'x' || out.as_bytes()[1] == b'X')
        {
            out = out[2..].to_string();
        }
        out.to_uppercase()
    }

    pub fn formatted_vid(&self) -> String {
        if self.vid.is_empty() {
            "N/A".to_string()
        } else {
            Self::strip0x(&self.vid)
        }
    }

    pub fn formatted_pid(&self) -> String {
        if self.pid.is_empty() {
            "N/A".to_string()
        } else {
            Self::strip0x(&self.pid)
        }
    }

    /// "VID:PID"。
    pub fn vid_pid_string(&self) -> String {
        format!("{}:{}", self.formatted_vid(), self.formatted_pid())
    }

    /// 剪贴板多行文本（字段顺序/缺省文案与 C++ 逐字节一致）。
    pub fn clipboard_text(&self) -> String {
        let or_n = |v: &str| -> String {
            if v.is_empty() {
                "N/A".to_string()
            } else {
                v.to_string()
            }
        };
        let n = if self.name.is_empty() {
            "未知设备".to_string()
        } else {
            self.name.clone()
        };
        format!(
            "名称: {}\nVID: {}\nPID: {}\n序列号: {}\n制造商: {}\n位置: {}\n路径: {}\n驱动: {}\n状态: {}",
            n,
            self.formatted_vid(),
            self.formatted_pid(),
            or_n(&self.serial),
            or_n(&self.manufacturer),
            or_n(&self.location),
            or_n(&self.path),
            or_n(&self.driver),
            or_n(&self.status)
        )
    }
}
