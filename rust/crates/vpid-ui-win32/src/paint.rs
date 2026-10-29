//! GDI 绘制封装：双缓冲 + 自绘控件原语（圆角矩形 / 文本 / 发丝线）。
//!
//! 所有绘制是"原生 Win32 直绘"——无任何框架开销（对比 SOUI 皮肤系统 /
//! FLTK 事件机制），这是本分支"运行效率"收益的主要来源之一。

use std::ffi::c_void;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{COLORREF, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush,
    DeleteObject, DrawTextW, FillRect, GetStockObject, IntersectClipRect, LineTo, MoveToEx,
    RestoreDC, RoundRect, SaveDC, SelectObject, SetBkMode, SetTextColor, DT_CENTER, DT_END_ELLIPSIS,
    DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DRAW_TEXT_FORMAT, HDC, HFONT,
    HGDIOBJ, NULL_BRUSH, PS_SOLID, SRCCOPY,
};

use vpid_ui_core::theme::Color;

/// 字体族：全部微软雅黑（现代 Windows UI 字体，比宋体更平滑、无"像素感"；
/// Rust 分支面向现代 Windows，不承袭 C++ 主线的 XP 宋体约束）。
///
/// 字重层级对齐 SOUI5 init.xml（层次靠"字重 + 文字深浅"拉开，不靠字号堆叠）：
/// 400 数据 · 600 可交互与表头 · 700 数字与分组标签。
pub enum FontKind {
    /// 13/400 数据行
    Row,
    /// 13/600 按钮与表头
    BtnHeader,
    /// 13/700 分组标签
    Section,
    /// 12/700 计数徽标
    Badge,
    /// 12/600 监控胶囊
    Pill,
    /// 12/400 状态栏/次要元信息
    Small,
}

/// 文本对齐方式（单行）。
#[derive(Clone, Copy)]
pub enum Align {
    /// 左对齐 + 垂直居中 + 省略（表格单元格）。
    LeftV,
    /// 居中 + 垂直居中（按钮/胶囊）。
    CenterV,
    /// 右对齐 + 垂直居中 + 省略。
    RightV,
}

impl Align {
    fn fmt(&self) -> DRAW_TEXT_FORMAT {
        match self {
            Align::LeftV => DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            Align::CenterV => DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            Align::RightV => DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        }
    }
}

// ---- 字体常量（避免 newtype 转换样板）----
const FW_NORMAL: i32 = 400;
const FW_SEMIBOLD: i32 = 600;
const FW_BOLD: i32 = 700;
const DEFAULT_CHARSET: u32 = 1;
const CLEARTYPE_QUALITY: u32 = 5;
const PITCH_AND_FAMILY: u32 = 0;
const BK_TRANSPARENT: i32 = 1;

/// 把逻辑像素（96 DPI 基准）换算为物理像素。
/// 所有布局/命中/滚动都工作在逻辑空间，只有绘制这一刻才乘 scale 落到物理。
#[inline]
pub fn px(v: i32, scale: f32) -> i32 {
    (v as f32 * scale).round() as i32
}

/// 字体句柄集（窗口生命周期内持有，销毁时释放）。
pub struct Fonts {
    row: HFONT,
    btn_header: HFONT,
    section: HFONT,
    badge: HFONT,
    pill: HFONT,
    small: HFONT,
}

impl Fonts {
    /// 在任意 HDC 上创建字体（通常用 `BeginPaint` 的 HDC）。
    /// `scale` = dpi / 96.0，字号按 DPI 缩放（150% 时 13px -> 19.5px）。
    pub fn new(hdc: HDC, scale: f32) -> Self {
        unsafe {
            let face = to_wide("Microsoft YaHei");
            Self {
                row: create_font(hdc, px(-13, scale), FW_NORMAL, face.as_ptr()),
                btn_header: create_font(hdc, px(-13, scale), FW_SEMIBOLD, face.as_ptr()),
                section: create_font(hdc, px(-13, scale), FW_BOLD, face.as_ptr()),
                badge: create_font(hdc, px(-12, scale), FW_BOLD, face.as_ptr()),
                pill: create_font(hdc, px(-12, scale), FW_SEMIBOLD, face.as_ptr()),
                small: create_font(hdc, px(-12, scale), FW_NORMAL, face.as_ptr()),
            }
        }
    }

