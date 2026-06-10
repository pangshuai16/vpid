use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use qmetaobject::*;

use crate::usb::{enumerator::{self, EnumerationError}, hotplug, models::UsbDeviceInfo};

/// 全局 USB 设备状态
struct DeviceState {
    devices: Vec<UsbDeviceInfo>,
    baseline: Vec<UsbDeviceInfo>,
    error: Option<String>,
    /// 数据版本号，用于 QML 判断是否需要重绘
    version: u64,
}

static STATE: LazyLock<Mutex<DeviceState>> = LazyLock::new(|| {
    Mutex::new(DeviceState {
        devices: Vec::new(),
        baseline: Vec::new(),
        error: None,
        version: 0,
    })
});

/// 数据版本号原子计数器，QML 通过比较版本号决定是否重绘
static LAST_SEEN_VERSION: AtomicU64 = AtomicU64::new(0);

/// 获取全局状态，处理 Mutex 中毒
fn get_state() -> std::sync::MutexGuard<'static, DeviceState> {
    STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 热插拔监听器（全局单例启动）
static HOTPLUG: LazyLock<Mutex<Option<hotplug::HotplugWatcher>>> = LazyLock::new(|| {
    let (tx, rx) = std::sync::mpsc::channel::<hotplug::HotplugEvent>();

    let watcher = hotplug::HotplugWatcher::new(tx);

    // 后台线程接收事件，立即触发枚举并更新全局状态
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            ::log::info!("hotplug event: {:?}", std::mem::discriminant(&event));
            match futures_lite::future::block_on(enumerator::list_usb_devices()) {
                Ok(devs) => {
                    let mut state = get_state();
                    state.devices = devs;
                    state.error = None;
                    state.version += 1;
                }
                Err(EnumerationError::Nusb(e)) => {
                    ::log::error!("USB enumeration after hotplug failed: {}", e);
                    let mut state = get_state();
                    state.error = Some(e);
                    state.version += 1;
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
    get_error: qt_method!(fn(&self) -> QString),
}

impl UsbManager {
    fn refresh(&self) {
        let _ = &*HOTPLUG;

        match futures_lite::future::block_on(enumerator::list_usb_devices()) {
            Ok(devs) => {
                let mut state = get_state();
                state.devices = devs;
                state.error = None;
                state.version += 1;
            }
            Err(EnumerationError::Nusb(e)) => {
                ::log::error!("USB enumeration failed: {}", e);
                let mut state = get_state();
                state.error = Some(e);
                state.version += 1;
            }
        }
        self.devices_changed();
    }

    /// 检查数据是否有变更，有则发射信号
    fn poll_changes(&self) {
        let _ = &*HOTPLUG;
        let state = get_state();
        let current = state.version;
        drop(state);

        let last = LAST_SEEN_VERSION.load(Ordering::Relaxed);
        if current != last {
            LAST_SEEN_VERSION.store(current, Ordering::Relaxed);
            self.devices_changed();
        }
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
        let baseline_set: HashSet<_> = state.baseline.iter().collect();
        let added: Vec<_> = state
            .devices
            .iter()
            .filter(|d| !baseline_set.contains(d))
            .cloned()
            .collect();
        serde_json::to_string(&added)
            .unwrap_or_else(|_| "[]".to_string())
            .into()
    }

    fn get_removed_devices_json(&self) -> QString {
        let state = get_state();
        let current_set: HashSet<_> = state.devices.iter().collect();
        let removed: Vec<_> = state
            .baseline
            .iter()
            .filter(|d| !current_set.contains(d))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_increments() {
        let state = get_state();
        let v = state.version;
        drop(state);

        // refresh 应该增加版本
        let mgr = UsbManager::default();
        mgr.refresh();
        assert!(get_state().version > v);
    }
}
