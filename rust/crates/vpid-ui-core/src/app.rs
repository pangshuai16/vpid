//! UI 状态机：扫描结果 → 视图更新；排序 / 选中 / 基准 / 复制 / 监控状态。
//!
//! 平台无关、可单测。交互行为逐条对齐旧版 MainDlg 状态机：
//! - 变更记录行序：新增在前、移除在后
//! - 排序：默认序 VID→PID→名称；用户点表头按列排，重复点同列翻转方向
//! - 三列表互斥选中（全部 / 变更两表）
//! - 复制：取当前选中设备的剪贴板文本（优先全部表，其次变更表）

use vpid_core::device::UsbDevice;

use crate::model::RowKind;

/// 一次扫描的原始结果（由后台线程产生，UI 线程消费）。
#[derive(Debug, Clone, Default)]
pub struct ScanResult {
    pub all: Vec<UsbDevice>,
    pub added: Vec<UsbDevice>,
    pub removed: Vec<UsbDevice>,
}

/// 一次扫描后的视图更新载荷（供渲染层消费）。
#[derive(Debug, Clone, Default)]
pub struct ViewUpdate {
    pub all: Vec<UsbDevice>,
    pub added: Vec<UsbDevice>,
    pub removed: Vec<UsbDevice>,
    pub changed: Vec<UsbDevice>,
    pub changed_kinds: Vec<RowKind>,
    pub diff_changed: bool,
}

/// 当前监控状态（应用栏胶囊）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorState {
    Live,
    Paused,
}

/// UI 状态机。所有字段只在 UI 线程读写（扫描/比对在后台线程，结果经消息回投）。
#[derive(Debug)]
pub struct AppState {
    pub all: Vec<UsbDevice>,
    pub baseline: Vec<UsbDevice>,
    pub added: Vec<UsbDevice>,
    pub removed: Vec<UsbDevice>,
    pub changed: Vec<UsbDevice>,
    pub changed_kinds: Vec<RowKind>,
    /// 首次扫描是否已建立基准。false = 尚未扫过（首扫只填充"全部列表"+建基准，
    /// **不**把全部设备当"新增"塞进变更栏 —— 修复"打开时右侧先全量闪现"的 bug）。
    pub baselined: bool,
    pub auto_refresh: bool,
    pub scanning: bool,
    pub monitor: MonitorState,
    // 排序状态（-1 = 默认序 VID→PID→名称；两表各自独立）
    pub sort_col_all: i32,
    pub sort_asc_all: bool,
    pub sort_col_changed: i32,
    pub sort_asc_changed: bool,
    // 选中（互斥）
    pub sel_all: Option<usize>,
    pub sel_changed: Option<usize>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            all: Vec::new(),
            baseline: Vec::new(),
            added: Vec::new(),
            removed: Vec::new(),
            changed: Vec::new(),
            changed_kinds: Vec::new(),
            baselined: false,
            auto_refresh: true,
            scanning: false,
            monitor: MonitorState::Live,
            sort_col_all: -1,
            sort_asc_all: true,
            sort_col_changed: -1,
            sort_asc_changed: true,
            sel_all: None,
            sel_changed: None,
        }
    }
}

impl AppState {
    /// 应用一次扫描结果：重建变更记录、按当前排序重排、钳制选中，返回视图更新。
    ///
    /// 首次扫描（`baselined == false`）只建立基准：填充"全部列表"、把当前快照设为
    /// 基准、**清空变更记录**（不把全部设备当"新增"）。此后每次扫描按与上一帧的差集
    /// 生成变更记录。
    pub fn apply_scan(&mut self, result: &ScanResult) -> ViewUpdate {
        self.all = result.all.clone();

        if !self.baselined {
            // 首扫：只建基准，不产生变更记录（修复打开时右侧全量闪现）
            self.baseline = result.all.clone();
            self.added.clear();
            self.removed.clear();
            self.changed.clear();
            self.changed_kinds.clear();
            self.baselined = true;
        } else {
            self.added = result.added.clone();
            self.removed = result.removed.clone();

            self.changed.clear();
            self.changed_kinds.clear();
            for d in &self.added {
                self.changed.push(d.clone());
                self.changed_kinds.push(RowKind::Add);
            }
            for d in &self.removed {
                self.changed.push(d.clone());
                self.changed_kinds.push(RowKind::Del);
            }
        }

        self.apply_sort_all();
        self.apply_sort_changed();
        self.clamp_selection();

        ViewUpdate {
            all: self.all.clone(),
            added: self.added.clone(),
            removed: self.removed.clone(),
            changed: self.changed.clone(),
            changed_kinds: self.changed_kinds.clone(),
            diff_changed: !self.added.is_empty() || !self.removed.is_empty(),
        }
    }