    pub fn get(&self, kind: FontKind) -> HFONT {
        match kind {
            FontKind::Row => self.row,
            FontKind::BtnHeader => self.btn_header,
            FontKind::Section => self.section,
            FontKind::Badge => self.badge,
            FontKind::Pill => self.pill,
            FontKind::Small => self.small,
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.row as HGDIOBJ);
            DeleteObject(self.btn_header as HGDIOBJ);
            DeleteObject(self.section as HGDIOBJ);
            DeleteObject(self.badge as HGDIOBJ);
            DeleteObject(self.pill as HGDIOBJ);
            DeleteObject(self.small as HGDIOBJ);
        }
    }
}

unsafe fn create_font(hdc: HDC, height: i32, weight: i32, face: *const u16) -> HFONT {
    let _ = hdc;
    CreateFontW(
        height, 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY,
        PITCH_AND_FAMILY, face,
    )
}

/// 绘制上下文：封装 HDC + 字体 + DPI 缩放，提供自绘原语。
/// 所有入参坐标都是**逻辑像素（96 DPI 基准）**，绘制瞬间按 `scale` 换算为物理。
pub struct Painter<'a> {
    pub hdc: HDC,
    fonts: &'a Fonts,
    scale: f32,
}

impl<'a> Painter<'a> {
    pub fn new(hdc: HDC, fonts: &'a Fonts, scale: f32) -> Self {
        Self { hdc, fonts, scale }
    }

    fn rect_phys(&self, r: &RECT) -> RECT {
        RECT {
            left: px(r.left, self.scale),
            top: px(r.top, self.scale),
            right: px(r.right, self.scale),
            bottom: px(r.bottom, self.scale),
        }
    }

    /// 纯色填充矩形（逻辑坐标）。
    pub fn fill(&self, r: &RECT, c: Color) {
        let r = self.rect_phys(r);
        unsafe {
            let b = CreateSolidBrush(cref(c));
            FillRect(self.hdc, &r, b);
            DeleteObject(b as HGDIOBJ);
        }
    }

    /// 用**物理尺寸**铺满整块画布底（避免逻辑->物理取整误差在边缘露底）。
    pub fn fill_canvas(&self, phys_w: i32, phys_h: i32, c: Color) {
        unsafe {
            let b = CreateSolidBrush(cref(c));
            let r = RECT { left: 0, top: 0, right: phys_w, bottom: phys_h };
            FillRect(self.hdc, &r, b);
            DeleteObject(b as HGDIOBJ);
        }
    }

    /// 限制后续绘制到指定矩形（逻辑坐标，按 scale 换算物理）。
    /// 返回守卫，`Drop` 时恢复完整裁剪区。
    ///
    /// 用途：表格体裁剪——数据行只能画在固定表头之下，滚动时**不会覆盖表头**，
    /// 实现"表头固定不动、仅内容滑动"。若 `SaveDC` 失败（saved==0），守卫为 no-op。
    pub fn clip_rect(&self, r: &RECT) -> ClipGuard {
        let r = self.rect_phys(r);
        unsafe {
            let saved = SaveDC(self.hdc);
            if saved != 0 {
                IntersectClipRect(self.hdc, r.left, r.top, r.right, r.bottom);
            }
            ClipGuard { hdc: self.hdc, saved }
        }
    }

    /// 圆角实心矩形（按钮/胶囊底，逻辑坐标；radius 也是逻辑像素）。
    pub fn round_rect(&self, r: &RECT, c: Color, radius: i32) {
        let r = self.rect_phys(r);
        let radius = px(radius, self.scale);
        unsafe {
            let b = CreateSolidBrush(cref(c));
            let old = SelectObject(self.hdc, b as HGDIOBJ);
            RoundRect(
                self.hdc,
                r.left,
                r.top,
                r.right,
                r.bottom,
                radius * 2,
                radius * 2,
            );
            SelectObject(self.hdc, old);
            DeleteObject(b as HGDIOBJ);
        }
    }

    /// 圆角边框（1px 描边，内部空心；逻辑坐标）。
    pub fn round_border(&self, r: &RECT, c: Color, radius: i32) {
        let r = self.rect_phys(r);
        let radius = px(radius, self.scale);
        unsafe {
            let pen = CreatePen(PS_SOLID, 1, cref(c));
            let old_pen = SelectObject(self.hdc, pen as HGDIOBJ);
            let old_brush = SelectObject(self.hdc, GetStockObject(NULL_BRUSH));
            RoundRect(
                self.hdc,
                r.left,
                r.top,
                r.right,
                r.bottom,
                radius * 2,
                radius * 2,
            );
            SelectObject(self.hdc, old_brush);
            SelectObject(self.hdc, old_pen);
            DeleteObject(pen as HGDIOBJ);
        }
    }

