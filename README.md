# vpid — USB 设备 ID 查看器

用 **Rust + Win32 原生 API** 实现（v2.0.0 起为唯一主线）。原 C++/SOUI5 实现已移除，
核心收益 = Rust 所有权模型（根治跨线程 UAF/悬垂指针）+ 原生 Win32 直绘（零框架开销）
带来的 **运行效率与稳定性**。UI/UX 完全沿用原主线：布局（66/34 双栏 + 应用栏 + 状态栏）、
冷色商务风主题、交互 1:1 复刻。

## 特性

- **实时监控** USB 设备插拔（SetupAPI + 注册表兜底），自动比对增/删并记录变更。
- **双通道标识**：新增（蓝）/移除（红）行底晕 + 行首竖条；表头点击排序（▲/▼）。
- **固定表头**：滚动时表头钉在顶部，数据行从其下滚过。
- **Per-Monitor V2 DPI**：多显示器各自缩放自适应，跨电脑不模糊、不写死缩放。
- **监控暂停/恢复、重置基准、复制选中、手动刷新**。

## 目录结构

```
rust/
├── Cargo.toml                # workspace：4 个 crate + 统一 windows-sys 特性 + release profile
├── docs/PLATFORM_EXTENSION.md    # ★ 多平台 UI 插槽扩展指南（新增 Linux/macOS 后端）
├── crates/
│   ├── vpid-core/            # 业务核心：设备模型 / 扫描器(SetupAPI+注册表) / 比对器 / 通知器
│   ├── vpid-ui-core/         # 平台无关 UI 核心：状态机 / 视图模型 / 主题 / Platform trait
│   ├── vpid-ui-win32/        # Windows 原生 UI：消息循环 / 窗口 / GDI 自绘 / 控件
│   └── vpid-ui-linux/        # Linux 原生 UI：GTK3 窗口 / 按钮 / 列表 / 状态栏
└── target/                   # 构建产物（release: vpid-viewer.exe / vpid-viewer-linux）
```

| crate | 职责 | 平台 |
|---|---|---|
| `vpid-core` | 设备模型、扫描（SetupAPI 主路径 + 注册表兜底）、diff 比对、设备插拔通知 | Windows |
| `vpid-ui-core` | UI 状态机（扫描→视图更新/排序/选中/基准/复制）、主题常量、`Platform`/`DeviceProvider` trait | 平台无关 |
| `vpid-ui-win32` | Win32 消息循环、`WndProc`、GDI 双缓冲自绘、按钮/列表控件、扫描线程模型 | Windows |
| `vpid-ui-linux` | GTK3 窗口/按钮/列表/状态栏、sysfs 枚举扫描、100ms 轮询线程模型 | Linux |

## 构建与运行

```bash
cd rust
cargo build --release              # 产物 target/release/vpid-viewer.exe
target/release/vpid-viewer.exe     # 启动 UI（真实 USB 扫描）
target/release/vpid-viewer.exe --demo   # 演示桩数据（验证渲染/交互）
target/release/vpid-viewer.exe --scan   # CLI 自检：打印设备列表后退出
cargo test                         # 行为等价性单元测试
```

### Linux GTK3 构建与运行（容器 / WSL）

GTK3 是 C 绑定，需在 Linux 环境编译（Windows 主机无法编 GTK）。推荐用 Docker / WSL 容器（`wslc`）构建：

```bash
# 容器内（挂载仓库后）
cd /vpid/rust
cargo build -p vpid-ui-linux            # 产物 target/debug/vpid-viewer-linux
target/debug/vpid-viewer-linux --demo   # 演示桩数据（验证渲染/交互）
target/debug/vpid-viewer-linux --scan   # CLI 自检：打印设备列表后退出
cargo test -p vpid-core -p vpid-ui-linux
```

## 发布（v2.0.0 起）

```bash
python rust/scripts/build_release.py          # 构建 + 复制 dist/vpid_viewer_windows_x64.exe + 硬化 + 8 项审计
python rust/scripts/release_github.py --tag v2.0.0 --title "v2.0.0" --notes-file RELEASE_NOTES_v2.0.0.md
```

发布硬化（参考 skill `desktop-app-build`）：winres 生成 VERSIONINFO（含 BuildDate）+ 清单
（asInvoker/comctl v6/PerMonitorV2）+ 图标；`editbin /RELEASE` 写校验和；`verify_release.py`
8 项审计（导出表空/.rsrc/清单/版本/图标/校验和/子系统）。

## 多平台扩展

新增 Linux / macOS 后端只需实现 `vpid-ui-core::platform::Platform` + `DeviceProvider`，
零改动核心。Linux GTK3 后端已实现（`vpid-ui-linux`）。见 `rust/docs/PLATFORM_EXTENSION.md`。

## 关键约束

- **纯 Rust**：无 C++ 参与构建。
- **目标平台 Windows**（Win7+）；**不承诺 XP**（Rust MSVC 下限；XP 兼容由旧 C++ v1.x 承担，
  历史见 git 记录与 RELEASE_NOTES_v1.x）。
- 发布用 `panic = "abort"`（确定性退出，不 unwind 跨 `extern "system"` 边界）。
