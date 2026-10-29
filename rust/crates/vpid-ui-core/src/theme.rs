//! 主题与布局常量（对齐 `src/ui/Theme.h` + `src/common/constants.h`）。
//!
//! 平台无关：颜色以 `(r, g, b)` 表示，具体编码（如 Win32 COLORREF）由各平台
//! 后端在渲染时转换。UI/UX 完全沿用主线 —— 以下数值与旧 SOUI/FLTK 版 1:1。

/// RGB 颜色（平台无关表示）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

// ---- 表面层级（灰窗底 + 白面板）----
pub const K_CANVAS: Color = Color::rgb(228, 231, 237);
pub const K_SURFACE1: Color = Color::rgb(255, 255, 255);
pub const K_SURFACE2: Color = Color::rgb(242, 244, 249);
pub const K_SURFACE3: Color = Color::rgb(230, 233, 240);
pub const K_SURFACE4: Color = Color::rgb(216, 220, 229);

// ---- 描边 ----
pub const K_HAIRLINE: Color = Color::rgb(203, 208, 218);
pub const K_BORDER: Color = Color::rgb(134, 141, 155);
pub const K_BORDER_STRONG: Color = Color::rgb(98, 106, 120);

// ---- 文本 ----
pub const K_TEXT_PRIMARY: Color = Color::rgb(24, 27, 33);
pub const K_TEXT_SECONDARY: Color = Color::rgb(76, 83, 94);
pub const K_TEXT_MUTED: Color = Color::rgb(106, 114, 126);
pub const K_TEXT_DISABLED: Color = Color::rgb(164, 171, 182);
pub const K_TEXT_ON_ACCENT: Color = Color::rgb(255, 255, 255);

// ---- 主色：深海蓝 ----
pub const K_ACCENT: Color = Color::rgb(30, 80, 148);
pub const K_ACCENT_HOVER: Color = Color::rgb(22, 66, 124);
pub const K_ACCENT_PRESS: Color = Color::rgb(16, 51, 99);
pub const K_ACCENT_TEXT: Color = Color::rgb(26, 74, 138);
pub const K_ACCENT_TINT: Color = Color::rgb(233, 240, 250);
pub const K_ACCENT_BORDER: Color = Color::rgb(163, 190, 225);
pub const K_ACCENT_TINT_HOVER: Color = Color::rgb(216, 230, 246);

// ---- 语义色：运行中（绿）----
pub const K_SUCCESS: Color = Color::rgb(22, 101, 72);
pub const K_SUCCESS_TINT: Color = Color::rgb(230, 243, 236);
pub const K_SUCCESS_BORDER: Color = Color::rgb(157, 199, 181);

// ---- 语义色：移除 / 危险（红）----
pub const K_DANGER: Color = Color::rgb(176, 42, 48);
pub const K_DANGER_TINT: Color = Color::rgb(251, 234, 235);
pub const K_DANGER_BORDER: Color = Color::rgb(228, 175, 178);
// 危险按钮 hover/press（SOUI skin.xml btn_danger 显式色）
pub const K_DANGER_HOVER: Color = Color::rgb(247, 219, 220);
pub const K_DANGER_PRESS: Color = Color::rgb(242, 204, 206);

// ---- 暂停（中性灰）----
pub const K_IDLE: Color = Color::rgb(99, 107, 119);
pub const K_IDLE_TINT: Color = Color::rgb(238, 241, 245);
pub const K_IDLE_BORDER: Color = Color::rgb(198, 205, 214);

// ---- 强调幽灵按钮按下色（SOUI skin.xml btn_accent pushdown）----
pub const K_ACCENT_PRESS_TINT: Color = Color::rgb(200, 218, 242);

// ---- 数据行 ----
pub const K_ROW_ODD: Color = Color::rgb(255, 255, 255);
pub const K_ROW_EVEN: Color = Color::rgb(250, 251, 253);
pub const K_ROW_HOVER: Color = Color::rgb(230, 233, 240);
pub const K_SELECT_BG: Color = Color::rgb(30, 80, 148);
pub const K_SELECT_TEXT: Color = Color::rgb(255, 255, 255);

// ---- 变更记录行标识（双通道：行底晕 + 行首竖条）----
pub const K_ROW_ADD_BG: Color = Color::rgb(233, 240, 250);
pub const K_ROW_DEL_BG: Color = Color::rgb(251, 234, 235);
pub const K_ROW_ADD_BAR: Color = Color::rgb(30, 80, 148);
pub const K_ROW_DEL_BAR: Color = Color::rgb(176, 42, 48);

// ---- 版式节奏（对齐 SOUI5 dlg_main.xml：外边距 16，分组条内边距 12）----
pub const K_PAD_OUTER: i32 = 16;
pub const K_PAD_SECTION: i32 = 12;
pub const K_PAD_GAP: i32 = 12;
pub const K_PAD_CTRL: i32 = 8;
pub const K_APP_BAR_H: i32 = 52;
pub const K_SECTION_H: i32 = 32;
pub const K_HEADER_H: i32 = 28;
pub const K_ROW_H: i32 = 26;
pub const K_STATUS_BAR_H: i32 = 28;
pub const K_BTN_H: i32 = 34;
pub const K_BADGE_H: i32 = 20;

// ---- 应用常量（对齐 constants.h）----
pub const K_APP_NAME: &str = "USB设备ID查看";
pub const K_APP_VERSION: &str = "2.0.0";
pub const K_APP_AUTHOR: &str = "USB Manager";
pub const K_AUTO_REFRESH_INTERVAL_MS: u32 = 100;
pub const K_DEFAULT_WINDOW_WIDTH: i32 = 1280;
pub const K_DEFAULT_WINDOW_HEIGHT: i32 = 720;
pub const K_MIN_WINDOW_WIDTH: i32 = 960;
pub const K_MIN_WINDOW_HEIGHT: i32 = 600;
pub const K_STATUS_CONNECTED: &str = "Connected";
