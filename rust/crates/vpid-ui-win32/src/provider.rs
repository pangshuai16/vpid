//! 设备数据源：生产 = `vpid-core` 扫描器；演示桩 = 样例数据（`--demo`）。

use vpid_core::device::UsbDevice;
use vpid_core::scanner;
use vpid_ui_core::platform::DeviceProvider;

/// 生产数据源：调用 `vpid-core` 的 Windows USB 扫描。
pub struct RealProvider;

impl RealProvider {
    pub fn new() -> Self {
        Self
    }
}

impl DeviceProvider for RealProvider {
    fn scan(&self) -> Vec<UsbDevice> {
        scanner::scan_usb_devices()
    }
    fn request_cancel(&self) {
        scanner::request_scan_cancel();
    }
}

/// 演示桩数据源：返回固定样例（用于 `--demo` 验证渲染/交互，无真实设备时也有画面）。
pub struct StubProvider;

impl DeviceProvider for StubProvider {
    fn scan(&self) -> Vec<UsbDevice> {
        demo_devices()
    }
    fn request_cancel(&self) {}
}

fn dev(
    vid: &str,
    pid: &str,
    serial: &str,
    name: &str,
    manufacturer: &str,
) -> UsbDevice {
    let vid = format!("0x{vid}");
    let pid = format!("0x{pid}");
    let device_id = format!(
        "USB\\VID_{}&PID_{}\\{}",
        &vid[2..],
        &pid[2..],
        serial
    );
    UsbDevice {
        vid,
        pid,
        serial: serial.to_string(),
        name: name.to_string(),
        manufacturer: manufacturer.to_string(),
        location: "Port_#1.Hub_#1".to_string(),
        driver: "usbhub".to_string(),
        device_id: device_id.clone(),
        pnp_device_id: device_id.clone(),
        status: "Connected".to_string(),
        path: device_id,
    }
}

/// 样例设备列表（覆盖新增/移除场景便于验证双通道渲染）。
fn demo_devices() -> Vec<UsbDevice> {
    vec![
        dev("8087", "0024", "0001", "Intel(R) USB 3.0 eXtensible Host Controller", "Intel"),
        dev("046d", "c534", "A1B2C3D4", "USB Receiver", "Logitech"),
        dev("0951", "1666", "F00123", "Kingston DataTraveler 3.0", "Kingston"),
        dev("0bda", "2838", "5E4F3A21", "Realtek USB 3.0 Hub", "Realtek"),
        dev("04f2", "b6d8", "CAM-2024", "USB Camera", "Chicony"),
        dev("1d6b", "0002", "ROOT-HUB", "USB Root Hub", "Linux Foundation"),
    ]
}
