//! Linux GTK3 应用：窗口 + 顶部按钮行 + 左右双表 + 底部状态栏。
//!
//! 线程模型（镜像 win32）：UI 状态只在主线程读写；扫描/比对在后台线程。
//! - `glib::timeout_add_local(100ms)` 排空 `mpsc::Receiver` 并对结果 `state.apply_scan` 后重刷两表；
//! - `auto_refresh && !scanning` 时后台线程 `provider.scan()` + `comparer::diff` 经 `mpsc` 回投；
//! - 按钮/表头/选中信号 → `Command`；复制 → `gtk::Clipboard`；关闭 → 请求取消 + join + 移除 timeout。

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use gtk::prelude::*;
use gtk::gdk;
use gtk::glib;
use gtk::{
    Box as GtkBox, Button, Label, ListStore, Orientation, ScrolledWindow, TreeView,
    TreeViewColumn, Window, WindowType,
};

use vpid_ui_core::app::{AppState, ScanResult};
use vpid_ui_core::command::Command;
use vpid_ui_core::platform::{DeviceProvider, Platform, PlatformError};
use vpid_ui_core::theme::*;

/// 列定义（本 crate 局部定义，不移动 win32 的列定义到 ui-core）。
/// (标题, 首选宽)。名称/序列号为 flex 列，在 win32 中拉宽；GTK 表格自动分配列宽，此处近似。
const COLS_ALL: [(&str, i32); 4] = [("VID", 70), ("PID", 70), ("名称", 260), ("序列号", 200)];
const COLS_CHANGED: [(&str, i32); 4] = [("状态", 60), ("VID", 70), ("PID", 70), ("名称", 260)];

/// 按钮语义（决定 CSS class，近似 win32 四类配色）。
#[derive(Clone, Copy)]
enum ButtonKind {
    Primary,
    Accent,
    Danger,
    Ghost,
}

impl ButtonKind {
    fn css_class(self) -> &'static str {
        match self {
            ButtonKind::Primary => "k-primary",
            ButtonKind::Accent => "k-accent",
            ButtonKind::Danger => "k-danger",
            ButtonKind::Ghost => "k-ghost",
        }
    }
}

/// UI 线程上下文（GTK 对象非 Send；仅主线程访问）。
struct Ctx {
    state: AppState,
    provider: Arc<dyn DeviceProvider>,
    window: Option<Window>,
    tx: Sender<ScanResult>,
    rx: Receiver<ScanResult>,
    scan_join: Option<JoinHandle<()>>,
    closing: bool,
    suppress_select: bool,
    source: Option<glib::SourceId>,
    model_all: Option<ListStore>,
    model_changed: Option<ListStore>,
    view_all: Option<TreeView>,
    view_changed: Option<TreeView>,
    cols_all: Vec<TreeViewColumn>,
    cols_changed: Vec<TreeViewColumn>,
    status_all: Option<Label>,
    status_mode: Option<Label>,
}

/// 平台后端（Linux GTK3 实现 `Platform` trait）。
pub struct LinuxApp;

impl Platform for LinuxApp {
    fn run(self, state: AppState, provider: Arc<dyn DeviceProvider>) -> Result<(), PlatformError> {
        gtk::init().map_err(|e| format!("gtk::init 失败: {e}"))?;
        install_css();

        let (tx, rx) = channel::<ScanResult>();
        let ctx = Rc::new(RefCell::new(Ctx {
            state,
            provider,
            window: None,
            tx,
            rx,
            scan_join: None,
            closing: false,
            suppress_select: false,
            source: None,
            model_all: None,
            model_changed: None,
            view_all: None,
            view_changed: None,
            cols_all: Vec::new(),
            cols_changed: Vec::new(),
            status_all: None,
            status_mode: None,
        }));

        let source = build_ui(&ctx);
        ctx.borrow_mut().source = Some(source);
        {
            let mut c = ctx.borrow_mut();
            c.refresh_views();
            c.update_status();
            spawn_scan(&mut c);
        }
        gtk::main();
        Ok(())
    }
}

