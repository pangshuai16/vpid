//! 设备比对：新增/移除 diff。
//!
//! 忠实移植自 `src/core/device_comparer.cpp`。
//! 用 `BTreeSet<(vid, pid, serial)>`（字典序，等价 C++ `std::set<DeviceKey>`）：
//! - added   = 在 new 但不在 old（保持 new 顺序）
//! - removed = 在 old 但不在 new（保持 old 顺序）
//! - changed = 唯一键集合是否不同

use std::collections::BTreeSet;

use crate::device::UsbDevice;

#[derive(Debug, Default, Clone)]
pub struct Diff {
    pub added: Vec<UsbDevice>,
    pub removed: Vec<UsbDevice>,
    pub changed: bool,
}

pub fn diff(old_dev: &[UsbDevice], new_dev: &[UsbDevice]) -> Diff {
    let old_keys: BTreeSet<(String, String, String)> = old_dev.iter().map(|d| d.key()).collect();
    let new_keys: BTreeSet<(String, String, String)> = new_dev.iter().map(|d| d.key()).collect();

    let changed = old_keys != new_keys;
    let added = new_dev
        .iter()
        .filter(|d| !old_keys.contains(&d.key()))
        .cloned()
        .collect();
    let removed = old_dev
        .iter()
        .filter(|d| !new_keys.contains(&d.key()))
        .cloned()
        .collect();

    Diff {
        added,
        removed,
        changed,
    }
}

/// 唯一键集合是否不一致（对齐 `hasChanged`）。
pub fn has_changed(a: &[UsbDevice], b: &[UsbDevice]) -> bool {
    let ka: BTreeSet<_> = a.iter().map(|d| d.key()).collect();
    let kb: BTreeSet<_> = b.iter().map(|d| d.key()).collect();
    if ka.len() != kb.len() {
        return true;
    }
    ka.iter().any(|k| !kb.contains(k))
}
