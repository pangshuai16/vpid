//! 自绘控件：按钮 / 监控胶囊 / 设备列表（表头 + 斑马纹 + 选中 + 双通道标识 + 排序指示）。
//!
//! UI/UX 完全沿用主线：主题色、行高、斑马纹、新增(蓝)/移除(红)双通道标识、排序三角，
//! 全部对齐旧版 `Theme.h` + `UsbTable` 自绘。

use windows_sys::Win32::Foundation::RECT;

use vpid_core::device::UsbDevice;
use vpid_ui_core::model::RowKind;
use vpid_ui_core::theme::*;

use crate::paint::{Align, FontKind, Painter};

/// 按钮语义（决定配色）。对齐 SOUI5 skin.xml 的四类按钮：
/// `Primary`（实心主色，无描边）/ `Ghost`（surface2+描边）/ `Danger`（危险 tint+描边）/
/// `Accent`（强调 tint+描边）。整窗只允许一个实心主按钮（刷新）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Accent,
    Danger,
    Ghost,
}

/// 按钮交互状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtnState {
    Normal,
    Hover,
    Press,
}

/// 表格悬停状态（表头列 + 数据行），由 App 在鼠标移动/按下时维护并传入绘制。
/// 对齐 SOUI5 表头三态（normal/hover/pushdown）与列表 hotTrack。
#[derive(Debug, Clone, Copy, Default)]
pub struct TableHover {
    pub header_col: Option<usize>,
    pub press_col: Option<usize>,
    pub row: Option<usize>,
}

/// 列定义：标题 + 首选宽（逻辑 px）+ 是否吸宽（flex）。
/// flex 列（长文本：名称/序列号）均分表格宽余，使列宽总和恰好填满表格，
/// 消除表头右侧空白；VID/PID/状态等短列保持紧凑，不被拉宽。
#[derive(Debug, Clone, Copy)]
pub struct ColDef {
    pub title: &'static str,
    pub w: i32,
    pub flex: bool,
}

/// 列定义：全部设备（VID / PID / 名称 / 序列号）。
pub const COLS_ALL: [ColDef; 4] = [
    ColDef { title: "VID", w: 56, flex: false },
    ColDef { title: "PID", w: 56, flex: false },
    ColDef { title: "名称", w: 240, flex: true },
    ColDef { title: "序列号", w: 200, flex: true },
];
/// 列定义：变更记录（状态 / VID / PID / 名称）。
pub const COLS_CHANGED: [ColDef; 4] = [
    ColDef { title: "状态", w: 48, flex: false },
    ColDef { title: "VID", w: 56, flex: false },
    ColDef { title: "PID", w: 56, flex: false },
    ColDef { title: "名称", w: 200, flex: true },
];

/// 由表格可用宽度计算实际列宽，使总和恰等于 `table_w`（消除表头右侧空白）。
///
/// - 宽余（`table_w > 首选总宽`）：按 flex 列首选宽比例分摊，其余列保持首选宽。
/// - 缺口（`table_w < 首选总宽`）：按首选宽比例统一收缩（防溢出、防单列过窄）。
pub fn layout_cols(table_w: i32, cols: &[ColDef]) -> Vec<i32> {
    let n = cols.len();
    if n == 0 {
        return Vec::new();
    }
    let total: i32 = cols.iter().map(|c| c.w).sum();
    let mut out: Vec<i32> = cols.iter().map(|c| c.w).collect();
    if table_w >= total {
        let extra = table_w - total;
        let flex_total: i32 = cols.iter().filter(|c| c.flex).map(|c| c.w).sum();
        if flex_total > 0 {
            let mut distributed = 0;
            for (i, c) in cols.iter().enumerate() {
                if c.flex {
                    let share = (extra * c.w) / flex_total;
                    out[i] += share;
                    distributed += share;
                }
            }
            // 取整误差补到最后一个 flex 列，保证总和精确等于 table_w
            let last_flex = cols.iter().rposition(|c| c.flex).unwrap();
            out[last_flex] += extra - distributed;
        } else {
            out[n - 1] += extra;
        }
    } else if table_w > 0 {
        let deficit = total - table_w;
        for w in out.iter_mut() {
            *w -= (*w * deficit) / total;
        }
        let sum: i32 = out.iter().sum();
        out[n - 1] += table_w - sum;
    }
    for w in out.iter_mut() {
        *w = (*w).max(1);
    }
    out
}

