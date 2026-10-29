//! Windows USB 扫描：SetupAPI 主路径 + 注册表兜底。
//!
//! 忠实移植自 `src/core/windows_scanner.cpp`，行为逐条对齐：
//! - 主路径：只枚举 USB Device Interface 类 GUID（几十~几百个设备，而非全系统
//!   上千个），SetupAPI 调用次数降约 5-10× —— 100ms 定时器下这是每帧 CPU
//!   最大头，优化后界面卡顿显著缓解（[perf]）。
//! - 兜底：某些老系统/特殊驱动未注册 device interface（如 USBSTOR 复合设备），
//!   走 `DIGCF_ALLCLASSES` 全枚举；再空则注册表枚举 `Enum\USB`。
//! - 取消：循环顶部检查 `scan_cancel_requested()`，保证单次枚举往返内响应退出。
//! - 去重：按唯一键 `(vid, pid, serial)` 保序去重（对齐 `dedup`）。
//! - 连接判定（注册表路径）：`ConfigManagerErrorCode==0` 且未标记 `ConfigFlags bit2`。

use std::collections::BTreeSet;
use std::ptr::{null, null_mut};

use windows_sys::core::{GUID, PCWSTR};
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
    SetupDiGetDeviceInstanceIdW, SetupDiGetDeviceRegistryPropertyW, SP_DEVINFO_DATA, DIGCF_ALLCLASSES,
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SPDRP_DEVICEDESC, SPDRP_DRIVER,
    SPDRP_FRIENDLYNAME, SPDRP_LOCATION_INFORMATION, SPDRP_MFG,
};
use windows_sys::Win32::Globalization::{WideCharToMultiByte, CP_UTF8};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
    REG_DWORD,
};

use crate::device::UsbDevice;
use crate::scanner::{extract_serial, extract_vid_pid, scan_cancel_requested, Scanner};

/// USB Device Interface Class GUID（对齐 C++ `kGuidUsbDeviceInterface`）。
const GUID_USB_DEVICE_INTERFACE: GUID = GUID {
    data1: 0x58ea4af6,
    data2: 0xfc82,
    data3: 0x4d5b,
    data4: [0xba, 0x1f, 0x29, 0xaf, 0x3f, 0x9a, 0xe5, 0xe],
};

const INVALID_HDEVINFO: isize = -1; // SetupDiGetClassDevsW 失败返回 INVALID_HANDLE_VALUE
const STATUS_CONNECTED: &str = "Connected";

#[derive(Default)]
pub struct WindowsScanner;

impl Scanner for WindowsScanner {
    fn scan(&self) -> Vec<UsbDevice> {
        let mut result = scan_via_setupapi();
        if result.is_empty() && !scan_cancel_requested() {
            result = scan_via_registry();
        }
        result
    }
}

// ---------------- 工具：宽字符 <-> UTF-8 ----------------

/// UTF-16 -> UTF-8（设备名等含非 ASCII，须保真）。对齐 `w2utf8`。
fn w2utf8(w: &[u16]) -> String {
    if w.is_empty() {
        return String::new();
    }
    unsafe {
        let n = WideCharToMultiByte(
            CP_UTF8,
            0,
            w.as_ptr(),
            w.len() as i32,
            null_mut(),
            0,
            null_mut(),
            null_mut(),
        );
        if n <= 0 {
            return String::new();
        }
        let mut out = vec![0u8; n as usize];
        WideCharToMultiByte(
            CP_UTF8,
            0,
            w.as_ptr(),
            w.len() as i32,
            out.as_mut_ptr(),
            n,
            null_mut(),
            null_mut(),
        );
        String::from_utf8_lossy(&out).into_owned()
    }
}

/// Rust `&str` -> UTF-16 向量（含结尾 NUL）。
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---------------- SetupAPI 主路径 ----------------

