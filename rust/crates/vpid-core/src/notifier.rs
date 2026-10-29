//! 设备插拔通知（RegisterDeviceNotification）。
//!
//! 忠实移植自 `src/core/device_notifier.h` + `windows_notifier.cpp`。
//! Windows 走宿主窗口消息循环（WM_DEVICECHANGE），notifier 仅负责注册/注销监听，
//! 不持有回调线程；事件派发由 UI 层的 WndProc 完成（对齐旧 MainDlg::OnDeviceChange）。

use std::ffi::c_void;
use std::ptr::null_mut;

use windows_sys::core::GUID;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    RegisterDeviceNotificationW, UnregisterDeviceNotification, DEV_BROADCAST_DEVICEINTERFACE_W,
    DEVICE_NOTIFY_WINDOW_HANDLE, DBT_DEVTYP_DEVICEINTERFACE, HDEVNOTIFY,
};

/// 设备接口类 GUID（对齐 C++ `dbcc_classguid`）。
const GUID_DEVINTERFACE: GUID = GUID {
    data1: 0xA5DCBF10,
    data2: 0x6530,
    data3: 0x11D2,
    data4: [0x90, 0x1F, 0x00, 0xC0, 0x4F, 0xB9, 0x51, 0xED],
};

/// Windows 设备插拔监听（宿主窗口消息循环驱动）。
pub struct WindowsNotifier {
    hwnd: *mut c_void,
    notify: HDEVNOTIFY,
}

impl WindowsNotifier {
    pub fn new() -> Self {
        Self {
            hwnd: null_mut(),
            notify: null_mut(),
        }
    }

    /// 启动监听。`hwnd` 为宿主窗口句柄；返回 false 表示失败（调用方应退化为定时轮询）。
    pub fn start(&mut self, hwnd: *mut c_void) -> bool {
        if hwnd.is_null() || !self.notify.is_null() {
            return false;
        }
        self.hwnd = hwnd;
        let mut ifc: DEV_BROADCAST_DEVICEINTERFACE_W = unsafe { std::mem::zeroed() };
        ifc.dbcc_size = std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32;
        ifc.dbcc_devicetype = DBT_DEVTYP_DEVICEINTERFACE;
        ifc.dbcc_classguid = GUID_DEVINTERFACE;
        unsafe {
            self.notify = RegisterDeviceNotificationW(
                self.hwnd,
                &ifc as *const DEV_BROADCAST_DEVICEINTERFACE_W as *const c_void,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            );
        }
        !self.notify.is_null()
    }

    pub fn stop(&mut self) {
        if !self.notify.is_null() {
            unsafe {
                UnregisterDeviceNotification(self.notify);
            }
            self.notify = null_mut();
        }
        self.hwnd = null_mut();
    }
}

impl Default for WindowsNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WindowsNotifier {
    fn drop(&mut self) {
        self.stop();
    }
}