/// 按钮绘制。
/// 复刻 SOUI5 skin.xml：**3px 微圆角**（报表工具，非卡片流）；层级靠明度差；
/// 仅 Primary 实心无描边，Ghost/Danger/Accent 均 1px 描边 + tint 底 + 语义字色。
pub fn draw_button(p: &Painter, r: &RECT, label: &str, kind: ButtonKind, state: BtnState, disabled: bool) {
    let (bg, fg, border): (Color, Color, Option<Color>) = match (kind, state, disabled) {
        (_, _, true) => (K_SURFACE2, K_TEXT_DISABLED, None),
        (ButtonKind::Primary, BtnState::Normal, _) => (K_ACCENT, K_TEXT_ON_ACCENT, None),
        (ButtonKind::Primary, BtnState::Hover, _) => (K_ACCENT_HOVER, K_TEXT_ON_ACCENT, None),
        (ButtonKind::Primary, BtnState::Press, _) => (K_ACCENT_PRESS, K_TEXT_ON_ACCENT, None),
        (ButtonKind::Accent, BtnState::Normal, _) => (K_ACCENT_TINT, K_ACCENT_TEXT, Some(K_ACCENT_BORDER)),
        (ButtonKind::Accent, BtnState::Hover, _) => (K_ACCENT_TINT_HOVER, K_ACCENT_TEXT, Some(K_ACCENT)),
        (ButtonKind::Accent, BtnState::Press, _) => (K_ACCENT_PRESS_TINT, K_ACCENT_TEXT, Some(K_ACCENT)),
        (ButtonKind::Danger, BtnState::Normal, _) => (K_DANGER_TINT, K_DANGER, Some(K_DANGER_BORDER)),
        (ButtonKind::Danger, BtnState::Hover, _) => (K_DANGER_HOVER, K_DANGER, Some(K_DANGER)),
        (ButtonKind::Danger, BtnState::Press, _) => (K_DANGER_PRESS, K_DANGER, Some(K_DANGER)),
        (ButtonKind::Ghost, BtnState::Normal, _) => (K_SURFACE2, K_TEXT_PRIMARY, Some(K_BORDER)),
        (ButtonKind::Ghost, BtnState::Hover, _) => (K_SURFACE3, K_TEXT_PRIMARY, Some(K_BORDER_STRONG)),
        (ButtonKind::Ghost, BtnState::Press, _) => (K_SURFACE4, K_TEXT_PRIMARY, Some(K_BORDER_STRONG)),
    };
    p.round_rect(r, bg, 3);
    if let Some(b) = border {
        p.round_border(r, b, 3);
    }
    p.text(r, label, fg, FontKind::BtnHeader, Align::CenterV);
}

/// 监控状态胶囊（live/idle）。
/// 复刻 SOUI5 skin.xml pill：13px 圆角 + 1px 描边 + tint 底，文字 12/600。
pub fn draw_pill(p: &Painter, r: &RECT, text: &str, live: bool) {
    let (bg, fg, border) = if live {
        (K_SUCCESS_TINT, K_SUCCESS, K_SUCCESS_BORDER)
    } else {
        (K_IDLE_TINT, K_IDLE, K_IDLE_BORDER)
    };
    p.round_rect(r, bg, 13);
    p.round_border(r, border, 13);
    p.text(r, text, fg, FontKind::Pill, Align::CenterV);
}

/// 小计数徽标（分组条右侧）。
/// 复刻 SOUI5 skin.xml badge：3px 微圆角 + 1px 描边 + tint 底，文字 12/700。
pub fn draw_badge(p: &Painter, r: &RECT, text: &str, bg: Color, fg: Color, border: Color) {
    p.round_rect(r, bg, 3);
    p.round_border(r, border, 3);
    p.text(r, text, fg, FontKind::Badge, Align::CenterV);
}