    // ---- 排序 ----

    /// 点击"全部设备"表头列。
    pub fn sort_all(&mut self, col: usize) {
        let col = col as i32;
        if col == self.sort_col_all {
            self.sort_asc_all = !self.sort_asc_all;
        } else {
            self.sort_col_all = col;
            self.sort_asc_all = true;
        }
        self.apply_sort_all();
    }

    /// 点击"变更记录"表头列。
    pub fn sort_changed(&mut self, col: usize) {
        let col = col as i32;
        if col == self.sort_col_changed {
            self.sort_asc_changed = !self.sort_asc_changed;
        } else {
            self.sort_col_changed = col;
            self.sort_asc_changed = true;
        }
        self.apply_sort_changed();
    }

    fn apply_sort_all(&mut self) {
        let col = self.sort_col_all;
        let asc = self.sort_asc_all;
        self.all.sort_by(|a, b| {
            let ka = sort_key_all(a, col);
            let kb = sort_key_all(b, col);
            if asc {
                ka.cmp(&kb)
            } else {
                kb.cmp(&ka)
            }
        });
    }

    fn apply_sort_changed(&mut self) {
        let col = self.sort_col_changed;
        let asc = self.sort_asc_changed;
        let mut pairs: Vec<(RowKind, UsbDevice)> = std::mem::take(&mut self.changed_kinds)
            .into_iter()
            .zip(std::mem::take(&mut self.changed))
            .collect();
        pairs.sort_by(|(ka, da), (kb, db)| {
            let a = sort_key_changed(da, *ka, col);
            let b = sort_key_changed(db, *kb, col);
            if asc {
                a.cmp(&b)
            } else {
                b.cmp(&a)
            }
        });
        self.changed = pairs.iter().map(|(_, d)| d.clone()).collect();
        self.changed_kinds = pairs.iter().map(|(k, _)| *k).collect();
    }

    // ---- 选中 ----

    /// 设置"全部设备"选中（互斥：同时清空变更表选中）。
    pub fn select_all(&mut self, row: Option<usize>) {
        self.sel_all = row;
        if row.is_some() {
            self.sel_changed = None;
        }
    }

    /// 设置"变更记录"选中（互斥）。
    pub fn select_changed(&mut self, row: Option<usize>) {
        self.sel_changed = row;
        if row.is_some() {
            self.sel_all = None;
        }
    }

    fn clamp_selection(&mut self) {
        if self.sel_all.is_some() && self.sel_all.unwrap() >= self.all.len() {
            self.sel_all = None;
        }
        if self.sel_changed.is_some() && self.sel_changed.unwrap() >= self.changed.len() {
            self.sel_changed = None;
        }
    }

    // ---- 基准 / 复制 / 监控 ----

    /// 把当前"全部设备"快照设为基准。
    pub fn set_baseline(&mut self) {
        self.baseline = self.all.clone();
    }

    /// 当前选中设备的剪贴板文本（优先全部表，其次变更表；无选中返回 None）。
    pub fn copy_selected_text(&self) -> Option<String> {
        if let Some(i) = self.sel_all {
            return self.all.get(i).map(|d| d.clipboard_text());
        }
        if let Some(i) = self.sel_changed {
            return self.changed.get(i).map(|d| d.clipboard_text());
        }
        None
    }

    /// 停止自动刷新（暂停识别）。
    pub fn stop_refresh(&mut self) {
        self.auto_refresh = false;
        self.monitor = MonitorState::Paused;
    }

