# v2.0.0 — 切换到纯 Rust + Win32 原生 UI

**这是从 C++/SOUI5 到纯 Rust 的主版本跃升。** UI/UX 完全沿用主线（布局 66/34 双栏 +
应用栏 + 状态栏、冷色商务风主题、交互 1:1 复刻），核心从 C++ 重写为 Rust。

## 核心变化

- **纯 Rust**：`vpid-core`（扫描/比对/通知）+ `vpid-ui-core`（平台无关 UI 状态机/主题/
  `Platform` trait）+ `vpid-ui-win32`（Win32 原生 UI）。**无 C++ 参与构建**。
- **运行效率与稳定性**：Rust 所有权模型根治跨线程 UAF/悬垂指针；原生 Win32 直绘
  （零框架开销）；`panic = "abort"` 确定性退出。
- **原 C++/SOUI5 实现已移除**（历史见 git 记录与 RELEASE_NOTES_v1.x）。

## UI/UX 增强（复刻 + 优化）

- **Per-Monitor V2 DPI**：多显示器各自缩放自适应（GetDpiForWindow/96，拖动跨屏实时重建
  字体/布局），跨电脑不模糊、不写死缩放。
- **固定表头**：滚动时表头钉在顶部，数据行从其下滚过（clip_rect 裁剪）。
- **列宽自适应**：列宽总和精确填满表格，消除表头右侧空白；短码列（VID/PID/状态）保持
  紧凑，长文本列（名称/序列号）吸宽。
- **表头三态**（normal/hover/pushdown）+ 行 hotTrack + 左对齐缩进。
- **首扫基线**：首扫只建基准、清空变更记录，杜绝"打开时闪现全部设备到变更栏"。

## 平台

- **Windows x64（Win7+）**：`vpid_viewer_windows_x64.exe`，~0.26 MB。
  **不承诺 XP**（Rust MSVC 下限；XP 兼容由旧 C++ v1.x 承担）。
- **Linux GTK3**：后端开发中（`vpid-ui-core::Platform` 插槽已就绪，零改动核心）。
- 发布硬化：VERSIONINFO（含 BuildDate）+ 清单（asInvoker/comctl v6/PerMonitorV2）+
  图标 + `editbin /RELEASE` 校验和，`verify_release.py` 8 项审计全过。

## 发布产物

| 平台 | 架构 | 产物 | 体积 |
|------|------|------|------|
| Windows | x64 | `vpid_viewer_windows_x64.exe` | ~0.26 MB |