/// 设备列表绘制（表头 + 数据行）。
///
/// - `rows`：数据行；`kinds`：行语义（普通表全 `None`）；`sel`：选中行（None=无）。
/// - `scroll`：垂直滚动像素偏移；`sort_col`/`sort_asc`：排序指示（-1 = 默认序）。
/// - `hover`：表头列/数据行悬停与按下状态（SOUI5 表头三态 + hotTrack）。
/// - `changed`：是否为变更记录表（首列状态 + 双通道标识）。
pub fn draw_table(
    p: &Painter,
    rect: &RECT,
    cols: &[ColDef],
    rows: &[UsbDevice],
    kinds: &[RowKind],
    sel: Option<usize>,
    scroll: i32,
    sort_col: i32,
    sort_asc: bool,
    changed: bool,
    hover: &TableHover,
) {
    // 实际列宽：总和恰好填满表格（消除表头右侧空白），且表头/数据行共用同一组宽度。
    let widths = layout_cols(rect.right - rect.left, cols);
    // 表头（三态：normal=surface2 hover=surface3 pushdown=surface4，对齐 SOUI5 head_item）
    let header = RECT {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.top + K_HEADER_H,
    };
    let mut x = rect.left;
    for (i, c) in cols.iter().enumerate() {
        let w = widths[i];
        let cell = RECT {
            left: x,
            top: header.top,
            right: x + w,
            bottom: header.bottom,
        };
        let hbg = if hover.press_col == Some(i) {
            K_SURFACE4
        } else if hover.header_col == Some(i) {
            K_SURFACE3
        } else {
            K_SURFACE2
        };
        p.fill(&cell, hbg);
        let label = if (sort_col as usize) == i {
            format!("{} {}", c.title, if sort_asc { "▲" } else { "▼" })
        } else {
            c.title.to_string()
        };
        // 表头文字：左对齐 + 8px 缩进（对齐数据区，SOUI5 text-x=8），13/600
        let hcell = RECT {
            left: cell.left + 8,
            top: cell.top,
            right: cell.right - 4,
            bottom: cell.bottom,
        };
        p.text(&hcell, &label, K_TEXT_SECONDARY, FontKind::BtnHeader, Align::LeftV);
        // 列分隔（右缘发丝线）
        p.vline(cell.right - 1, header.top, header.bottom, K_HAIRLINE);
        x += w;
    }
    p.hline(rect.left, rect.right, header.bottom, K_HAIRLINE);

    // 数据行（裁剪到表格体：表头固定不动，内容仅在下半区滚动）。
    // 关键：clip_rect 把行绘制限制在 [header.bottom, rect.bottom]，滚动时行向上滚
    // 也不会覆盖固定表头（行在表头之后绘制，若无裁剪会盖住表头文字）。
    let body_top = rect.top + K_HEADER_H;
    let body_bottom = rect.bottom;
    let _clip = p.clip_rect(&RECT {
        left: rect.left,
        top: body_top,
        right: rect.right,
        bottom: body_bottom,
    });
    let mut y = header.bottom - scroll;
    for (i, d) in rows.iter().enumerate() {
        let row = RECT {
            left: rect.left,
            top: y,
            right: rect.right,
            bottom: y + K_ROW_H,
        };
        y += K_ROW_H;
        // 快速跳过完全在表格体之外的行（裁剪已兜底，这里是避免无谓绘制）
        if row.bottom <= body_top || row.top >= body_bottom {
            continue;
        }
        let kind = kinds.get(i).copied().unwrap_or(RowKind::None);

        // 行底：选中 > 悬停(hotTrack) > 变更晕 > 斑马纹
        let bg = if Some(i) == sel {
            K_SELECT_BG
        } else if hover.row == Some(i) {
            K_ROW_HOVER
        } else if changed && kind == RowKind::Add {
            K_ROW_ADD_BG
        } else if changed && kind == RowKind::Del {
            K_ROW_DEL_BG
        } else if i % 2 == 1 {
            K_ROW_EVEN
        } else {
            K_ROW_ODD
        };
        p.fill(&row, bg);

        // 双通道：行首竖条（新增=主色，移除=危险）
        if changed {
            let bar = RECT {
                left: row.left,
                top: row.top,
                right: row.left + 3,
                bottom: row.bottom,
            };
            match kind {
                RowKind::Add => p.fill(&bar, K_ROW_ADD_BAR),
                RowKind::Del => p.fill(&bar, K_ROW_DEL_BAR),
                RowKind::None => {}
            }
        }

        let text_color = if Some(i) == sel {
            K_SELECT_TEXT
        } else {
            K_TEXT_PRIMARY
        };
        // 文字缩进：每列左 8px（变更表首列 12px 让开竖条），右 4px；对齐 SOUI5 text-x=8
        let mut cx = rect.left;
        for (j, _) in cols.iter().enumerate() {
            let w = widths[j];
            let pad = if changed && j == 0 { 12 } else { 8 };
            let cell = RECT {
                left: cx + pad,
                top: row.top,
                right: cx + w - 4,
                bottom: row.bottom,
            };
            let s = cell_text(d, kind, j, changed);
            p.text(&cell, &s, text_color, FontKind::Row, Align::LeftV);
            cx += w;
        }
    }
}

