//! Win32 应用：窗口类注册、窗口创建、WndProc、消息循环、扫描线程模型、自绘编排。
//!
//! 线程模型（对齐旧版铁律，且比旧版更安全）：
//! - UI 状态只在 UI 线程读写；扫描/比对在后台 `std::thread`。
//! - 后台线程只持有 `Arc<dyn DeviceProvider>` + 结果发送端 + 旧快照克隆，
//!   **绝不持有窗口/App 引用**（旧版要"只装结果堆指针"规避 UAF，Rust 所有权
//!   从类型层面根除这一整类 bug）。
//! - 结果经 `std::sync::mpsc` 回投，UI 线程在 100ms 定时器里 `try_recv` 排空。
//! - 退出：请求取消 + `JoinHandle::join()` 阻塞到在途扫描返回（确定性收尾），
//!   替代旧版"请求取消 + join + ExitProcess"三重保险。

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT, UpdateWindow};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW,
    GetWindowLongPtrW, KillTimer, LoadCursorW, LoadIconW, PostQuitMessage, RegisterClassExW,
    SetTimer,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, CREATESTRUCTW, CS_HREDRAW,
    CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA, IDC_ARROW, MSG, SW_SHOW, SWP_NOACTIVATE,
    SWP_NOZORDER, WNDCLASSEXW, WM_CLOSE, WM_CREATE, WM_DESTROY, WM_DEVICECHANGE, WM_DPICHANGED,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY,
    WM_PAINT, WM_SIZE, WM_TIMER, WS_OVERLAPPEDWINDOW,
};

use vpid_core::notifier::WindowsNotifier;
use vpid_ui_core::app::{AppState, MonitorState, ScanResult};
use vpid_ui_core::command::Command;
use vpid_ui_core::platform::{DeviceProvider, Platform, PlatformError};
use vpid_ui_core::theme::*;

use crate::paint::{px, rect, DoubleBuffer, Fonts, Painter};
use crate::widgets::{self, BtnState, ButtonKind, TableHover, draw_badge, draw_button, draw_pill, draw_table};

const TIMER_AUTO: usize = 1;

/// 平台后端（Win32 实现 `Platform` trait）。
pub struct Win32App;

impl Platform for Win32App {
    fn run(self, state: AppState, provider: Arc<dyn DeviceProvider>) -> Result<(), PlatformError> {
        unsafe { run_ui(state, provider) }
    }
}

// ---------------- 主入口：注册类 + 建窗 + 消息循环 ----------------

unsafe fn run_ui(state: AppState, provider: Arc<dyn DeviceProvider>) -> Result<(), PlatformError> {
    let hinstance = GetModuleHandleW(null());

    let class_name = crate::paint::to_wide("VPidMainWnd");
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        // 资源图标 id=1（build.rs winres set_icon 嵌入的 RT_GROUP_ICON）。
        // 仅嵌入不生效——窗口类必须显式 LoadIconW 加载，否则任务栏显示默认图标。
        hIcon: LoadIconW(hinstance, 1 as *const u16),
        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: LoadIconW(hinstance, 1 as *const u16),
    };
    if RegisterClassExW(&wc) == 0 {
        return Err("RegisterClassExW 失败".into());
    }

    let (tx, rx) = channel::<ScanResult>();
    let app = Box::new(App {
        hwnd: null_mut(),
        state,
        provider,
        tx,
        rx,
        layout: Layout::empty(),
        fonts: None,
        notifier: WindowsNotifier::new(),
        scan_join: None,
        hover_btn: None,
        press_btn: None,
        scroll_all: 0,
        scroll_changed: 0,
        hover_all: TableHover::default(),
        hover_changed: TableHover::default(),
        status_a: String::new(),
        status_b: String::new(),
        closing: false,
        scale: 1.0,
        client_w_phys: 0,
        client_h_phys: 0,
    });
    let app_ptr = Box::into_raw(app);

    // 初始 DPI 缩放：用系统 DPI 近似（窗口落在主显示器上），建窗后以 GetDpiForWindow 校准。
    // 窗口尺寸用物理像素（DPI 感知进程），故按 scale 放大，保持与 96dpi 相同的"外观大小"。
    let sys_dpi = unsafe { GetDpiForSystem() };
    let scale = if sys_dpi > 0 { sys_dpi as f32 / 96.0 } else { 1.0 };
    let win_w = px(K_DEFAULT_WINDOW_WIDTH, scale);
    let win_h = px(K_DEFAULT_WINDOW_HEIGHT, scale);

    let title = crate::paint::to_wide(K_APP_NAME);
    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        win_w,
        win_h,
        null_mut(),
        null_mut(),
        hinstance,
        app_ptr as *const c_void,
    );
    if hwnd.is_null() {
        drop(Box::from_raw(app_ptr));
        return Err("CreateWindowExW 失败".into());
    }

    // 用窗口实际所在显示器的 DPI 校准缩放，并重算布局（WM_SIZE 在创建期间以 scale=1 计算过）。
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        if dpi > 0 {
            (*app_ptr).scale = dpi as f32 / 96.0;
        }
        (*app_ptr).relayout();
        (*app_ptr).invalidate();
    }

    ShowWindow(hwnd, SW_SHOW);
    UpdateWindow(hwnd);
    // 定时器在 WM_CREATE 已启动（on_create）

    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    // App 在 WM_NCDESTROY 释放
    Ok(())
}