/// 构建窗口并返回轮询定时器 SourceId。
fn build_ui(ctx: &Rc<RefCell<Ctx>>) -> glib::SourceId {
    let window = Window::new(WindowType::Toplevel);
    window.set_title(K_APP_NAME);
    window.set_default_size(K_DEFAULT_WINDOW_WIDTH, K_DEFAULT_WINDOW_HEIGHT);
    window.set_border_width(0);

    let root = GtkBox::new(Orientation::Vertical, 0);

    // ---- 顶部按钮行 ----
    let btn_bar = GtkBox::new(Orientation::Horizontal, K_PAD_CTRL);
    btn_bar.set_margin_top(K_PAD_OUTER);
    btn_bar.set_margin_bottom(K_PAD_OUTER);
    btn_bar.set_margin_start(K_PAD_OUTER);
    btn_bar.set_margin_end(K_PAD_OUTER);
    let btns: [(&str, ButtonKind); 6] = [
        ("停止刷新", ButtonKind::Ghost),
        ("恢复刷新", ButtonKind::Accent),
        ("重置基准", ButtonKind::Ghost),
        ("复制", ButtonKind::Accent),
        ("刷新", ButtonKind::Primary),
        ("退出", ButtonKind::Danger),
    ];
    for (label, kind) in btns {
        let b = Button::with_label(label);
        b.style_context().add_class(kind.css_class());
        let cmd = match label {
            "停止刷新" => Command::StopRefresh,
            "恢复刷新" => Command::ResumeRefresh,
            "重置基准" => Command::SetBaseline,
            "复制" => Command::CopySelected,
            "刷新" => Command::RefreshNow,
            _ => Command::Exit,
        };
        let ctx = ctx.clone();
        b.connect_clicked(move |_| dispatch_command(&ctx, cmd.clone()));
        btn_bar.pack_start(&b, false, false, 0);
    }
    root.pack_start(&btn_bar, false, false, 0);

    // ---- 中部：左右双表 ----
    let main_row = GtkBox::new(Orientation::Horizontal, K_PAD_GAP);
    main_row.set_margin_start(K_PAD_OUTER);
    main_row.set_margin_end(K_PAD_OUTER);
    main_row.set_margin_bottom(K_PAD_OUTER);
    main_row.set_vexpand(true);

    let (left, view_all, model_all, cols_all) = make_panel("全部设备", &COLS_ALL, ctx.clone(), true);
    let (right, view_changed, model_changed, cols_changed) =
        make_panel("变更记录", &COLS_CHANGED, ctx.clone(), false);
    main_row.pack_start(&left, true, true, 0);
    main_row.pack_start(&right, true, true, 0);
    root.pack_start(&main_row, true, true, 0);

    // ---- 底部状态栏 ----
    let status = GtkBox::new(Orientation::Horizontal, K_PAD_GAP);
    status.set_margin_start(K_PAD_OUTER);
    status.set_margin_end(K_PAD_OUTER);
    status.set_margin_bottom(K_PAD_OUTER);
    let status_all = Label::new(None);
    status_all.set_xalign(0.0);
    let status_mode = Label::new(None);
    status_mode.set_xalign(1.0);
    status.pack_start(&status_all, true, true, 0);
    status.pack_start(&status_mode, false, false, 0);
    root.pack_start(&status, false, false, 0);

    window.add(&root);
    window.show_all();

    // 关闭：请求取消 + join 在途扫描 + 移除 timeout + 退出主循环
    {
        let ctx = ctx.clone();
        window.connect_delete_event(move |_, _| {
            close_app(&ctx);
            glib::Propagation::Proceed
        });
    }

    // 表头点击 → 排序
    for (i, col) in cols_all.iter().enumerate() {
        let ctx = ctx.clone();
        let i = i;
        col.connect_clicked(move |_| dispatch_command(&ctx, Command::SortAll { col: i }));
    }
    for (i, col) in cols_changed.iter().enumerate() {
        let ctx = ctx.clone();
        let i = i;
        col.connect_clicked(move |_| dispatch_command(&ctx, Command::SortChanged { col: i }));
    }

    // 选中变化 → 命令
    {
        let ctx_all = ctx.clone();
        view_all
            .selection()
            .connect_changed(move |sel| on_selected(&ctx_all, sel, true));
        let ctx_changed = ctx.clone();
        view_changed
            .selection()
            .connect_changed(move |sel| on_selected(&ctx_changed, sel, false));
    }

    {
        let mut c = ctx.borrow_mut();
        c.model_all = Some(model_all);
        c.model_changed = Some(model_changed);
        c.view_all = Some(view_all);
        c.view_changed = Some(view_changed);
        c.cols_all = cols_all;
        c.cols_changed = cols_changed;
        c.window = Some(window);
        c.status_all = Some(status_all);
        c.status_mode = Some(status_mode);
    }

    // 100ms 轮询：排空扫描结果 + 按需触发下一次扫描
    let ctx2 = ctx.clone();
    glib::timeout_add_local(
        Duration::from_millis(K_AUTO_REFRESH_INTERVAL_MS as u64),
        move || {
            let mut c = ctx2.borrow_mut();
            if c.closing {
                return glib::ControlFlow::Break;
            }
            while let Ok(result) = c.rx.try_recv() {
                c.state.scanning = false;
                c.state.apply_scan(&result);
                c.refresh_views();
                c.update_status();
            }
            if c.state.auto_refresh && !c.state.scanning {
                spawn_scan(&mut c);
            }
            glib::ControlFlow::Continue
        },
    )
}

