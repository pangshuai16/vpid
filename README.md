# vpid - USB 设备查看器 (Qt6 + qmetaobject-rs)

跨平台 USB 外设查看工具。基于 Rust + Qt6 Quick (QML) + qmetaobject-rs 实现，
USB 枚举使用纯 Rust 的 `nusb` 库，无需 CMake/C++ 桥接生成环节。

## 功能

- 列出当前连接的所有 USB 设备（厂商/产品/VID/PID/序列号等）
- 与基准对比，识别**新增**和**移除**设备
- 一键复制设备信息到剪贴板
- 跨平台：Linux / Windows / macOS

## 技术栈

- **Rust** (edition 2021) — 业务逻辑 + USB 枚举（`nusb`）
- **Qt 6.5+** — Quick / QuickControls 2 / QML
- **[qmetaobject-rs](https://github.com/woboq/qmetaobject-rs)** — Rust QObject → QML 绑定（纯 Rust，无 C++ 代码生成）
- 纯 Cargo 构建，无需 CMake / Node.js / 前端打包

## 项目结构

```
vpid/
├── Cargo.toml              # Rust 依赖（qmetaobject + nusb）
├── rust-toolchain.toml     # Rust 工具链固定
├── run.sh                  # WSL2/X11 启动脚本
├── qml/
│   ├── main.qml            # 窗口入口
│   ├── MainPage.qml        # 主页面布局
│   ├── DeviceTable.qml     # 设备表格组件
│   └── Theme.qml           # 设计 Token 定义
├── src/
│   ├── main.rs             # 应用入口（QmlEngine）
│   ├── lib.rs              # 库入口
│   ├── usb/
│   │   ├── mod.rs          # USB 模块
│   │   ├── enumerator.rs   # USB 设备枚举（nusb）
│   │   ├── models.rs       # USB 设备数据结构
│   │   ├── class_codes.rs  # USB 类别码映射
│   │   └── hotplug.rs      # USB 热插拔监听（预留）
│   └── ui/
│       ├── mod.rs          # UI 模块
│       └── usb_manager.rs  # UsbManager QObject 桥接
└── README.md
```

## 快速开始

### 前置条件

- Rust 1.80+ （`LazyLock` 支持）
- Qt 6.5+ 开发包
  - **Ubuntu/Debian**: `apt install qt6-base-dev qt6-declarative-dev libqt6quickcontrols2-6`
  - **macOS**: `brew install qt@6`
  - **Windows**: 使用 MSYS2 或 Qt 在线安装器

### 构建与运行

```bash
# 调试运行
cargo run

# 发布构建
cargo build --release
./target/release/vpid
```

### 构建脚本

无需自定义构建脚本。qmetaobject 在编译期自动通过 `cpp` 宏调用系统 C++ 编译器
链接 Qt 库，无需 CMake 或 .qrc 资源文件。

## 架构说明

与传统的 cxx-qt 方案不同，qmetaobject-rs 将 Rust QObject 直接暴露给 QML：

- **UsbManager** — `#[derive(QObject)]` 宏生成 QMetaObject，QML 可直接实例化
- **内部状态** — 设备数据存放在全局 `LazyLock<Mutex<…>>` 中，QObject 方法通过 Mutex 访问
- **Theme** — 设计 Token 定义为普通 `QtObject`，由入口文件实例化后向下传递
- **无 signal 驱动** — USB 枚举（毫秒级）采用同步阻塞，QML 调用后立即更新
