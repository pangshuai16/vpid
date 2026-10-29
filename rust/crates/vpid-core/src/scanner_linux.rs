//! Linux USB 扫描：sysfs 枚举（纯 `std::fs`，无 libudev 依赖）。
//!
//! 遍历 `/sys/bus/usb/devices/`：
//! - 跳过接口项（目录名含 `:`，如 `1-1:1.0`）与无 `idVendor` 的项；
//! - 读取 `idVendor/idProduct/serial/product/manufacturer/driver/devpath`；
//! - `vid/pid` 统一为 `0xXXXX` 大写（对齐 Windows `extract_hex` 输出）；
//! - `location = devpath`；`driver` = `driver` 符号链接名；`status = "Connected"`；
//! - `path/device_id/pnp_device_id` = sysfs 设备名（如 `1-1`）。
//!
//! 取消：循环顶部检查 `scanner::scan_cancel_requested()`（对齐 Win32 取消语义）。
//! 热插拔靠上层 100ms 轮询（本扫描器只做一次性枚举）。

use std::fs;
use std::path::{Path, PathBuf};

use crate::device::UsbDevice;
use crate::scanner::{scan_cancel_requested, Scanner};

const SYSFS_USB: &str = "/sys/bus/usb/devices";

/// Linux 平台扫描器。
pub struct LinuxScanner;

impl Scanner for LinuxScanner {
    fn scan(&self) -> Vec<UsbDevice> {
        scan_root(Path::new(SYSFS_USB))
    }
}

/// 枚举一个 sysfs 设备根目录（默认 `/sys/bus/usb/devices`；测试可传临时目录）。
fn scan_root(root: &Path) -> Vec<UsbDevice> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    // 稳定顺序（read_dir 不保证顺序；排序后测试可预测）
    entries.sort();

    for dir in entries {
        if scan_cancel_requested() {
            break;
        }
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // 接口项（目录名含 `:`）与缺 idVendor 项跳过
        if name.is_empty() || name.contains(':') || !dir.join("idVendor").exists() {
            continue;
        }
        let vid = read_hex(&dir, "idVendor");
        if vid.is_empty() {
            continue;
        }
        let pid = read_hex(&dir, "idProduct");
        let mut d = UsbDevice {
            vid,
            pid,
            serial: read_str(&dir, "serial"),
            name: read_str(&dir, "product"),
            manufacturer: read_str(&dir, "manufacturer"),
            location: read_str(&dir, "devpath"),
            driver: read_driver(&dir),
            device_id: name.to_string(),
            pnp_device_id: name.to_string(),
            status: "Connected".to_string(),
            path: name.to_string(),
        };
        // 缺 idProduct 用空串（对齐 Windows 空 PID 语义），但保留条目
        if d.location.is_empty() {
            d.location = name.to_string();
        }
        out.push(d);
    }
    out
}

fn read_str(dir: &Path, field: &str) -> String {
    fs::read_to_string(dir.join(field))
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// 读 16 进制字段并规范为 `0xXXXX` 大写（如 "046d" -> "0x046D"）。
fn read_hex(dir: &Path, field: &str) -> String {
    let raw = read_str(dir, field);
    if raw.is_empty() {
        return String::new();
    }
    let hex = raw.trim_start_matches("0x").trim_start_matches("0X");
    if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return String::new();
    }
    format!("0x{}", hex.to_uppercase())
}

/// 取 `driver` 符号链接的末段名（如 "../../bus/usb/drivers/usb" -> "usb"）。
fn read_driver(dir: &Path) -> String {
    fs::read_link(dir.join("driver"))
        .ok()
        .and_then(|p| p.file_name().and_then(|n| n.to_str()).map(|s| s.to_string()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 伪造临时 sysfs 布局，返回根目录（Drop 时清理）。
    struct FakeSys {
        root: PathBuf,
    }
    static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    impl FakeSys {
        fn new() -> Self {
            let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!("vpid-sysfs-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn dev(&self, name: &str) -> PathBuf {
            let d = self.root.join(name);
            fs::create_dir_all(&d).unwrap();
            d
        }
    }

    impl Drop for FakeSys {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn write(dir: &Path, field: &str, val: &str) {
        fs::write(dir.join(field), val).unwrap();
    }

    #[test]
    fn maps_fields_and_filters_interfaces() {
        let fsys = FakeSys::new();
        let d = fsys.dev("1-1");
        write(&d, "idVendor", "046d");
        write(&d, "idProduct", "c534");
        write(&d, "serial", "A1B2C3D4");
        write(&d, "product", "USB Receiver");
        write(&d, "manufacturer", "Logitech");
        write(&d, "devpath", "1-1");
        std::os::unix::fs::symlink("../../../bus/usb/drivers/usb", d.join("driver")).unwrap();

        // 接口项（名字含 `:`）与缺 idVendor 项必须被过滤
        let iface = fsys.dev("1-1:1.0");
        write(&iface, "idVendor", "046d");
        fsys.dev("1-2");

        let devs = scan_root(&fsys.root);
        assert_eq!(devs.len(), 1);
        let got = &devs[0];
        assert_eq!(got.vid, "0x046D");
        assert_eq!(got.pid, "0xC534");
        assert_eq!(got.serial, "A1B2C3D4");
        assert_eq!(got.name, "USB Receiver");
        assert_eq!(got.manufacturer, "Logitech");
        assert_eq!(got.location, "1-1");
        assert_eq!(got.driver, "usb");
        assert_eq!(got.status, "Connected");
        assert_eq!(got.path, "1-1");
        assert_eq!(got.device_id, "1-1");
        assert_eq!(got.pnp_device_id, "1-1");
    }

    #[test]
    fn missing_serial_and_product_default_empty() {
        let fsys = FakeSys::new();
        let d = fsys.dev("2-1");
        write(&d, "idVendor", "1d6b");
        write(&d, "idProduct", "0002");

        let devs = scan_root(&fsys.root);
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].serial, "");
        assert_eq!(devs[0].name, "");
        assert_eq!(devs[0].display_name(), "Unknown USB Device");
    }

    #[test]
    fn empty_root_returns_empty() {
        let fsys = FakeSys::new();
        assert!(scan_root(&fsys.root).is_empty());
    }
}