fn scan_via_setupapi() -> Vec<UsbDevice> {
    unsafe {
        // 主路径：只枚举 USB Device Interface 类
        let devs = SetupDiGetClassDevsW(
            &GUID_USB_DEVICE_INTERFACE,
            null(),
            null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if devs != INVALID_HDEVINFO {
            let (found, cancelled) = scan_from_devinfo(devs);
            if !found.is_empty() {
                return found;
            }
            // 主路径因取消而中止 → 不再跑昂贵的 ALLCLASSES 兜底
            if cancelled {
                return Vec::new();
            }
        }
        if scan_cancel_requested() {
            return Vec::new();
        }
        // [fallback] ALLCLASSES 全枚举
        scan_from_devinfo(SetupDiGetClassDevsW(
            null(),
            null(),
            null_mut(),
            DIGCF_PRESENT | DIGCF_ALLCLASSES,
        ))
        .0
    }
}

/// 从 HDEVINFO 枚举 USB 设备。返回 (设备列表, 是否因取消中止)。
unsafe fn scan_from_devinfo(devs: isize) -> (Vec<UsbDevice>, bool) {
    if devs == INVALID_HDEVINFO {
        return (Vec::new(), false);
    }
    let mut devices: Vec<UsbDevice> = Vec::new();
    let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut cancelled = false;

    for idx in 0u32.. {
        if scan_cancel_requested() {
            cancelled = true;
            break;
        }
        let mut di: SP_DEVINFO_DATA = std::mem::zeroed();
        di.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
        if SetupDiEnumDeviceInfo(devs, idx, &mut di) == 0 {
            break;
        }
        let mut inst_buf = [0u16; 1024];
        if SetupDiGetDeviceInstanceIdW(devs, &di, inst_buf.as_mut_ptr(), 1024, null_mut()) == 0 {
            continue;
        }
        let len = inst_buf.iter().position(|&c| c == 0).unwrap_or(1024);
        let instance_id = w2utf8(&inst_buf[..len]);

        let (vid, pid) = extract_vid_pid(&instance_id);
        if vid.is_empty() || pid.is_empty() {
            continue;
        }
        let serial = extract_serial(&instance_id);
        let key = (vid.clone(), pid.clone(), serial.clone());
        if !seen.insert(key) {
            continue;
        }

        let prop_str = |which: u32| -> String {
            unsafe {
                let mut ty: u32 = 0;
                let mut need: u32 = 0;
                let ok = SetupDiGetDeviceRegistryPropertyW(
                    devs, &di, which, &mut ty, null_mut(), 0, &mut need,
                );
                if ok == 0 && need == 0 {
                    return String::new();
                }
                let mut buf = vec![0u16; (need as usize) / 2 + 4];
                let nbytes = (buf.len() * 2) as u32;
                let ok2 = SetupDiGetDeviceRegistryPropertyW(
                    devs,
                    &di,
                    which,
                    &mut ty,
                    buf.as_mut_ptr() as *mut u8,
                    nbytes,
                    &mut need,
                );
                if ok2 == 0 {
                    return String::new();
                }
                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                w2utf8(&buf[..len])
            }
        };

        let mut name = prop_str(SPDRP_FRIENDLYNAME);
        if name.is_empty() {
            name = prop_str(SPDRP_DEVICEDESC);
        }
        let manufacturer = prop_str(SPDRP_MFG);
        let driver = prop_str(SPDRP_DRIVER);
        let location = prop_str(SPDRP_LOCATION_INFORMATION);

        devices.push(build_device(
            &vid, &pid, &serial, &name, &manufacturer, &location, &driver, &instance_id,
        ));
    }

    SetupDiDestroyDeviceInfoList(devs);
    (dedup(devices), cancelled)
}

// ---------------- 注册表兜底 ----------------

fn scan_via_registry() -> Vec<UsbDevice> {
    unsafe {
        let mut base: HKEY = null_mut();
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            to_wide("SYSTEM\\CurrentControlSet\\Enum\\USB").as_ptr(),
            0,
            KEY_READ,
            &mut base,
        ) != 0
        {
            return Vec::new();
        }
        let mut devices: Vec<UsbDevice> = Vec::new();

        for i in 0u32.. {
            if scan_cancel_requested() {
                break;
            }
            let mut name = [0u16; 256];
            let mut name_len: u32 = 256;
            if RegEnumKeyExW(
                base,
                i,
                name.as_mut_ptr(),
                &mut name_len,
                null(),
                null_mut(),
                null_mut(),
                null_mut(),
            ) != 0
            {
                break;
            }
            let vid_pid_key = w2utf8(&name[..name_len as usize]);
            let (vid, pid) = extract_vid_pid(&vid_pid_key);
            if vid.is_empty() || pid.is_empty() {
                continue;
            }

            let sub_path = format!("SYSTEM\\CurrentControlSet\\Enum\\USB\\{}", vid_pid_key);
            let mut vp_key: HKEY = null_mut();
            if RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                to_wide(&sub_path).as_ptr(),
                0,
                KEY_READ,
                &mut vp_key,
            ) != 0
            {
                continue;
            }

            for j in 0u32.. {
                if scan_cancel_requested() {
                    break;
                }
                let mut inst = [0u16; 256];
                let mut inst_len: u32 = 256;
                if RegEnumKeyExW(
                    vp_key,
                    j,
                    inst.as_mut_ptr(),
                    &mut inst_len,
                    null(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                ) != 0
                {
                    break;
                }
                let serial_part = w2utf8(&inst[..inst_len as usize]);

                let inst_path = format!("{}\\{}", sub_path, serial_part);
                let mut inst_key: HKEY = null_mut();
                if RegOpenKeyExW(
                    HKEY_LOCAL_MACHINE,
                    to_wide(&inst_path).as_ptr(),
                    0,
                    KEY_READ,
                    &mut inst_key,
                ) != 0
                {
                    continue;
                }

                // 连接判定：ConfigManagerErrorCode==0 且未标记 ConfigFlags bit2
                let err_code = reg_dword(inst_key, to_wide("ConfigManagerErrorCode").as_ptr(), 0);
                let config_flags = reg_dword(inst_key, to_wide("ConfigFlags").as_ptr(), 0);
                let connected = err_code == 0 && (config_flags & 0x4) == 0;

                let vid_nox = vid
                    .strip_prefix("0x")
                    .or_else(|| vid.strip_prefix("0X"))
                    .unwrap_or(&vid);
                let pid_nox = pid
                    .strip_prefix("0x")
                    .or_else(|| pid.strip_prefix("0X"))
                    .unwrap_or(&pid);
                let device_id = format!("USB\\VID_{}&PID_{}\\{}", vid_nox, pid_nox, serial_part);

                if connected {
                    let name = {
                        let mut v =
                            clean_registry_string(reg_string(inst_key, to_wide("FriendlyName").as_ptr()));
                        if v.is_empty() {
                            v = clean_registry_string(reg_string(inst_key, to_wide("DeviceDesc").as_ptr()));
                        }
                        v
                    };
                    let manufacturer =
                        clean_registry_string(reg_string(inst_key, to_wide("Mfg").as_ptr()));
                    let driver = reg_string(inst_key, to_wide("Driver").as_ptr());
                    let location = reg_string(inst_key, to_wide("LocationInformation").as_ptr());
                    devices.push(build_device(
                        &vid, &pid, &serial_part, &name, &manufacturer, &location, &driver,
                        &device_id,
                    ));
                }
                RegCloseKey(inst_key);
            }
            RegCloseKey(vp_key);
        }
        RegCloseKey(base);
        dedup(devices)
    }
}

