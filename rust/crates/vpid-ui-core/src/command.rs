//! 用户命令：UI 层（任何平台）把输入翻译成命令，交给 `AppState` 处理。
//!
//! 命令集 1:1 对齐旧版 MainDlg 的按钮/表头/列表交互语义。

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// 停止自动刷新（暂停识别）
    StopRefresh,
    /// 恢复自动刷新
    ResumeRefresh,
    /// 立即手动刷新一次
    RefreshNow,
    /// 把当前"全部设备"快照设为基准
    SetBaseline,
    /// 复制当前选中到剪贴板
    CopySelected,
    /// 退出应用
    Exit,
    /// 点击"全部设备"表头列
    SortAll { col: usize },
    /// 点击"变更记录"表头列
    SortChanged { col: usize },
    /// 选择"全部设备"某行（None = 清空选中）
    SelectAll { row: Option<usize> },
    /// 选择"变更记录"某行
    SelectChanged { row: Option<usize> },
}
