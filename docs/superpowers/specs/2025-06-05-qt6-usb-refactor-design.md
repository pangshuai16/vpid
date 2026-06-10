# Qt6 + 全USB外设 重构设计方案

> **重构目标**：将Tauri + Svelte前端迁移至Qt6 Quick (QML)，USB枚举从HID扩展至全USB外设，保留完整UI功能和用户体验。

---

## 1. 技术选型

| 维度 | 决策 | 说明 |
|------|------|------|
| **语言** | Rust (edition 2021) | 业务逻辑 + USB 枚举 |
| **前端框架** | Qt6 Quick (QML) + Qt Quick Controls 2 | 声明式UI，接近Web开发体验 |
| **USB枚举库** | nusb 0.2 (纯Rust) | 跨平台、异步、原生热插拔支持 |
| **Rust ↔ QML通信** | qmetaobject-rs 0.2 | 通过 `#[derive(QObject)]` 宏，纯Rust无C++桥接 |
| **异步运行时** | futures-lite 2.0 | USB枚举异步化 |
| **序列化** | serde + serde_json | JSON 序列化设备数据供 QML 消费 |
| **构建系统** | 纯 Cargo，无需 CMake | qmetaobject-rs 在编译期调用系统 C++ 编译器 |
| **CI/CD** | GitHub Actions | 多平台构建 + Release 发布 |

---

## 2. 各平台编译

| 平台 | 编译器 | Qt 版本 | 依赖 | 难度 |
|------|--------|---------|------|------|
| **Linux** | GCC 11+ | Qt 6.5 | Rust + nusb | ⭐ 简单 |
| **Windows 10+** | MSVC 2022 | Qt 6.5 | Rust + nusb | ⭐ 简单 |
| **macOS (Intel)** | Xcode 13+ (Clang) | Qt 6.5 | Rust + nusb | ⭐ 简单 |
| **macOS (Apple Silicon)** | Xcode 13+ (Clang) | Qt 6.5 | Rust + nusb | ⭐ 简单 |
| **Linux ARM64** | GCC + 交叉工具链 | Qt 6.5 | Rust + nusb | ⭐⭐ 中等 |

### 2.1 qmetaobject-rs 跨平台优势

- 纯 Cargo 构建，无需 CMake 或 C++ 代码生成
- nusb 纯 Rust 实现，无 libusb 等 C 库依赖
- 编译期通过 `cpp` 宏调用系统 C++ 编译器链接 Qt 库
- 无需 `.qrc` 资源文件，QML 文件直接通过文件系统加载

### 2.2 交叉编译可行性

| 目标 | 工具 | 说明 |
|------|------|------|
| Linux → Linux ARM64 | `aarch64-unknown-linux-gnu` + 交叉工具链 | 可行，需 `gcc-aarch64-linux-gnu` |
| Linux → Windows | `x86_64-pc-windows-gnu` + MinGW | 可行，nusb支持 |
| Linux → macOS | ❌ 困难 | 需 osxcross 或 macOS 硬件 |
| macOS → iOS | ✅ 可行 | 需完整 iOS SDK |

### 2.3 软件渲染兼容

项目默认设置 `QT_QUICK_BACKEND=software` 环境变量，确保在以下场景正常运行：
- WSL2 环境
- 无 GPU 的 CI 服务器
- 远程桌面环境

---

## 3. 系统架构

### 3.1 整体架构

