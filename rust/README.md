# vpid — 纯 Rust 项目（refactor/rust-win32-ui 分支目标形态）

用 **Rust + Win32 原生 API** 重写 vpid（USB 设备 ID 查看器），UI/UX 完全沿用主线，
核心收益 = Rust 所有权模型（根治跨线程 UAF/悬垂指针）+ 原生 Win32 直绘（零框架开销）
带来的 **运行效率与稳定性**。

## 目录结构

```
rust/
├── Cargo.toml              # workspace：3 个 crate + 统一 windows-sys 特性 + release profile
├── docs/PLATFORM_EXTENSION.md   # ★ 多平台 UI 插槽扩展指南
├── crates/
│   ├── vpid-core/          # 业务核心：设备模型 / 扫描器(SetupAPI+注册表) / 比对器 / 通知器
│   ├── vpid-ui-core/       # 平台无关 UI 核心：状态机 / 视图模型 / 主题 / Platform trait
│   └── vpid-ui-win32/      # Windows 原生 UI：消息循环 / 窗口 / GDI 自绘 / 控件
└── target/                 # 构建产物（release: vpid-viewer.exe）
```

## 构建与运行

```bash
cd rust
cargo build --release              # 产物 target/release/vpid-viewer.exe
target/release/vpid-viewer.exe     # 启动 UI（真实 USB 扫描）
target/release/vpid-viewer.exe --demo   # 演示桩数据（验证渲染/交互）
target/release/vpid-viewer.exe --scan   # CLI 自检：打印设备列表后退出
cargo test                         # 行为等价性单元测试
```

## 分层职责

| crate | 职责 | 平台 |
|---|---|---|
| `vpid-core` | 设备模型、扫描（SetupAPI 主路径 + 注册表兜底）、diff 比对、设备插拔通知 | Windows |
| `vpid-ui-core` | UI 状态机（扫描→视图更新/排序/选中/基准/复制）、主题常量、`Platform`/`DeviceProvider` trait | 平台无关 |
| `vpid-ui-win32` | Win32 消息循环、`WndProc`、GDI 双缓冲自绘、按钮/列表控件、扫描线程模型 | Windows |

## 多平台扩展

新增 Linux / macOS 后端只需实现 `vpid-ui-core::platform::Platform` + `DeviceProvider`，
零改动核心。见 `docs/PLATFORM_EXTENSION.md`。

## 关键约束

- **纯 Rust**：无 C++ 参与构建。
- **UI/UX 沿用主线**：布局（66/34 双栏 + 应用栏 + 状态栏）、主题色、交互 1:1。
- **目标平台 Windows**（Win7+）；**不承诺 XP**（Rust MSVC 下限）。
- 发布用 `panic = "abort"`（确定性退出，不 unwind 跨 `extern "system"` 边界）。