/// 单元格文本（按列/表语义分派）。
fn cell_text(d: &UsbDevice, kind: RowKind, col: usize, changed: bool) -> String {
    if changed {
        match col {
            0 => kind.label().to_string(),
            1 => d.formatted_vid(),
            2 => d.formatted_pid(),
            3 => d.display_name(),
            _ => String::new(),
        }
    } else {
        match col {
            0 => d.formatted_vid(),
            1 => d.formatted_pid(),
            2 => d.display_name(),
            3 => d.serial.clone(),
            _ => String::new(),
        }
    }
}

/// 命中：表头列索引（返回所在列，未命中返回 None）。
/// 列宽与绘制一致（`layout_cols`），保证悬停/排序命中的列与视觉列完全对齐。
pub fn hit_header(cols: &[ColDef], table: &RECT, x: i32, y: i32) -> Option<usize> {
    if y < table.top || y >= table.top + K_HEADER_H {
        return None;
    }
    let widths = layout_cols(table.right - table.left, cols);
    let mut cx = table.left;
    for (i, w) in widths.iter().enumerate() {
        if x >= cx && x < cx + w {
            return Some(i);
        }
        cx += w;
    }
    None
}

/// 命中：数据行索引（返回 (行号, 是否表头区)。未命中返回 None）。
pub fn hit_row(table: &RECT, y: i32, scroll: i32) -> Option<usize> {
    let body_top = table.top + K_HEADER_H;
    if y < body_top || y >= table.bottom {
        return None;
    }
    let rel = y - body_top + scroll;
    if rel < 0 {
        return None;
    }
    Some((rel / K_ROW_H) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 列宽总和必须精确等于表格可用宽度（消除表头右侧空白），任意宽度下成立。
    #[test]
    fn layout_cols_sum_equals_table_width() {
        // 默认/最小/超宽/超窄 各种表格宽度下，两表列宽总和都精确填满。
        for table_w in [300, 360, 419, 552, 602, 813, 1280] {
            let s: i32 = layout_cols(table_w, &COLS_ALL).iter().sum();
            assert_eq!(s, table_w, "COLS_ALL @{}", table_w);
            let s: i32 = layout_cols(table_w, &COLS_CHANGED).iter().sum();
            assert_eq!(s, table_w, "COLS_CHANGED @{}", table_w);
        }
    }

    /// 宽余只给 flex 列（名称/序列号），VID/PID/状态保持首选宽不被拉宽。
    #[test]
    fn extra_goes_to_flex_columns_only() {
        // 默认 1280 窗口：左表宽 813 > 首选总宽 552，宽余 261 全部给 flex 列。
        let w = layout_cols(813, &COLS_ALL);
        assert_eq!(w[0], 56, "VID 保持首选宽");
        assert_eq!(w[1], 56, "PID 保持首选宽");
        assert!(w[2] > 240, "名称吸宽");
        assert!(w[3] > 200, "序列号吸宽");
        assert_eq!(w[2] + w[3], 813 - 112, "flex 列吸收全部宽余");
        // 变更表：状态/VID/PID 固定，名称吸宽。
        let w2 = layout_cols(419, &COLS_CHANGED);
        assert_eq!(w2[0], 48);
        assert_eq!(w2[1], 56);
        assert_eq!(w2[2], 56);
        assert_eq!(w2[3], 419 - 160, "名称吸收全部宽余");
    }

    /// 缺口（表格窄于首选总宽）按比例收缩且总和仍精确，不溢出面板。
    #[test]
    fn deficit_shrinks_proportionally_without_overflow() {
        let w = layout_cols(310, &COLS_CHANGED);
        assert_eq!(w.iter().sum::<i32>(), 310);
        assert!(w.iter().all(|x| *x >= 1));
        // 收缩不改变列顺序单调性（名称仍最宽）
        assert!(w[3] > w[2] && w[2] == w[1]);
    }
}