```
┌──────────────────────────────────────────────────────────────┐
│                         Qt6 Application                       │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  QML UI Layer                                          │ │
│  │  ├── main.qml              # 应用入口                   │ │
│  │  ├── MainPage.qml          # 主页面 + 刷新逻辑          │ │
│  │  ├── DeviceTable.qml       # 设备表格组件               │ │
│  │  └── Theme.qml             # 设计 Token 定义            │ │
│  └────────────────────────────────────────────────────────┘ │
│                           ↓ qml_register_type                │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Rust Backend Layer                                    │ │
│  │  ├── main.rs               # 应用入口 + QML引擎        │ │
│  │  ├── usb/                   # USB模块                   │ │
│  │  │   ├── enumerator.rs     # 设备枚举 (nusb, async)     │ │
│  │  │   ├── hotplug.rs        # 热插拔监听                 │ │
│  │  │   ├── models.rs         # 数据模型                   │ │
│  │  │   └── class_codes.rs    # USB类代码映射              │ │
│  │  └── ui/                                                │ │
│  │      └── usb_manager.rs    # UsbManager QObject 桥接    │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 3.2 数据流

```
nusb::watch_devices()
       │
       ▼
┌──────────────────┐
│ Hotplug Event     │  Connected / Disconnected / Change
│ Stream (Rust)     │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐    emit devicesChanged()
│ UsbManager       │─────────────────────────┐
│ (QObject)        │                         ▼
└────────┬─────────┘                 ┌──────────────────┐
         │ getDevicesJson()          │ MainPage.qml     │
         ▼                           │ loadDevices()    │
┌──────────────────┐                 └────────┬─────────┘
│ DeviceInfo List  │                          │
│ (Vec<DeviceInfo>)│                          ▼
│ → JSON String    │                 ┌──────────────────┐
└──────────────────┘                 │ DeviceTable.qml  │
                                     │ (ListView)       │
                                     └──────────────────┘
```

**关键设计**：
- USB 枚举结果通过 **JSON 字符串** 传递给 QML，QML 用 `JSON.parse()` 消费
- 设备对比（新增/移除）在 Rust 端计算，QML 仅负责展示
- 热插拔通过 `nusb::watch_devices()` 监听，QML 用 Timer 轮询触发刷新兜底

---

## 4. 数据模型设计

### 4.1 Rust 数据模型

```rust
// src/usb/models.rs

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub enum DeviceSpeed {
    Low,    // 1.5 Mbps
    Full,   // 12 Mbps
    High,   // 480 Mbps
    Super,  // 5 Gbps
    Unknown,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct UsbDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub device_speed: DeviceSpeed,
    pub device_class_name: String,  // 从 class_codes 映射
}
```

### 4.2 USB 类代码映射

```rust
// src/usb/class_codes.rs
pub fn usb_class_name(class: u8) -> &'static str {
    match class {
        0x00 => "Device Class",
        0x01 => "Audio",
        0x02 => "CDC (Communication)",
        0x03 => "HID (Human Interface)",
        0x08 => "Mass Storage",
        0x09 => "Hub",
        0x0E => "Video",
        0xFF => "Vendor Specific",
        _ => "Unknown",
    }
}
```

---

## 5. Rust 后端实现

### 5.1 USB 枚举器

```rust
// src/usb/enumerator.rs

pub async fn list_usb_devices() -> Result<Vec<UsbDeviceInfo>, String> {
    let devices = nusb::list_devices()
        .await
        .map_err(|e| format!("nusb error: {}", e))?;

    Ok(devices.into_iter().map(convert_device_info).collect())
}

pub fn convert_device_info(info: nusb::DeviceInfo) -> UsbDeviceInfo {
    UsbDeviceInfo {
        vendor_id: info.vendor_id(),
        product_id: info.product_id(),
        device_class: info.class(),
        // ... 更多字段
        device_class_name: class_codes::usb_class_name(info.class()).to_string(),
    }
}
```

### 5.2 热插拔监听

```rust
// src/usb/hotplug.rs

pub enum HotplugEvent {
    Connected(UsbDeviceInfo),
    Disconnected { vendor_id: u16, product_id: u16 },
    Change,
}

pub struct HotplugWatcher {
    _thread: thread::JoinHandle<()>,
}