/// 取 App 的可变引用（UI 线程独占；WndProc 串行调用，无并发访问）。
unsafe fn app_ref(hwnd: HWND) -> &'static mut App {
    let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    &mut *p
}

// ---------------- WndProc ----------------

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = lparam as *const CREATESTRUCTW;
            let app = (*cs).lpCreateParams as *mut App;
            if !app.is_null() {
                (*app).hwnd = hwnd;
            }
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, app as isize);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if p != 0 {
                drop(Box::from_raw(p as *mut App));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CREATE => {
            app_ref(hwnd).on_create();
            0
        }
        WM_PAINT => {
            app_ref(hwnd).on_paint();
            0
        }
        WM_SIZE => {
            let w = (lparam & 0xFFFF) as i32;
            let h = ((lparam >> 16) & 0xFFFF) as i32;
            app_ref(hwnd).on_size(w, h);
            0
        }
        WM_TIMER => {
            app_ref(hwnd).on_timer();
            0
        }
        WM_LBUTTONDOWN => {
            let app = app_ref(hwnd);
            let (x, y) = lp_xy(lparam, app.scale);
            app.on_lbutton_down(x, y);
            0
        }
        WM_LBUTTONUP => {
            let app = app_ref(hwnd);
            let (x, y) = lp_xy(lparam, app.scale);
            app.on_lbutton_up(x, y);
            0
        }
        WM_MOUSEMOVE => {
            let app = app_ref(hwnd);
            let (x, y) = lp_xy(lparam, app.scale);
            app.on_mouse_move(x, y);
            0
        }
        WM_MOUSEWHEEL => {
            app_ref(hwnd).on_mouse_wheel(wparam);
            0
        }
        WM_DPICHANGED => {
            // 窗口拖到不同 DPI 的显示器：按系统建议矩形缩放，更新 scale + 重排 + 重建字体。
            let app = app_ref(hwnd);
            let new_dpi = (wparam & 0xFFFF) as u32;
            if new_dpi > 0 {
                app.scale = new_dpi as f32 / 96.0;
            }
            // 字号随 DPI 变化，必须重建字体（Drop 释放旧 GDI 对象，下次 on_paint 用新 scale 重建）
            app.fonts = None;
            let rc = lparam as *const RECT;
            if !rc.is_null() {
                unsafe {
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        (*rc).left,
                        (*rc).top,
                        (*rc).right - (*rc).left,
                        (*rc).bottom - (*rc).top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            app.relayout();
            app.invalidate();
            0
        }
        WM_DEVICECHANGE => {
            app_ref(hwnd).on_device_change();
            0
        }
        WM_CLOSE => {
            app_ref(hwnd).on_close();
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// lParam -> (client 逻辑 x, 逻辑 y)。lParam 是物理像素（DPI 感知进程），除以 scale 得逻辑坐标。
fn lp_xy(lparam: LPARAM, scale: f32) -> (i32, i32) {
    let x = (lparam & 0xFFFF) as i16 as i32;
    let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
    let inv = if scale > 0.0 { 1.0 / scale } else { 1.0 };
    (px(x, inv), px(y, inv))
}

/// 把表格滚动偏移钳制到 `[0, 最大可滚动]`。
/// 最大可滚动 = `max(0, 行数 - 可视行数) * K_ROW_H`；行数不足一屏时为 0。
fn clamp_table_scroll(table: &RECT, row_count: usize, scroll: i32) -> i32 {
    let rows = row_count as i32;
    let view_rows = (table.bottom - table.top - K_HEADER_H).max(0) / K_ROW_H;
    let max_scroll = ((rows - view_rows).max(0)) * K_ROW_H;
    scroll.clamp(0, max_scroll)
}

// ---------------- 按钮 / 布局 ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonId {
    Stop,
    Resume,
    Baseline,
    Copy,
    Refresh,
    Exit,
}

/// 布局矩形集合（由 client 尺寸计算，复刻 SOUI5 dlg_main.xml 布局）。
struct Layout {
    client_w: i32,
    client_h: i32,
    appbar: RECT,
    main: RECT,
    statusbar: RECT,
    left_panel: RECT,
    right_panel: RECT,
    left_section: RECT,
    right_section: RECT,
    left_table: RECT,
    right_table: RECT,
    pill: RECT,
    btn_stop: RECT,
    btn_resume: RECT,
    btn_baseline: RECT,
    btn_copy: RECT,
    btn_refresh: RECT,
    btn_exit: RECT,
    txt_total: RECT,
    txt_added: RECT,
    txt_removed: RECT,
    status_a: RECT,
    status_b: RECT,
}

impl Layout {
    fn empty() -> Self {
        let z = || RECT { left: 0, top: 0, right: 0, bottom: 0 };
        Self {
            client_w: 0,
            client_h: 0,
            appbar: z(),
            main: z(),
            statusbar: z(),
            left_panel: z(),
            right_panel: z(),
            left_section: z(),
            right_section: z(),
            left_table: z(),
            right_table: z(),
            pill: z(),
            btn_stop: z(),
            btn_resume: z(),
            btn_baseline: z(),
            btn_copy: z(),
            btn_refresh: z(),
            btn_exit: z(),
            txt_total: z(),
            txt_added: z(),
            txt_removed: z(),
            status_a: z(),
            status_b: z(),
        }
    }

    fn compute(w: i32, h: i32) -> Self {
        let mut l = Layout::empty();
        l.client_w = w;
        l.client_h = h;
        l.appbar = rect(0, 0, w, K_APP_BAR_H);

        let main_top = K_APP_BAR_H + 1;
        let main_h = (h - main_top - K_STATUS_BAR_H - 1).max(1);
        l.main = rect(0, main_top, w, main_h);
        l.statusbar = rect(0, h - K_STATUS_BAR_H, w, K_STATUS_BAR_H);

        // 应用栏子控件（SOUI5：无标题文字，监控胶囊在最左 16px；按钮右对齐至 w-16）
        let cy = (K_APP_BAR_H - K_BTN_H) / 2;
        l.pill = rect(K_PAD_OUTER, (K_APP_BAR_H - 26) / 2, 120, 26);

        let gap = K_PAD_CTRL;
        let mut bx = w - K_PAD_OUTER - 64;
        l.btn_exit = rect(bx, cy, 64, K_BTN_H);
        bx -= gap + 72;
        l.btn_refresh = rect(bx, cy, 72, K_BTN_H);
        bx -= gap + 64;
        l.btn_copy = rect(bx, cy, 64, K_BTN_H);
        bx -= gap + 92;
        l.btn_baseline = rect(bx, cy, 92, K_BTN_H);
        bx -= gap + 92;
        l.btn_resume = rect(bx, cy, 92, K_BTN_H);
        bx -= gap + 92;
        l.btn_stop = rect(bx, cy, 92, K_BTN_H);

        // 主区：外边距 16（左右）/12（上下），面板间距 12；面板带 1px 描边（内容内缩 1px）
        let ph = K_PAD_OUTER;
        let pv = 12;
        let gapm = K_PAD_GAP;
        let avail_w = (w - ph * 2).max(1);
        let avail_h = (main_h - pv * 2).max(1);
        let left_w = (avail_w - gapm) * 66 / 100;
        let right_w = avail_w - gapm - left_w;
        let py = main_top + pv;
        l.left_panel = rect(ph, py, left_w, avail_h);
        l.right_panel = rect(ph + left_w + gapm, py, right_w, avail_h);

        // 面板内容区（内缩 1px 描边）：分组条 32 + 发丝 1 + 表格
        let sec_y = py + 1;
        l.left_section = rect(ph + 1, sec_y, left_w - 2, K_SECTION_H);
        l.right_section = rect(ph + left_w + gapm + 1, sec_y, right_w - 2, K_SECTION_H);
        let tbl_h = (avail_h - 1 - K_SECTION_H - 1).max(1);
        l.left_table = rect(ph + 1, sec_y + K_SECTION_H + 1, left_w - 2, tbl_h);
        l.right_table = rect(ph + left_w + gapm + 1, sec_y + K_SECTION_H + 1, right_w - 2, tbl_h);

        // 计数徽标（分组条右侧，距面板右缘 12px）
        let bcy = sec_y + (K_SECTION_H - 20) / 2;
        let ls_right = l.left_section.right;
        l.txt_total = rect(ls_right - K_PAD_SECTION - 70, bcy, 70, 20);
        let rs_right = l.right_section.right;
        l.txt_removed = rect(rs_right - K_PAD_SECTION - 70, bcy, 70, 20);
        l.txt_added = rect(rs_right - K_PAD_SECTION - 70 - 8 - 70, bcy, 70, 20);

        // 状态栏
        l.status_a = rect(K_PAD_OUTER, h - K_STATUS_BAR_H + (K_STATUS_BAR_H - 20) / 2, w / 2, 20);
        l.status_b = rect(w / 2, h - K_STATUS_BAR_H + (K_STATUS_BAR_H - 20) / 2, w / 2 - K_PAD_OUTER, 20);
        l
    }
}

// ---------------- App ----------------

struct App {
    hwnd: HWND,
    state: AppState,
    provider: Arc<dyn DeviceProvider>,
    tx: Sender<ScanResult>,
    rx: Receiver<ScanResult>,
    layout: Layout,
    fonts: Option<Fonts>,
    notifier: WindowsNotifier,
    scan_join: Option<JoinHandle<()>>,
    hover_btn: Option<ButtonId>,
    press_btn: Option<ButtonId>,
    scroll_all: i32,
    scroll_changed: i32,
    /// 表格悬停/按下状态（表头三态 + 行 hotTrack），由鼠标移动/按下维护。
    hover_all: TableHover,
    hover_changed: TableHover,
    status_a: String,
    status_b: String,
    closing: bool,
    /// DPI 缩放因子（dpi / 96.0）。1.0 = 100%。
    scale: f32,
    /// 物理客户区尺寸（WM_SIZE 的 lParam；布局按 scale 换算为逻辑）。
    client_w_phys: i32,
    client_h_phys: i32,
}

impl App {
    fn on_create(&mut self) {
        // 用窗口所在显示器的实际 DPI 校准缩放（Per-Monitor V2 下即每显示器各自的值）。
        // 建窗期间 WM_SIZE 先于首个 WM_PAINT，先在此定 scale，后续 on_size 即按正确缩放布局。
        unsafe {
            let dpi = GetDpiForWindow(self.hwnd);
            if dpi > 0 {
                self.scale = dpi as f32 / 96.0;
            }
        }
        // 注册设备插拔通知（WM_DEVICECHANGE 驱动；失败则退化为定时轮询）
        self.notifier.start(self.hwnd as *mut c_void);
        unsafe {
            SetTimer(self.hwnd, TIMER_AUTO, K_AUTO_REFRESH_INTERVAL_MS, None);
        }
    }

    fn on_size(&mut self, w: i32, h: i32) {
        self.client_w_phys = w;
        self.client_h_phys = h;
        self.relayout();
        self.invalidate();
    }

    /// 由物理客户区尺寸 + 当前 scale 重算逻辑布局。
    fn relayout(&mut self) {
        let lw = px(self.client_w_phys, if self.scale > 0.0 { 1.0 / self.scale } else { 1.0 });
        let lh = px(self.client_h_phys, if self.scale > 0.0 { 1.0 / self.scale } else { 1.0 });
        self.layout = Layout::compute(lw.max(1), lh.max(1));
    }

    fn on_paint(&mut self) {
        unsafe {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(self.hwnd, &mut ps);
            let mut cr: RECT = std::mem::zeroed();
            GetClientRect(self.hwnd, &mut cr);
            let w = cr.right - cr.left;
            let h = cr.bottom - cr.top;
            if self.fonts.is_none() {
                self.fonts = Some(Fonts::new(hdc, self.scale));
            }
            if let Some(fonts) = self.fonts.as_ref() {
                if let Some(buf) = DoubleBuffer::new(hdc, w, h) {
                    let p = Painter::new(buf.hdc(), fonts, self.scale);
                    // 物理铺底，避免逻辑->物理取整误差在边缘露底
                    p.fill_canvas(w, h, K_CANVAS);
                    self.draw(&p);
                    buf.blit_to(hdc);
                }
            }
            EndPaint(self.hwnd, &ps);
        }
    }

    /// 唯一"数据 -> 屏幕"出口（对齐旧版 RefreshViews 职责）。
    /// 画布底已由 `on_paint` 物理铺满（fill_canvas），此处只画内容层。
    fn draw(&self, p: &Painter) {
        let l = &self.layout;
        let s = &self.state;

        // ---- 应用栏（SOUI5：无标题文字，只有监控胶囊 + 右侧操作组）----
        p.fill(&l.appbar, K_SURFACE1);
        let live = s.monitor == MonitorState::Live;
        draw_pill(p, &l.pill, if live { "● 实时监控中" } else { "● 监控已暂停" }, live);

        let st = |b: ButtonId| self.btn_state(b);
        // 层级：只有"刷新"是实心主按钮（Primary），其余为带描边的幽灵/tint
        draw_button(p, &l.btn_stop, "停止刷新", ButtonKind::Danger, st(ButtonId::Stop), !s.auto_refresh);
        draw_button(p, &l.btn_resume, "恢复刷新", ButtonKind::Accent, st(ButtonId::Resume), s.auto_refresh);
        draw_button(p, &l.btn_baseline, "重置基准", ButtonKind::Ghost, st(ButtonId::Baseline), false);
        draw_button(p, &l.btn_copy, "复制", ButtonKind::Ghost, st(ButtonId::Copy), false);
        draw_button(p, &l.btn_refresh, "刷新", ButtonKind::Primary, st(ButtonId::Refresh), false);
        draw_button(p, &l.btn_exit, "退出", ButtonKind::Danger, st(ButtonId::Exit), false);
        p.hline(0, l.client_w, K_APP_BAR_H, K_HAIRLINE);

        // ---- 左：全部设备 ----
        self.draw_panel_left(p);

        // ---- 右：变更记录 ----
        self.draw_panel_right(p);

        // ---- 状态栏 ----
        p.fill(&l.statusbar, K_SURFACE2);
        p.hline(0, l.client_w, l.client_h - K_STATUS_BAR_H - 1, K_HAIRLINE);
        p.text(&l.status_a, &self.status_a, K_TEXT_SECONDARY, crate::paint::FontKind::Small, crate::paint::Align::LeftV);
        p.text(&l.status_b, &self.status_b, K_TEXT_MUTED, crate::paint::FontKind::Small, crate::paint::Align::RightV);
    }

    fn draw_panel_left(&self, p: &Painter) {
        let l = &self.layout;
        p.fill(&l.left_panel, K_SURFACE1);
        // 面板 1px 描边（SOUI5 panel skin：surface1 + hairline 边框）
        p.round_border(&l.left_panel, K_HAIRLINE, 0);
        p.fill(&l.left_section, K_SURFACE2);
        p.text(
            &rect(l.left_section.left + K_PAD_SECTION, l.left_section.top, 100, K_SECTION_H),
            "全部设备",
            K_TEXT_PRIMARY,
            crate::paint::FontKind::Section,
            crate::paint::Align::LeftV,
        );
        draw_badge(
            p,
            &l.txt_total,
            &format!("{}", self.state.all.len()),
            K_SURFACE2,
            K_TEXT_SECONDARY,
            K_BORDER,
        );
        p.hline(l.left_section.left, l.left_section.right, l.left_section.top + K_SECTION_H, K_HAIRLINE);
        draw_table(
            p,
            &l.left_table,
            &widgets::COLS_ALL,
            &self.state.all,
            &[],
            self.state.sel_all,
            self.scroll_all,
            self.state.sort_col_all,
            self.state.sort_asc_all,
            false,
            &self.hover_all,
        );
    }

    fn draw_panel_right(&self, p: &Painter) {
        let l = &self.layout;
        p.fill(&l.right_panel, K_SURFACE1);
        p.round_border(&l.right_panel, K_HAIRLINE, 0);
        p.fill(&l.right_section, K_SURFACE2);
        p.text(
            &rect(l.right_section.left + K_PAD_SECTION, l.right_section.top, 100, K_SECTION_H),
            "变更记录",
            K_TEXT_PRIMARY,
            crate::paint::FontKind::Section,
            crate::paint::Align::LeftV,
        );
        draw_badge(p, &l.txt_added, &format!("新增 {}", self.state.added.len()), K_ACCENT_TINT, K_ACCENT_TEXT, K_ACCENT_BORDER);
        draw_badge(p, &l.txt_removed, &format!("移除 {}", self.state.removed.len()), K_DANGER_TINT, K_DANGER, K_DANGER_BORDER);
        p.hline(l.right_section.left, l.right_section.right, l.right_section.top + K_SECTION_H, K_HAIRLINE);
        draw_table(
            p,
            &l.right_table,
            &widgets::COLS_CHANGED,
            &self.state.changed,
            &self.state.changed_kinds,
            self.state.sel_changed,
            self.scroll_changed,
            self.state.sort_col_changed,
            self.state.sort_asc_changed,
            true,
            &self.hover_changed,
        );
    }

    fn btn_state(&self, b: ButtonId) -> BtnState {
        if self.press_btn == Some(b) {
            BtnState::Press
        } else if self.hover_btn == Some(b) {
            BtnState::Hover
        } else {
            BtnState::Normal
        }
    }

    // ---- 定时器：排空扫描结果 + 自动刷新 ----

    fn on_timer(&mut self) {
        while let Ok(result) = self.rx.try_recv() {
            self.state.scanning = false;
            self.state.apply_scan(&result);
            // 列表长度可能变化：钳制滚动偏移到新可见范围（防止滚动停在空区）
            self.clamp_scroll();
            self.update_status();
            self.invalidate();
        }
        if self.state.auto_refresh && !self.state.scanning && !self.closing {
            self.spawn_scan();
        }
    }

    /// 数据变化后重算两表最大滚动偏移并钳制（设备数变少时防止停留在空区）。
    fn clamp_scroll(&mut self) {
        self.scroll_all = clamp_table_scroll(&self.layout.left_table, self.state.all.len(), self.scroll_all);
        self.scroll_changed = clamp_table_scroll(&self.layout.right_table, self.state.changed.len(), self.scroll_changed);
    }

    fn on_device_change(&mut self) {
        // 必须同时看 auto_refresh："停止刷新"=冻结自动识别（对齐旧版注释）
        if self.state.auto_refresh && !self.state.scanning && !self.closing {
            self.spawn_scan();
        }
    }

    // ---- 扫描线程模型 ----

    fn spawn_scan(&mut self) {
        if self.state.scanning || self.closing {
            return;
        }
        self.state.scanning = true;
        // 旧快照（UI 线程克隆；worker 只读它，绝不读 App/窗口 —— 无 UAF 可能）
        let old = self.state.all.clone();
        let provider = self.provider.clone();
        let tx = self.tx.clone();
        let handle = std::thread::spawn(move || {
            let all = provider.scan();
            let d = vpid_core::comparer::diff(&old, &all);
            let _ = tx.send(ScanResult {
                all,
                added: d.added,
                removed: d.removed,
            });
        });
        self.scan_join = Some(handle);
    }

    fn update_status(&mut self) {
        self.status_a = format!("已连接 USB 设备：{} 台", self.state.all.len());
        self.status_b = if self.state.auto_refresh {
            "自动刷新中".to_string()
        } else {
            "监控已暂停".to_string()
        };
    }

    // ---- 输入 ----

    fn button_at(&self, x: i32, y: i32) -> Option<ButtonId> {
        let l = &self.layout;
        let hit = |r: &RECT| x >= r.left && x < r.right && y >= r.top && y < r.bottom;
        if hit(&l.btn_stop) {
            Some(ButtonId::Stop)
        } else if hit(&l.btn_resume) {
            Some(ButtonId::Resume)
        } else if hit(&l.btn_baseline) {
            Some(ButtonId::Baseline)
        } else if hit(&l.btn_copy) {
            Some(ButtonId::Copy)
        } else if hit(&l.btn_refresh) {
            Some(ButtonId::Refresh)
        } else if hit(&l.btn_exit) {
            Some(ButtonId::Exit)
        } else {
            None
        }
    }

    fn on_lbutton_down(&mut self, x: i32, y: i32) {
        if let Some(b) = self.button_at(x, y) {
            self.press_btn = Some(b);
            self.invalidate();
            return;
        }
        // 表格：表头排序（记录按下列以显示三态）/ 行选中（互斥）
        if let Some(col) = widgets::hit_header(&widgets::COLS_ALL, &self.layout.left_table, x, y) {
            self.hover_all.press_col = Some(col);
            self.state.sort_all(col);
            self.invalidate();
            return;
        }
        if let Some(col) = widgets::hit_header(&widgets::COLS_CHANGED, &self.layout.right_table, x, y) {
            self.hover_changed.press_col = Some(col);
            self.state.sort_changed(col);
            self.invalidate();
            return;
        }
        if let Some(row) = widgets::hit_row(&self.layout.left_table, y, self.scroll_all) {
            let row = if row < self.state.all.len() { Some(row) } else { None };
            self.state.select_all(row);
            self.invalidate();
            return;
        }
        if let Some(row) = widgets::hit_row(&self.layout.right_table, y, self.scroll_changed) {
            let row = if row < self.state.changed.len() { Some(row) } else { None };
            self.state.select_changed(row);
            self.invalidate();
        }
    }

    fn on_lbutton_up(&mut self, x: i32, y: i32) {
        if let Some(pressed) = self.press_btn {
            self.press_btn = None;
            if self.button_at(x, y) == Some(pressed) {
                self.dispatch_command(pressed);
            }
            self.invalidate();
        }
        // 表头按下态在抬起时清除（三态回到 hover/normal）
        if self.hover_all.press_col.is_some() || self.hover_changed.press_col.is_some() {
            self.hover_all.press_col = None;
            self.hover_changed.press_col = None;
            self.invalidate();
        }
    }

    fn on_mouse_move(&mut self, x: i32, y: i32) {
        let new_hover = self.button_at(x, y);
        if new_hover != self.hover_btn {
            self.hover_btn = new_hover;
            self.invalidate();
        }
        if self.update_table_hover(x, y) {
            self.invalidate();
        }
    }

    /// 更新两表的表头列悬停 + 数据行悬停（hotTrack）。返回是否有变化。
    fn update_table_hover(&mut self, x: i32, y: i32) -> bool {
        let mut changed = false;
        let hc = widgets::hit_header(&widgets::COLS_ALL, &self.layout.left_table, x, y);
        if hc != self.hover_all.header_col {
            self.hover_all.header_col = hc;
            changed = true;
        }
        let row = widgets::hit_row(&self.layout.left_table, y, self.scroll_all)
            .filter(|r| *r < self.state.all.len());
        if row != self.hover_all.row {
            self.hover_all.row = row;
            changed = true;
        }
        let hc = widgets::hit_header(&widgets::COLS_CHANGED, &self.layout.right_table, x, y);
        if hc != self.hover_changed.header_col {
            self.hover_changed.header_col = hc;
            changed = true;
        }
        let row = widgets::hit_row(&self.layout.right_table, y, self.scroll_changed)
            .filter(|r| *r < self.state.changed.len());
        if row != self.hover_changed.row {
            self.hover_changed.row = row;
            changed = true;
        }
        changed
    }

    fn on_mouse_wheel(&mut self, wparam: WPARAM) {
        // 骨架阶段：滚轮仅滚动"全部设备"表（无光标 x，简化处理）。
        let delta = (wparam >> 16) as i16 as i32; // 120 = 一格
        if delta == 0 {
            return;
        }
        let table = &self.layout.left_table;
        self.scroll_all = clamp_table_scroll(table, self.state.all.len(), self.scroll_all - (delta / 120) * K_ROW_H);
        self.invalidate();
    }

    fn dispatch_command(&mut self, b: ButtonId) {
        let cmd = match b {
            ButtonId::Stop => Command::StopRefresh,
            ButtonId::Resume => Command::ResumeRefresh,
            ButtonId::Baseline => Command::SetBaseline,
            ButtonId::Copy => Command::CopySelected,
            ButtonId::Refresh => Command::RefreshNow,
            ButtonId::Exit => Command::Exit,
        };
        match cmd {
            Command::StopRefresh => self.state.stop_refresh(),
            Command::ResumeRefresh => {
                self.state.resume_refresh();
                self.spawn_scan();
            }
            Command::RefreshNow => self.spawn_scan(),
            Command::SetBaseline => self.state.set_baseline(),
            Command::CopySelected => {
                if let Some(text) = self.state.copy_selected_text() {
                    copy_to_clipboard(self.hwnd, &text);
                }
            }
            Command::Exit => self.on_close(),
            _ => {}
        }
        self.update_status();
        self.invalidate();
    }

    // ---- 退出（确定性收尾）----

    fn on_close(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        // 1. 请求取消在途扫描
        self.provider.request_cancel();
        // 2. 阻塞到在途扫描返回（取消标志使其快速返回）
        if let Some(j) = self.scan_join.take() {
            let _ = j.join();
        }
        // 3. 停定时器 + 注销设备通知
        unsafe {
            KillTimer(self.hwnd, TIMER_AUTO);
        }
        self.notifier.stop();
        // 4. 结束消息循环（WM_DESTROY 会 PostQuitMessage）
        unsafe {
            PostQuitMessage(0);
        }
    }

    fn invalidate(&self) {
        unsafe {
            InvalidateRect(self.hwnd, null(), 1);
        }
    }
}

// ---------------- 剪贴板 ----------------

const CF_UNICODETEXT: u32 = 13;

fn copy_to_clipboard(hwnd: HWND, text: &str) {
    unsafe {
        if OpenClipboard(hwnd) == 0 {
            return;
        }
        EmptyClipboard();
        let mut w: Vec<u16> = text.encode_utf16().collect();
        w.push(0);
        let bytes = w.len() * 2;
        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if !hmem.is_null() {
            let dst = GlobalLock(hmem);
            if !dst.is_null() {
                std::ptr::copy_nonoverlapping(w.as_ptr(), dst as *mut u16, w.len());
                GlobalUnlock(hmem);
            }
            SetClipboardData(CF_UNICODETEXT, hmem);
            // SetClipboardData 接管所有权；失败才需释放
        }
        CloseClipboard();
    }
}