    /// 恢复自动刷新。
    pub fn resume_refresh(&mut self) {
        self.auto_refresh = true;
        self.monitor = MonitorState::Live;
    }
}

// ---- 排序键（自由函数，避免与 self 借用冲突）----

/// "全部设备"排序键。默认序（col=-1）VID→PID→名称；否则按所点列。
fn sort_key_all(d: &UsbDevice, col: i32) -> (String, String, String) {
    match col {
        0 => (d.formatted_vid(), d.formatted_pid(), d.display_name()),
        1 => (d.formatted_pid(), d.formatted_vid(), d.display_name()),
        2 => (d.display_name(), d.formatted_vid(), d.formatted_pid()),
        3 => (d.serial.clone(), d.formatted_vid(), d.formatted_pid()),
        _ => (d.formatted_vid(), d.formatted_pid(), d.display_name()),
    }
}

/// "变更记录"排序键（首列是状态）。
fn sort_key_changed(d: &UsbDevice, kind: RowKind, col: i32) -> (String, String, String) {
    let status = kind.label().to_string();
    match col {
        0 => (status, d.formatted_vid(), d.formatted_pid()),
        1 => (d.formatted_vid(), d.formatted_pid(), d.display_name()),
        2 => (d.formatted_pid(), d.formatted_vid(), d.display_name()),
        3 => (d.display_name(), d.formatted_vid(), d.formatted_pid()),
        _ => (status, d.formatted_vid(), d.formatted_pid()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vpid_core::device::UsbDevice;

    fn dev(vid: &str, pid: &str, name: &str) -> UsbDevice {
        UsbDevice {
            vid: vid.to_string(),
            pid: pid.to_string(),
            name: name.to_string(),
            ..Default::default()
        }
    }

    fn result(all: Vec<UsbDevice>, added: Vec<UsbDevice>, removed: Vec<UsbDevice>) -> ScanResult {
        ScanResult { all, added, removed }
    }

    /// 首扫：只建基准 + 填充全部列表，**不**把全部设备当"新增"（修复右侧全量闪现）。
    #[test]
    fn first_scan_establishes_baseline_without_change() {
        let mut st = AppState::default();
        let vu = st.apply_scan(&result(
            vec![dev("8087", "0a2b", "Intel BT"), dev("1d6b", "0002", "Hub")],
            vec![dev("8087", "0a2b", "Intel BT"), dev("1d6b", "0002", "Hub")], // 首扫 worker 会算成全量"新增"
            vec![],
        ));
        assert!(st.baselined);
        assert_eq!(st.all.len(), 2);
        assert_eq!(st.baseline.len(), 2);
        assert!(st.added.is_empty());
        assert!(st.removed.is_empty());
        assert!(st.changed.is_empty());
        assert!(st.changed_kinds.is_empty());
        assert!(!vu.diff_changed);
    }

    /// 次扫：按差集正常产生变更记录。
    #[test]
    fn second_scan_reports_diff() {
        let mut st = AppState::default();
        st.apply_scan(&result(vec![dev("8087", "0a2b", "Intel BT")], vec![], vec![]));
        let vu = st.apply_scan(&result(
            vec![dev("8087", "0a2b", "Intel BT"), dev("1d6b", "0002", "Hub")],
            vec![dev("1d6b", "0002", "Hub")],
            vec![],
        ));
        assert!(vu.diff_changed);
        assert_eq!(st.changed.len(), 1);
        assert_eq!(st.changed_kinds, vec![RowKind::Add]);
    }

    /// 首扫 0 台设备后插入设备：仍应报"新增"（baselined 标志 vs old.is_empty() 启发式的区别）。
    #[test]
    fn after_empty_first_scan_new_device_is_added() {
        let mut st = AppState::default();
        st.apply_scan(&result(vec![], vec![], vec![])); // 首扫空
        assert!(st.baselined);
        let vu = st.apply_scan(&result(
            vec![dev("1d6b", "0002", "Hub")],
            vec![dev("1d6b", "0002", "Hub")],
            vec![],
        ));
        assert!(vu.diff_changed);
        assert_eq!(st.changed.len(), 1);
        assert_eq!(st.changed_kinds, vec![RowKind::Add]);
    }
}