impl HotplugWatcher {
    pub fn new(tx: Sender<HotplugEvent>) -> Self {
        let _thread = thread::spawn(move || {
            if let Ok(stream) = nusb::watch_devices() {
                for event in futures_lite::stream::block_on(stream) {
                    // 发送事件...
                }
            }
        });
        HotplugWatcher { _thread }
    }
}
```

### 5.3 UsbManager QObject 桥接

```rust
// src/ui/usb_manager.rs

/// 全局 USB 设备状态
static STATE: LazyLock<Mutex<DeviceState>> = LazyLock::new(|| {
    Mutex::new(DeviceState {
        devices: Vec::new(),
        baseline: Vec::new(),
        error: None,
    })
});

/// USB 管理器 — qmetaobject QObject 桥接
#[derive(Default, QObject)]
pub struct UsbManager {
    base: qt_base_class!(trait QObject),
    devices_changed: qt_signal!(),
    error_changed: qt_signal!(),

    refresh: qt_method!(fn(&mut self)),
    set_baseline: qt_method!(fn(&mut self)),
    get_devices_json: qt_method!(fn(&self) -> QString),
    get_added_devices_json: qt_method!(fn(&self) -> QString),
    get_removed_devices_json: qt_method!(fn(&self) -> QString),
    get_error: qt_method!(fn(&self) -> QString),
}
```

**设计要点**：
- 设备状态存放在 `LazyLock<Mutex<DeviceState>>` 全局变量中
- `refresh()` 调用 `futures_lite::future::block_on()` 同步执行异步枚举
- 设备对比（新增/移除）在 Rust 端计算，返回 JSON 字符串
- QML 通过 `devicesChanged` 信号感知数据变更

---

## 6. QML UI 设计

### 6.1 应用入口

```qml
// qml/main.qml
ApplicationWindow {
    Theme { id: theme }
    UsbManager { id: usbManager }
    MainPage {
        anchors.fill: parent
        usbManager: usbManager
        theme: theme
    }
}
```

### 6.2 MainPage — 主页面

```qml
// qml/MainPage.qml
Page {
    property var currentDevices: []
    property var addedDevices: []
    property var removedDevices: []

    function loadDevices() {
        currentDevices = theme.parseDevices(usbManager.getDevicesJson());
        addedDevices = theme.parseDevices(usbManager.getAddedDevicesJson());
        removedDevices = theme.parseDevices(usbManager.getRemovedDevicesJson());
    }

    Component.onCompleted: {
        usbManager.devicesChanged.connect(loadDevices);
        loadDevices();
    }

    // 每 3 秒自动刷新（热插拔兜底方案）
    Timer {
        interval: 3000; running: true; repeat: true
        onTriggered: usbManager.refresh();
    }
}
```

### 6.3 DeviceTable — 设备表格

```qml
// qml/DeviceTable.qml
ColumnLayout {
    property string title: ""
    property var model: []
    required property QtObject theme

    // 表头
    Rectangle { /* 设备/厂商/VID/PID/类别/速度/序列号 */ }

    // 数据行（ListView）
    ListView {
        model: root.model
        delegate: Rectangle {
            TapHandler {
                onTapped: {
                    Qt.application.clipboard.text = info;
                }
            }
        }
    }
}
```

### 6.4 Theme — 设计 Token

```qml
// qml/Theme.qml
QtObject {
    readonly property color primaryColor: "#0d6efd"
    readonly property int fontSizeTitle: 18
    readonly property int colWidthProduct: 160
    // ...

    function formatHex(value, digits) {
        return "0x" + ((value >>> 0).toString(16)).padStart(d, "0");
    }

    function parseDevices(json) {
        return JSON.parse(json);
    }
}
```

**设计要点**：
- `Theme.qml` 定义设计 Token，作为普通 `QtObject` 向下传递
- `formatHex` 将整数格式化为十六进制字符串
- `parseDevices` 解析 Rust 端传来的 JSON

---

## 7. 项目结构

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
│   │   ├── enumerator.rs   # USB 设备枚举（nusb, async）
│   │   ├── models.rs       # USB 设备数据结构
│   │   ├── class_codes.rs  # USB 类别码映射
│   │   └── hotplug.rs      # USB 热插拔监听
│   └── ui/
│       ├── mod.rs          # UI 模块
│       └── usb_manager.rs  # UsbManager QObject 桥接
└── .github/workflows/
    ├── build.yml            # CI 构建
    └── release.yml          # Release 发布
```