/// 构建一个带标题面板：标题 + 可滚动 TreeView，返回 (panel, view, store, cols)。
fn make_panel(
    title: &str,
    cols: &[(&str, i32)],
    ctx: Rc<RefCell<Ctx>>,
    is_all: bool,
) -> (GtkBox, TreeView, ListStore, Vec<TreeViewColumn>) {
    let panel = GtkBox::new(Orientation::Vertical, K_PAD_SECTION);
    let lbl = Label::new(Some(title));
    lbl.set_xalign(0.0);
    lbl.set_margin_top(K_PAD_SECTION);
    lbl.set_margin_bottom(4);
    lbl.style_context().add_class("k-section");
    panel.pack_start(&lbl, false, false, 0);

    let store = ListStore::new(&[glib::Type::STRING; 4]);
    let view = TreeView::new();
    view.set_model(Some(&store));
    let mut gcols = Vec::new();
    for (i, (t, _w)) in cols.iter().enumerate() {
        let col = TreeViewColumn::new();
        let renderer = gtk::CellRendererText::new();
        gtk::prelude::CellLayoutExt::pack_start(&col, &renderer, true);
        gtk::prelude::TreeViewColumnExt::add_attribute(&col, &renderer, "text", i as i32);
        col.set_title(t);
        view.append_column(&col);
        gcols.push(col);
    }
    let scroll = ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    scroll.add(&view);
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    panel.pack_start(&scroll, true, true, 0);

    // 选中变化在 build_ui 末尾统一连接（需要 view 引用），此处仅返回组件。
    let _ = (ctx, is_all);
    (panel, view, store, gcols)
}

fn on_selected(ctx: &Rc<RefCell<Ctx>>, sel: &gtk::TreeSelection, is_all: bool) {
    let mut c = ctx.borrow_mut();
    if c.suppress_select {
        return;
    }
    let row = selected_row(sel);
    if is_all {
        c.state.select_all(row);
        if let Some(v) = c.view_changed.as_ref() {
            v.selection().unselect_all();
        }
    } else {
        c.state.select_changed(row);
        if let Some(v) = c.view_all.as_ref() {
            v.selection().unselect_all();
        }
    }
}

/// 当前选中行索引（无选中 → None）。
fn selected_row(sel: &gtk::TreeSelection) -> Option<usize> {
    let (model, iter) = sel.selected()?;
    let mut path = model.path(&iter)?;
    path.indices_with_depth().first().copied().map(|i| i as usize)
}

fn dispatch_command(ctx: &Rc<RefCell<Ctx>>, cmd: Command) {
    if cmd == Command::Exit {
        close_app(ctx);
        return;
    }
    let mut c = ctx.borrow_mut();
    match cmd {
        Command::StopRefresh => c.state.stop_refresh(),
        Command::ResumeRefresh => c.state.resume_refresh(),
        Command::RefreshNow => {}
        Command::SetBaseline => c.state.set_baseline(),
        Command::CopySelected => {
            if let Some(text) = c.state.copy_selected_text() {
                copy_to_clipboard(&text);
            }
        }
        Command::SortAll { col } => c.state.sort_all(col),
        Command::SortChanged { col } => c.state.sort_changed(col),
        Command::SelectAll { row } => c.state.select_all(row),
        Command::SelectChanged { row } => c.state.select_changed(row),
        Command::Exit => unreachable!(),
    }
    if matches!(cmd, Command::RefreshNow | Command::ResumeRefresh) {
        spawn_scan(&mut c);
    }
    c.refresh_views();
    c.update_status();
}

/// 启动一次后台扫描（无并发；`state.scanning` 互斥）。
fn spawn_scan(c: &mut Ctx) {
    if c.state.scanning || c.closing {
        return;
    }
    c.state.scanning = true;
    let old = c.state.all.clone();
    let provider = c.provider.clone();
    let tx = c.tx.clone();
    let handle = std::thread::spawn(move || {
        let all = provider.scan();
        let d = vpid_core::comparer::diff(&old, &all);
        let _ = tx.send(ScanResult {
            all,
            added: d.added,
            removed: d.removed,
        });
    });
    c.scan_join = Some(handle);
}

