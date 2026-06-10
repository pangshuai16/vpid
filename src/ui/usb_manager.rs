use std::sync::{LazyLock, Mutex};

use qmetaobject::*;

use crate::usb::{enumerator, hotplug, models::UsbDeviceInfo};

/// 全局 USB 设备状态
struct DeviceState {
    devices: Vec<UsbDeviceInfo>,
    baseline: Vec<UsbDeviceInfo>,
    error: Option<String>,
}

static STATE: LazyLock<Mutex<DeviceState>> = LazyLock::new(|| {
    Mutex::new(DeviceState {
        devices: Vec::new(),
        baseline: Vec::new(),
        error: None,
    })
});

/// 获取全局状态，处理 Mutex 中毒
fn get_state() -> std::sync::MutexGuard<'static, DeviceState> {
    STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 热插拔监听器（全局单例启动）
static HOTPLUG: LazyLock<Mutex<Option<hotplug::HotplugWatcher>>> = LazyLock::new(|| {
    let (tx, rx) = std::sync::mpsc::channel::<hotplug::HotplugEvent>();

    // 启动热插拔监听线程
    let watcher = hotplug::HotplugWatcher::new(tx);

    // 后台线程接收事件，立即触发枚举并更新全局状态
    // UI 更新由 QML Timer 通过 poll_changes() 在主线程中触发
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            ::log::info!("hotplug event: {:?}", std::mem::discriminant(&event));
            match futures_lite::future::block_on(enumerator::list_usb_devices()) {
                Ok(devs) => {
                    let mut state = get_state();
                    state.devices = devs;
                    state.error = None;
                }
                Err(e) => {
                    ::log::error!("USB enumeration after hotplug failed: {}", e);
                    get_state().error = Some(e);
                }
            }
        }
    });

    Mutex::new(Some(watcher))
});

/// USB 管理器 — qmetaobject QObject 桥接
#[derive(Default, QObject)]
pub struct UsbManager {
    base: qt_base_class!(trait QObject),

    /// 设备数据变更信号（包含错误状态变更）
    devices_changed: qt_signal!(),

    refresh: qt_method!(fn(&self)),
    poll_changes: qt_method!(fn(&self)),
    set_baseline: qt_method!(fn(&self)),
    get_devices_json: qt_method!(fn(&self) -> QString),
    get_added_devices_json: qt_method!(fn(&self) -> QString),
    get_removed_devices_json: qt_method!(fn(&self) -> QString),
    /// 获取错误消息，如果无错误返回空字符串
    get_error: qt_method!(fn(&self) -> QString),
}

impl UsbManager {
    fn refresh(&self) {
        // 确保热插拔线程已启动（LazyLock 在首次 deref 时初始化）
        let _ = &*HOTPLUG;

        match futures_lite::future::block_on(enumerator::list_usb_devices()) {
            Ok(devs) => {
                let mut state = get_state();
                state.devices = devs;
                state.error = None;
            }
            Err(e) => {
                ::log::error!("USB enumeration failed: {}", e);
                let mut state = get_state();
                state.error = Some(e);
            }
        }
        self.devices_changed();
    }

    /// 检查热插拔后台线程是否更新了数据，若是则通知 UI
    /// 此方法不执行 USB 枚举（由热插拔线程完成），仅发射信号
    fn poll_changes(&self) {
        // 确保热插拔线程已启动
        let _ = &*HOTPLUG;
        // 直接发射信号，让 QML 读取最新数据
        self.devices_changed();
    }

    fn set_baseline(&self) {
        let mut state = get_state();
        state.baseline = state.devices.clone();
    }

    fn get_devices_json(&self) -> QString {
        let state = get_state();
        serde_json::to_string(&state.devices)
            .unwrap_or_else(|_| "[]".to_string())
            .into()
    }

    fn get_added_devices_json(&self) -> QString {
        let state = get_state();
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
        let state = get_state();
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

    fn get_error(&self) -> QString {
        let state = get_state();
        match &state.error {
            Some(e) => QString::from(e.as_str()),
            None => QString::default(),
        }
    }
}