---

## 8. 构建配置

### 8.1 Cargo.toml

```toml
[package]
name = "vpid"
version = "2.0.0"
description = "USB设备查看器 (Qt6 + qmetaobject-rs)"
edition = "2021"

[dependencies]
qmetaobject = "0.2"
nusb = "0.2"
futures-lite = "2.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.11"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

### 8.2 各平台构建命令

```bash
# Linux
cargo build --release --target x86_64-unknown-linux-gnu

# macOS Intel
cargo build --release --target x86_64-apple-darwin

# macOS Apple Silicon
cargo build --release --target aarch64-apple-darwin

# Linux ARM64（交叉编译）
cargo build --release --target aarch64-unknown-linux-gnu
```

---

## 9. 功能对照表

| 功能 | 状态 | 实现方式 |
|------|------|----------|
| USB 设备列表 | ✅ 完成 | nusb 全设备枚举 |
| 设备信息展示 | ✅ 完成 | ListView + 7列 |
| 设备对比（新增/移除） | ✅ 完成 | Rust 计算，JSON 传递 |
| 设为基准 | ✅ 完成 | UsbManager.setBaseline() |
| 复制到剪贴板 | ✅ 完成 | Qt.application.clipboard |
| 手动刷新 | ✅ 完成 | ToolButton → refresh() |
| 热插拔自动检测 | ✅ 完成 | nusb watch_devices + 3s 轮询 |
| 错误提示 | ✅ 完成 | 错误 Banner + Timer |
| 退出应用 | ✅ 完成 | Qt.quit() |

---

## 10. CI/CD 设计

### 10.1 构建工作流（build.yml）

| Job | 平台 | 架构 | 触发 |
|-----|------|------|------|
| build-linux | Ubuntu | x86_64 | push/PR |
| build-windows | Windows | x86_64 | push/PR |
| build-macos | macOS | x86_64 + arm64 | push/PR |
| build-linux-arm64 | Ubuntu | ARM64 (交叉) | push/PR |
| lint | Ubuntu | - | push/PR |
| format | Ubuntu | - | push/PR |

### 10.2 发布工作流（release.yml）

| Job | 平台 | 说明 |
|-----|------|------|
| create-release | Ubuntu | 创建 Release 草稿 |
| build-assets | 多平台 | 编译各平台二进制 |
| upload-assets | Ubuntu | 上传至 Release |

触发条件：`git tag v*` 推送

---

## 11. 风险与注意事项

### 11.1 nusb 驱动要求

| 平台 | 驱动 | 注意事项 |
|------|------|----------|
| Linux | usbfs | 可能需要 udev 规则或 plugdev 组 |
| Windows | WinUSB | 非 HID 设备需 WCID 或 Zadig |
| macOS | IOKit | 可能需要签署应用 |

### 11.2 Qt 版本

- 需要 Qt 6.5+ 开发包
- 安装方式：
  - **Ubuntu**: `apt install qt6-base-dev qt6-declarative libqt6quickcontrols2-6`
  - **macOS**: `brew install qt@6`
  - **Windows**: MSYS2 或 Qt 在线安装器

### 11.3 qmetaobject-rs 特性

- 编译期需要 C++ 编译器（用于生成 QMetaObject）
- 链接 Qt 库，无需 CMake
- 信号槽机制通过 `qt_signal!` 和 `qt_method!` 宏实现

---

**文档状态**: ✅ 设计完成