/// 关闭应用：请求取消 + join 在途扫描 + 移除 timeout + 退出主循环（幂等）。
fn close_app(ctx: &Rc<RefCell<Ctx>>) {
    {
        let mut c = ctx.borrow_mut();
        if c.closing {
            return;
        }
        c.closing = true;
        c.provider.request_cancel();
        if let Some(j) = c.scan_join.take() {
            let _ = j.join();
        }
        if let Some(s) = c.source.take() {
            let _ = s.remove();
        }
    }
    gtk::main_quit();
}

fn copy_to_clipboard(text: &str) {
    gtk::Clipboard::get(&gdk::Atom::intern("CLIPBOARD")).set_text(text);
}

impl Ctx {
    /// 重刷两表（清空重建 + 恢复选中 + 更新排序指示）。
    fn refresh_views(&mut self) {
        self.suppress_select = true;
        if let Some(model) = self.model_all.as_ref() {
            model.clear();
            for d in &self.state.all {
                let it = model.append();
                let v0 = d.formatted_vid().to_value();
                let v1 = d.formatted_pid().to_value();
                let v2 = d.display_name().to_value();
                let v3 = d.serial.to_value();
                model.set(&it, &[(0, &v0), (1, &v1), (2, &v2), (3, &v3)]);
            }
            if let (Some(view), Some(row)) = (self.view_all.as_ref(), self.state.sel_all) {
                if let Some(iter) = model.iter_nth_child(None, row as i32) {
                    view.selection().select_iter(&iter);
                }
            }
        }
        if let Some(model) = self.model_changed.as_ref() {
            model.clear();
            for (i, d) in self.state.changed.iter().enumerate() {
                let it = model.append();
                let kind = self
                    .state
                    .changed_kinds
                    .get(i)
                    .copied()
                    .unwrap_or(vpid_ui_core::model::RowKind::None);
                let v0 = kind.label().to_value();
                let v1 = d.formatted_vid().to_value();
                let v2 = d.formatted_pid().to_value();
                let v3 = d.display_name().to_value();
                model.set(&it, &[(0, &v0), (1, &v1), (2, &v2), (3, &v3)]);
            }
            if let (Some(view), Some(row)) = (self.view_changed.as_ref(), self.state.sel_changed) {
                if let Some(iter) = model.iter_nth_child(None, row as i32) {
                    view.selection().select_iter(&iter);
                }
            }
        }
        self.update_sort_indicators();
        self.suppress_select = false;
    }

    fn update_sort_indicators(&mut self) {
        for (i, col) in self.cols_all.iter().enumerate() {
            let active = self.state.sort_col_all == i as i32;
            col.set_sort_indicator(active);
            if active {
                col.set_sort_order(if self.state.sort_asc_all {
                    gtk::SortType::Ascending
                } else {
                    gtk::SortType::Descending
                });
            }
        }
        for (i, col) in self.cols_changed.iter().enumerate() {
            let active = self.state.sort_col_changed == i as i32;
            col.set_sort_indicator(active);
            if active {
                col.set_sort_order(if self.state.sort_asc_changed {
                    gtk::SortType::Ascending
                } else {
                    gtk::SortType::Descending
                });
            }
        }
    }

    fn update_status(&mut self) {
        if let Some(l) = self.status_all.as_ref() {
            l.set_text(&format!("已连接 USB 设备：{} 台", self.state.all.len()));
        }
        if let Some(l) = self.status_mode.as_ref() {
            l.set_text(if self.state.auto_refresh {
                "自动刷新中"
            } else {
                "监控已暂停"
            });
        }
    }
}

/// 近似 win32 四类按钮配色（浅色主题，近似而非逐像素）。
fn install_css() {
    let css = r#"
        button.k-primary { background: #1e5094; color: #ffffff; border: none; }
        button.k-primary:hover { background: #16427c; }
        button.k-accent { background: #e9f0fa; color: #1a4a8a; border: 1px solid #a3bee1; }
        button.k-accent:hover { background: #d8e6f6; }
        button.k-danger { background: #fbeaeb; color: #b02a30; border: 1px solid #e4afb2; }
        button.k-danger:hover { background: #f7dbdc; }
        button.k-ghost { background: #f2f4f9; color: #181b21; border: 1px solid #868d9b; }
        button.k-ghost:hover { background: #e6e9f0; }
        label.k-section { font-weight: bold; color: #181b21; }
    "#;
    let provider = gtk::CssProvider::new();
    let _ = provider.load_from_data(css.as_bytes());
    if let Some(screen) = gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
