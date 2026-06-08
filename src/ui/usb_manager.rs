use std::sync::{LazyLock, Mutex};

use qmetaobject::*;

use crate::usb::{enumerator, models::UsbDeviceInfo};

/// 全局 USB 设备状态
///
/// qmetaobject 的 #[derive(QObject)] 要求所有字段用 qt_* 宏标注，
/// 因此将内部状态存放在全局 static 中，QObject 只做方法桥接。
struct DeviceState {
    devices: Vec<UsbDeviceInfo>,
    baseline: Vec<UsbDeviceInfo>,
}

static STATE: LazyLock<Mutex<DeviceState>> = LazyLock::new(|| {
    Mutex::new(DeviceState {
        devices: Vec::new(),
        baseline: Vec::new(),
    })
});

/// USB 管理器 — qmetaobject QObject 桥接
///
/// QML 中通过 `import app 1.0` 引入，实例化方式：
/// ```qml
/// UsbManager { id: usbManager }
/// ```
/// 方法均为同步阻塞调用（USB 枚举毫秒级，对 UX 无感）。
#[derive(Default, QObject)]
pub struct UsbManager {
    base: qt_base_class!(trait QObject),

    /// 设备数据变更信号（QML 可连接 onDevicesChanged）
    devices_changed: qt_signal!(),

    /// 刷新设备列表
    refresh: qt_method!(fn(&mut self)),
    /// 将当前列表设为基准
    set_baseline: qt_method!(fn(&mut self)),
    /// 获取当前设备 JSON 字符串
    get_devices_json: qt_method!(fn(&self) -> QString),
    /// 获取新增设备（相对于基准）JSON 字符串
    get_added_devices_json: qt_method!(fn(&self) -> QString),
    /// 获取移除设备（相对于基准）JSON 字符串
    get_removed_devices_json: qt_method!(fn(&self) -> QString),
}

impl UsbManager {
    fn refresh(&mut self) {
        if let Ok(devs) = futures_lite::future::block_on(enumerator::list_usb_devices()) {
            STATE.lock().unwrap().devices = devs;
        }
        self.devices_changed();
    }

    fn set_baseline(&mut self) {
        let mut state = STATE.lock().unwrap();
        state.baseline = state.devices.clone();
    }

    fn get_devices_json(&self) -> QString {
        let state = STATE.lock().unwrap();
        serde_json::to_string(&state.devices)
            .unwrap_or_else(|_| "[]".to_string())
            .into()
    }

    fn get_added_devices_json(&self) -> QString {
        let state = STATE.lock().unwrap();
        let added: Vec<_> = state
            .devices
            .iter()
            .filter(|d| !state.baseline.contains(d))
            .cloned()
            .collect();
        serde_json::to_string(&added)
            .unwrap_or_else(|_| "[]".to_string())
            .into()
    }

    fn get_removed_devices_json(&self) -> QString {
        let state = STATE.lock().unwrap();
        let removed: Vec<_> = state
            .baseline
            .iter()
            .filter(|d| !state.devices.contains(d))
            .cloned()
            .collect();
        serde_json::to_string(&removed)
            .unwrap_or_else(|_| "[]".to_string())
            .into()
    }
}