    /// 单行文本（指定对齐；逻辑坐标）。
    pub fn text(&self, r: &RECT, s: &str, c: Color, font: FontKind, align: Align) {
        if s.is_empty() {
            return;
        }
        unsafe {
            let hf = self.fonts.get(font);
            let old = SelectObject(self.hdc, hf as HGDIOBJ);
            SetTextColor(self.hdc, cref(c));
            SetBkMode(self.hdc, BK_TRANSPARENT);
            let mut rect = self.rect_phys(r);
            let w = to_wide(s);
            DrawTextW(self.hdc, w.as_ptr(), w.len() as i32, &mut rect, align.fmt());
            SelectObject(self.hdc, old);
        }
    }

    /// 水平发丝线（逻辑坐标）。
    pub fn hline(&self, x0: i32, x1: i32, y: i32, c: Color) {
        self.seg(x0, y, x1, y, c);
    }

    /// 垂直发丝线（逻辑坐标）。
    pub fn vline(&self, x: i32, y0: i32, y1: i32, c: Color) {
        self.seg(x, y0, x, y1, c);
    }

    fn seg(&self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
        unsafe {
            let pen = CreatePen(PS_SOLID, 1, cref(c));
            let old = SelectObject(self.hdc, pen as HGDIOBJ);
            MoveToEx(self.hdc, px(x0, self.scale), px(y0, self.scale), null_mut());
            LineTo(self.hdc, px(x1, self.scale), px(y1, self.scale));
            SelectObject(self.hdc, old);
            DeleteObject(pen as HGDIOBJ);
        }
    }
}

/// 裁剪守卫：`Painter::clip_rect` 返回，`Drop` 时 `RestoreDC` 还原裁剪区。
/// 用 RAII 保证即使中途 panic（`panic="abort"` 下实为进程终止）也能还原 DC 状态。
pub struct ClipGuard {
    hdc: HDC,
    saved: i32,
}

impl Drop for ClipGuard {
    fn drop(&mut self) {
        if self.saved != 0 {
            unsafe {
                RestoreDC(self.hdc, self.saved);
            }
        }
    }
}

/// 双缓冲位图：内存 DC + 兼容位图，绘制后一次性 BitBlt 到窗口 DC（消除闪烁）。
pub struct DoubleBuffer {
    mem_dc: HDC,
    bitmap: *mut c_void,
    old_bitmap: *mut c_void,
    width: i32,
    height: i32,
}

impl DoubleBuffer {
    pub fn new(target: HDC, width: i32, height: i32) -> Option<Self> {
        if width <= 0 || height <= 0 {
            return None;
        }
        unsafe {
            let mem_dc = CreateCompatibleDC(target);
            if mem_dc.is_null() {
                return None;
            }
            let bitmap = CreateCompatibleBitmap(target, width, height);
            if bitmap.is_null() {
                DeleteObject(mem_dc as HGDIOBJ);
                return None;
            }
            let old_bitmap = SelectObject(mem_dc, bitmap as HGDIOBJ);
            Some(Self {
                mem_dc,
                bitmap,
                old_bitmap,
                width,
                height,
            })
        }
    }

    pub fn hdc(&self) -> HDC {
        self.mem_dc
    }

    pub fn blit_to(&self, target: HDC) {
        unsafe {
            BitBlt(
                target,
                0,
                0,
                self.width,
                self.height,
                self.mem_dc,
                0,
                0,
                SRCCOPY,
            );
        }
    }
}

impl Drop for DoubleBuffer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.mem_dc, self.old_bitmap);
            DeleteObject(self.bitmap as HGDIOBJ);
            DeleteObject(self.mem_dc as HGDIOBJ);
        }
    }
}

/// RGB -> Win32 COLORREF。
/// ⚠️ COLORREF 是 **0x00BBGGRR**（蓝在低位），不是 0x00RRGGBB——
/// 用错会把红/蓝互换（停止刷新/退出变蓝、主色变橙）。已实测修正。
pub fn cref(c: Color) -> COLORREF {
    ((c.b as u32) << 16) | ((c.g as u32) << 8) | (c.r as u32)
}

/// Rust `&str` -> UTF-16（含结尾 NUL）。
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 便捷：从 client 尺寸构造 RECT。
pub fn rect(x: i32, y: i32, w: i32, h: i32) -> RECT {
    RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
}
