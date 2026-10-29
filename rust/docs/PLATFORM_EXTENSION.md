# 多平台 UI 兼容插槽扩展指南

本分支保留"多平台 UI 兼容的插槽"：**平台无关层 `vpid-ui-core` + `Platform`/`DeviceProvider`
trait**。新增 Linux / macOS 原生 UI 后端时，**零改动核心**，只需新增一个 crate 并实现两个 trait。

## 插槽抽象（`vpid-ui-core/src/platform.rs`）

```rust
// 平台无关 UI 入口：任何后端实现它即可被 `main` 启动
pub trait Platform {
    fn run(self, state: AppState, provider: Arc<dyn DeviceProvider>) -> Result<(), PlatformError>;
}

// 设备数据源：后端负责"怎么扫到设备 + 怎么比对"
pub trait DeviceProvider: Send + Sync {
    fn scan(&self) -> Vec<UsbDevice>;   // 阻塞扫描（可取消）
    fn request_cancel(&self);           // 退出时取消在途扫描
}
```

## 后端需提供的四件事

1. **实现 `Platform`**：创建窗口 + 跑平台消息循环（Win32: `GetMessageW`；X11: `XNextEvent`；
   Cocoa: `NSApplicationMain`）。
2. **输入 → `Command`**：把平台事件翻译成 `vpid_ui_core::command::Command`
   （停止/恢复/刷新/基准/复制/退出/排序/选中）。
3. **渲染 `ViewUpdate`**：消费 `AppState::apply_scan` 产出的视图更新（数据 + 排序 + 选中），
   用平台绘图 API 画出。主题色/尺寸来自 `vpid-ui-core::theme`（平台无关 `Color{r,g,b}`）。
4. **实现 `DeviceProvider`**：平台扫描器（Linux 用 libudev monitor + sysfs 枚举；
   macOS 用 IOKit `IOServiceMatching("IOUSBDevice")`）。比对复用 `vpid-core::comparer::diff`。

## 最小新增步骤（以 Linux 为例）

```bash
# 1. 新增 crate（workspace members 注册）
crates/vpid-ui-linux/Cargo.toml        # deps: vpid-core, vpid-ui-core, x11 等
crates/vpid-ui-linux/src/lib.rs        # impl Platform for LinuxApp
crates/vpid-ui-linux/src/main.rs       # 入口（与 vpid-ui-win32 同构）
# 2. 实现 Platform + DeviceProvider（Linux 扫描器）
# 3. 注册到 workspace：rust/Cargo.toml members += "crates/vpid-ui-linux"
```

## 平台差异注意

| 维度 | Windows（已实现） | Linux（待） | macOS（待） |
|---|---|---|---|
| 窗口/事件 | Win32 消息循环 | X11/Wayland | Cocoa/AppKit |
| 自绘 | GDI 双缓冲 | Cairo | CoreGraphics |
| 字体 | 微软雅黑（`CreateFontW`，字号随 DPI 缩放） | fontconfig `sans-serif` | 系统字体 |
| 扫描 | SetupAPI + 注册表 | libudev + sysfs | IOKit |
| 通知 | RegisterDeviceNotification | udev monitor | IOKit notify |
| 颜色编码 | COLORREF 0x00BBGGRR | 0xAARRGGBB | NSColor |

> 注：Win32 的 `COLORREF` 是 **0x00BBGGRR**（蓝在低位）——本仓库 `paint.rs::cref`
> 已按此编码（曾误写成 0x00RRGGBB 导致红/蓝互换，实测修复）。其他平台按各自编码转换。
