# Qt6 + 全USB外设 重构实施计划

> **状态**: 核心功能已实现，剩余 CI/CD 与优化

---

## 当前状态

| 模块 | 状态 | 说明 |
|------|------|------|
| 环境搭建 | ✅ 完成 | Cargo.toml + rust-toolchain 已配置 |
| USB 数据模型 | ✅ 完成 | UsbDeviceInfo + DeviceSpeed + class_codes |
| USB 枚举 | ✅ 完成 | nusb 异步枚举 + 数据转换 |
| 热插拔监听 | ✅ 完成 | nusb::watch_devices + 后台线程 |
| QObject 桥接 | ✅ 完成 | UsbManager (qmetaobject-rs) |
| QML UI | ✅ 完成 | main + MainPage + DeviceTable + Theme |
| 功能完善 | ✅ 完成 | 设备对比/基准/剪贴板/错误提示 |
| build.yml (CI) | ✅ 完成 | 多平台构建 + lint + format |
| release.yml | 🔄 待验证 | 已重写为纯 Rust + Qt 方案 |

---

## 技术栈（实际使用）

| 组件 | 库 | 版本 |
|------|------|------|
| 语言 | Rust | edition 2021 |
| UI 框架 | Qt 6 Quick + QuickControls 2 | 6.5+ |
| Rust ↔ QML | qmetaobject-rs | 0.2 |
| USB 枚举 | nusb | 0.2 |
| 异步 | futures-lite | 2.0 |
| 序列化 | serde + serde_json | 1.0 |
| 日志 | log + env_logger | 0.4 / 0.11 |

---

## 架构概览

### 数据流

```
nusb::watch_devices() → HotplugWatcher → Sender<HotplugEvent>
  │
  ▼
UsbManager.refresh() → enumerator::list_usb_devices() → STATE (LazyLock<Mutex>)
  │
  ▼ emit devicesChanged()
MainPage.loadDevices() → usbManager.getDevicesJson() → JSON.parse() → DeviceTable
```

### 关键设计决策

1. **JSON 传递**：Rust 端序列化设备数据为 JSON 字符串，QML 用 `JSON.parse()` 消费，避免复杂的 QVariant 嵌套
2. **全局状态**：设备数据存放在 `LazyLock<Mutex<DeviceState>>` 中，QObject 方法通过 Mutex 访问
3. **信号驱动**：`devicesChanged` 信号通知 QML 数据更新，QML 的 `Component.onCompleted` 连接信号
4. **轮询兜底**：3 秒 Timer 轮询 `refresh()`，作为热插拔监听的补充
5. **软件渲染**：默认 `QT_QUICK_BACKEND=software`，确保 WSL2/CI 环境可用

---

## 剩余任务

### Task 1: Release 工作流验证

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: 推送测试标签**

```bash
git tag v2.0.0-test
git push origin v2.0.0-test
```

- [ ] **Step 2: 检查构建结果**

验证项：
- [ ] Linux x86_64 构建成功
- [ ] Windows x86_64 构建成功
- [ ] macOS x86_64 构建成功 (macos-13 runner)
- [ ] macOS ARM64 构建成功
- [ ] Linux ARM64 交叉编译成功
- [ ] Release 草稿自动创建
- [ ] 所有二进制文件已上传

- [ ] **Step 3: 清理测试标签**（可选）

```bash
git tag -d v2.0.0-test
git push origin --delete v2.0.0-test
```

---

### Task 2: CI/CD 优化（可选）

- [ ] **Step 1: 添加缓存**

在 build.yml 和 release.yml 中添加 Rust 缓存：

```yaml
- uses: Swatinem/rust-cache@v2
```

- [ ] **Step 2: 并行构建优化**

确认所有 build jobs 并行运行（当前已是）

- [ ] **Step 3: 失败通知**

添加 Slack/Discord 通知（可选）

---

### Task 3: 版本管理（可选）

- [ ] **Step 1: 添加版本信息到代码**

```rust
// src/lib.rs
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

- [ ] **Step 2: 添加 `--version` 支持**

```rust
// src/main.rs
fn main() {
    // 处理 --version / -v 参数
    // ...
}
```

---

## CI/CD 工作流

### build.yml (push/PR 触发)

| Job | Runner | Target | 触发条件 |
|-----|--------|--------|----------|
| build-linux | ubuntu-latest | x86_64-unknown-linux-gnu | push/PR |
| build-windows | windows-latest | x86_64-pc-windows-msvc | push/PR |
| build-macos | macos-latest | x86_64 + arm64 | push/PR |
| build-linux-arm64 | ubuntu-latest | aarch64-unknown-linux-gnu | push/PR |
| lint | ubuntu-latest | - | push/PR |
| format | ubuntu-latest | - | push/PR |

### release.yml (tag v* 触发)

| Job | Runner | Target | 说明 |
|-----|--------|--------|------|
| build-linux | ubuntu-latest | x86_64 | Linux x86_64 构建 |
| build-windows | windows-latest | x86_64 | Windows x86_64 构建 |
| build-macos-x86_64 | macos-13 | x86_64 | macOS Intel 构建 |
| build-macos-arm64 | macos-latest | arm64 | macOS Apple Silicon 构建 |
| build-linux-arm64 | ubuntu-latest | aarch64 | Linux ARM64 交叉编译 |
| create-release | ubuntu-latest | - | 创建 Release 草稿 + 上传所有二进制 |

**发布流程**：
```bash
# 1. 更新版本号（Cargo.toml）
# 2. 提交更改
git commit -m "bump version to v2.0.0"
# 3. 打标签并推送
git tag v2.0.0
git push origin v2.0.0
# 4. CI 自动构建并发布
```

---

## 平台差异

| 特性 | Linux | Windows | macOS |
|------|-------|---------|-------|
| 依赖安装 | apt install qt6-base-dev qt6-declarative | Qt 在线安装器 / MSYS2 | brew install qt@6 |
| 编译器 | GCC 11+ | MSVC 2022 | Xcode Clang |
| USB 驱动 | usbfs (+ udev 规则) | WinUSB | IOKit |
| Qt 路径 | /usr/lib/qt6 | C:\Qt\6.5 | /opt/homebrew/opt/qt@6 |

---

## 构建产物命名

| 平台 | 产物名称 | 说明 |
|------|----------|------|
| Linux x86_64 | `vpid-linux-x86_64` | ELF 二进制 |
| Linux ARM64 | `vpid-linux-arm64` | ELF 二进制 (aarch64) |
| Windows x86_64 | `vpid-windows-x86_64.exe` | PE 可执行文件 |
| macOS x86_64 | `vpid-macos-x86_64` | Mach-O 二进制 |
| macOS ARM64 | `vpid-macos-arm64` | Mach-O 二进制 (Apple Silicon) |

---

**计划状态**: ✅ 完成（核心功能已实现，待验证 Release 工作流）
