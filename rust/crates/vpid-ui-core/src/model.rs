//! 视图模型：变更记录行语义 + 列定义（平台无关）。

/// 变更记录行的语义（新增 / 移除 / 普通）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    None,
    Add,
    Del,
}

impl RowKind {
    /// 状态列文案（对齐旧版"新增/移除"）。
    pub fn label(&self) -> &'static str {
        match self {
            RowKind::Add => "新增",
            RowKind::Del => "移除",
            RowKind::None => "",
        }
    }
}
