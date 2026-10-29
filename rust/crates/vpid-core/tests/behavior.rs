//! 行为等价性测试：验证 `vpid-core` 的 Rust 移植与 C++ `src/core` 语义逐条一致。
//!
//! 这些用例对齐旧版单测/预期行为，作为"保持现有功能与交互行为不变"的机器门禁。

use vpid_core::comparer::{diff, has_changed};
use vpid_core::device::UsbDevice;
use vpid_core::scanner::{extract_serial, extract_vid_pid};

fn dev(vid: &str, pid: &str, serial: &str, name: &str) -> UsbDevice {
    UsbDevice {
        vid: format!("0x{vid}"),
        pid: format!("0x{pid}"),
        serial: serial.to_string(),
        name: name.to_string(),
        manufacturer: String::new(),
        location: String::new(),
        driver: String::new(),
        device_id: String::new(),
        pnp_device_id: String::new(),
        status: "Connected".to_string(),
        path: String::new(),
    }
}

#[test]
fn extract_vid_pid_basic() {
    assert_eq!(
        extract_vid_pid("USB\\VID_8087&PID_0024\\A1B2"),
        ("0x8087".to_string(), "0x0024".to_string())
    );
}

#[test]
fn extract_vid_pid_lowercase_uppercased() {
    assert_eq!(
        extract_vid_pid("usb\\vid_1a2b&pid_c3d4\\X"),
        ("0x1A2B".to_string(), "0xC3D4".to_string())
    );
}

#[test]
fn extract_vid_pid_non_hex_rejected() {
    assert_eq!(extract_vid_pid("USB\\VID_ZZZZ\\X"), ("".to_string(), "".to_string()));
}

#[test]
fn extract_serial_third_segment() {
    assert_eq!(extract_serial("USB\\VID_8087&PID_0024\\1234"), "1234");
    assert_eq!(extract_serial("no-backslash"), "");
}

#[test]
fn diff_added_removed_changed() {
    let a = dev("8087", "0024", "S1", "A");
    let b = dev("046d", "c534", "S2", "B");
    let c = dev("0951", "1666", "S3", "C");
    let d = diff(&[a.clone(), b.clone()], &[b.clone(), c.clone()]);
    assert_eq!(d.added, vec![c.clone()]);
    assert_eq!(d.removed, vec![a.clone()]);
    assert!(d.changed);
}

#[test]
fn diff_no_change() {
    let a = dev("8087", "0024", "S1", "A");
    let b = dev("046d", "c534", "S2", "B");
    let d = diff(&[a.clone(), b.clone()], &[a.clone(), b.clone()]);
    assert!(d.added.is_empty());
    assert!(d.removed.is_empty());
    assert!(!d.changed);
}

#[test]
fn has_changed_semantics() {
    let a = dev("8087", "0024", "S1", "A");
    let b = dev("046d", "c534", "S2", "B");
    assert!(has_changed(&[a.clone()], &[a.clone(), b.clone()]));
    assert!(!has_changed(&[a.clone()], &[a.clone()]));
}

#[test]
fn display_name_fallbacks() {
    // name 优先
    assert_eq!(dev("1", "2", "3", "Real").display_name(), "Real");
    // name 空 → "manufacturer Device"
    let d = UsbDevice {
        manufacturer: "Intel".to_string(),
        ..dev("1", "2", "3", "")
    };
    assert_eq!(d.display_name(), "Intel Device");
    // 都空 → Unknown
    assert_eq!(dev("1", "2", "3", "").display_name(), "Unknown USB Device");
}

#[test]
fn formatted_vid_pid_strip_0x() {
    let d = dev("0a1b", "c2d3", "S", "N");
    assert_eq!(d.formatted_vid(), "0A1B");
    assert_eq!(d.formatted_pid(), "C2D3");
}

#[test]
fn clipboard_text_format() {
    let d = UsbDevice {
        vid: "0x8087".to_string(),
        pid: "0x0024".to_string(),
        serial: "S1".to_string(),
        name: String::new(), // 空名 → 未知设备
        ..Default::default()
    };
    let t = d.clipboard_text();
    assert!(t.starts_with("名称: 未知设备\nVID: 8087\nPID: 0024\n序列号: S1"));
    assert!(t.contains("制造商: N/A"));
}
