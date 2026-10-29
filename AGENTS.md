# Repository Guidelines

## 项目结构与模块组织

- `rust/`：唯一主线（v2.0.0 起为纯 Rust + Win32，不再包含 C++/SOUI 实现）。
- `rust/crates/vpid-core/`：设备模型、SetupAPI/注册表扫描、diff 与插拔通知。
- `rust/crates/vpid-ui-core/`：平台无关的状态机、视图模型、主题及 `Platform`/`DeviceProvider` 抽象。
- `rust/crates/vpid-ui-win32/`：Win32 消息循环、窗口、GDI 绘制和控件，生成 `vpid-viewer.exe`。
- `rust/crates/vpid-ui-linux/`：GTK3 Linux 后端（`Platform`/`DeviceProvider` 实现），生成 `vpid-viewer-linux`；Windows 上编译为空占位，不改 `vpid-ui-core` 状态机。
- `rust/docs/PLATFORM_EXTENSION.md`：新增 Linux/macOS 等平台后端的扩展指南。
- `rust/scripts/`：发布构建、PE 硬化与验证脚本；`assets/`：图标等静态资源；`dist/`：发布产物（已忽略）。
- 测试位于 `rust/crates/*/tests/` 及源码内的 `#[cfg(test)]` 模块。

## 构建、测试与开发命令

```powershell
cd rust
cargo build --release          # 产物：rust/target/release/vpid-viewer.exe
cargo test --workspace         # 运行全部行为等价性测试
cargo run --release -- --demo  # 演示数据，验证渲染与交互
cargo run --release -- --scan  # CLI 扫描自检
```

Linux 后端在 Windows 主机上无法编译（GTK3 是 C 绑定，需完整 Linux sysroot），用 wslc 管理的 Docker 容器构建（单例，一次准备永久复用，不反复创建/删除）：

```powershell
# 容器 rust:1.85 + libgtk-3-dev + xvfb；源码先 tar 同步到容器原生盘再编（禁在 bind-mount 上编）
wslc exec vpid-linux-build bash -lc 'cd /build/rust && CARGO_TARGET_DIR=/build/target cargo build -p vpid-ui-linux && cargo test -p vpid-core -p vpid-ui-linux'
wslc exec vpid-linux-build bash -lc 'cd /build/rust && CARGO_TARGET_DIR=/build/target xvfb-run -a /build/target/debug/vpid-viewer-linux --demo'
```

发布构建从仓库根目录运行 `python rust/scripts/build_release.py`，自动执行 PE 硬化与 `verify_release.py` 的 8 项审计。需要 Rust 1.85+、Windows MSVC 工具链、Python 3；发布审计依赖 `pefile` 和 VS2022 的 `editbin`。

格式遵循默认 `rustfmt`；仓库尚未全量格式化，避免整仓 `cargo fmt` 产生的无关改动，仅格式化本次修改的文件。`clippy` 亦未清零，提交前确认改动未新增 warning。

## 编码风格与命名约定

- 使用 Rust 2021 和默认 `rustfmt`（4 空格缩进），不手工对齐代码。
- 模块/文件/函数使用 `snake_case`，类型使用 `CamelCase`，常量使用 `SCREAMING_SNAKE_CASE`。
- 平台无关逻辑放在 `vpid-ui-core`；Win32 调用仅放在 `vpid-ui-win32`。`unsafe` 块保持最小，并注明安全前提。
- 复用 workspace 的 `windows-sys` 特性；新增 Win7 之后才有的 API 时优先动态解析并提供降级路径。
- 注释解释"为什么"而非复述代码；涉及兼容性或 GDI 特殊行为时保留简短说明。

## 测试指南

- 使用 Rust 内置测试框架，重点覆盖 VID/PID/序列号解析、设备 diff、排序、选中及基准状态变化。
- 新测试命名为描述行为的 `snake_case`，例如 `extract_serial_third_segment`。
- UI 改动至少运行 `--demo`，并在可用时用 `rust/scripts/capture_demo.ps1` 或现有 `verify_*.ps1` 做截图验证。
- 当前未设覆盖率阈值；缺陷修复应补回归测试。

## 提交与 Pull Request 指南

- 提交信息采用 Conventional Commits：`feat(ui): ...`、`fix(core): ...`、`docs: ...`、`build: ...`、`chore: ...`，摘要可使用中文。
- PR 描述应说明行为变化、影响的 crate、验证命令及结果；UI 变化附截图；关联相关 issue。
- 发布使用语义化版本 `vX.Y.Z`，说明归档为 `RELEASE_NOTES_vX.Y.Z.md`，不要跳过发布审计。

## 架构与兼容性约束

- 保持 `vpid-core` 与 UI 后端解耦；新平台只实现 `Platform`/`DeviceProvider`，不复制状态机。
- 目标为 Windows 7+；不要承诺 XP 支持（历史 v1.x C++ 版本承担该能力）。
- 发布配置保持 `panic = "abort"`、体积优化与 PE 硬化；禁止使用 UPX。
- 清单必须保持纯 ASCII，避免系统代码页导致图标、DPI 或 `asInvoker` 声明失效。
- 任何 GUI 产物必须处理高 DPI（硬门槛，不可遗漏）：Win32 清单 `PerMonitorV2` + 代码侧 `GetProcAddress` 动态解析，scale 取 `GetDpiForWindow/96`、跨屏 `WM_DPICHANGED` 重建字体；Linux GTK 验 `GDK_SCALE=2`；禁止写死 1.5/2.0。
- Rust 输出目录统一 workspace 级 `target/`；跨容器/交叉构建固定 `CARGO_TARGET_DIR=<项目相对路径>`，禁止各 crate 各编各的 target。

## 安全与配置提示

- 不提交 `.env`、`.gh_token`、`dist/` 或任何 token；发布 token 仅通过环境变量或 Git credential manager 获取。
- 修改 `build.rs`、清单或发布脚本后必须重新运行完整发布构建，不能只验证 `cargo build`。