fn build_device(
    vid: &str,
    pid: &str,
    serial: &str,
    name: &str,
    manufacturer: &str,
    location: &str,
    driver: &str,
    device_id: &str,
) -> UsbDevice {
    UsbDevice {
        vid: vid.to_string(),
        pid: pid.to_string(),
        serial: serial.to_string(),
        name: if name.is_empty() {
            "USB Device".to_string()
        } else {
            name.to_string()
        },
        manufacturer: manufacturer.to_string(),
        location: location.to_string(),
        driver: driver.to_string(),
        device_id: device_id.to_string(),
        pnp_device_id: device_id.to_string(),
        status: STATUS_CONNECTED.to_string(),
        path: device_id.to_string(),
    }
}

/// 按唯一键保序去重（对齐 `dedup`）。
fn dedup(inp: Vec<UsbDevice>) -> Vec<UsbDevice> {
    let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
    inp.into_iter()
        .filter(|d| seen.insert(d.key()))
        .collect()
}

/// 取路径最后一段（对齐 `cleanRegistryString`：取最后一个 `\` 之后）。
fn clean_registry_string(v: String) -> String {
    if v.is_empty() {
        return v;
    }
    match v.rfind('\\') {
        Some(pos) => v[pos + 1..].to_string(),
        None => v,
    }
}

unsafe fn reg_dword(key: HKEY, name: PCWSTR, def: i32) -> i32 {
    let mut v: u32 = 0;
    let mut sz: u32 = std::mem::size_of::<u32>() as u32;
    let mut ty: u32 = 0;
    if RegQueryValueExW(key, name, null(), &mut ty, &mut v as *mut u32 as *mut u8, &mut sz) == 0
        && ty == REG_DWORD
    {
        return v as i32;
    }
    def
}

unsafe fn reg_string(key: HKEY, name: PCWSTR) -> String {
    let mut ty: u32 = 0;
    let mut sz: u32 = 0;
    if RegQueryValueExW(key, name, null(), &mut ty, null_mut(), &mut sz) != 0 || sz == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (sz as usize) / 2 + 4];
    let mut ok = sz;
    if RegQueryValueExW(
        key,
        name,
        null(),
        &mut ty,
        buf.as_mut_ptr() as *mut u8,
        &mut ok,
    ) != 0
    {
        return String::new();
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    w2utf8(&buf[..len])
}
