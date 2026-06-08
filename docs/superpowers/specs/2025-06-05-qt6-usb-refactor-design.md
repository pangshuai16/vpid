# Qt6 + 全USB外设 重构设计方案

> **重构目标**：将Tauri + Svelte前端迁移至Qt6 Quick (QML)，USB枚举从HID扩展至全USB外设，保留完整UI功能和用户体验。

---

## 1. 技术选型确认

| 维度 | 决策 | 说明 |
|------|------|------|
| **前端框架** | Qt6 Quick (QML) + Qt Quick Controls 2 | 声明式UI，接近Web开发体验 |
| **USB枚举库** | nusb (纯Rust) | 跨平台、异步、原生热插拔支持 |
| **Rust ↔ QML通信** | `#[QObject]` 宏暴露对象 | 通过信号槽机制通信 |
| **样式系统** | QML自定义 + QSS覆盖 | 复刻Pico CSS现代风格 |
| **构建系统** | Cargo + CXX-Qt 0.8.1 | 纯Cargo构建，无需CMake |
| **Windows XP兼容** | 第三方Qt6.5 (YY-Thunks) | https://github.com/pangshuai16/qt6_5_for_xp |

---

## 2. Windows XP 兼容方案（关键）

### 2.1 第三方 Qt6.5 for XP

由于官方 Qt 6.5 要求 Windows 10+，需使用第三方移植版本：

**来源**: https://github.com/pangshuai16/qt6_5_for_xp

**技术原理**:
- 使用 **YY-Thunks** 技术实现 Windows XP API 兼容层
- 将 Windows 10 API 调用 thunk 到 XP 等效实现
- 支持 x86 / x64 / ARM64 架构

**版本特性**:
| 特性 | 说明 |
|------|------|
| 仅 Release 构建 | Debug 版本不可用 |
| 关闭模块 | SQL、OpenGL、DBus 模块被禁用 |
| SSL 替代 | 使用 **schannel** (Windows 原生) 替代 OpenSSL |
| 架构支持 | x86 / x64 / ARM64 |

**对项目的限制**:
1. ❌ 不能使用 Qt SQL 模块 → 如需数据库需第三方方案
2. ❌ 不能使用 Qt OpenGL 模块 → 需使用 Qt Quick 2D 渲染
3. ❌ 不能使用 Qt DBus → Linux 下无影响（XP 无 DBus）
4. ⚠️ 必须 Release 构建 → 调试需额外配置

### 2.2 CXX-Qt 兼容性

CXX-Qt 0.8.1 与 XP 版本 Qt 的兼容性：

| 检查项 | 状态 |
|--------|------|
| 纯 Cargo 构建 | ✅ 兼容，无需 CMake |
| Qt 版本要求 | ✅ Qt 6.5 兼容 |
| 仅 Release | ✅ 生产环境无影响 |
| schannel 替代 OpenSSL | ✅ nusb 使用原生 API，无影响 |

### 2.3 各平台原生编译（推荐）

| 平台 | 编译器 | Qt 版本 | 依赖 | 难度 |
|------|--------|---------|------|------|
| **Windows XP** | MSVC 2019/2022 | 第三方 Qt6.5 (YY-Thunks) | Rust + nusb | ⭐⭐ 中等 |
| **Windows 10+** | MSVC 2022 | 官方 Qt 6.5 | Rust + nusb | ⭐ 简单 |
| **Linux** | GCC 11+ | Qt 6.5 | Rust + nusb | ⭐ 简单 |
| **macOS** | Xcode 13+ (Clang) | Qt 6.5 | Rust + nusb | ⭐ 简单 |

**XP 特殊注意事项**:
- 使用第三方 Qt 版本时需配置 `CXXQT_QT_VERSION=6.5`
- 需禁用 OpenGL 相关 QML 效果（使用 2D 渲染）
- 需测试 schannel 与 nusb 的兼容性（nusb 使用 WinUSB，应无冲突）

### 2.4 交叉编译可行性

| 源平台 → 目标平台 | 可行性 | 复杂度 | 说明 |
|-------------------|--------|--------|------|
| Linux → Windows | ✅ 可行 | ⭐⭐ 中等 | 需 MinGW + Qt for Windows |
| Linux → Linux (不同架构) | ✅ 可行 | ⭐⭐ 中等 | 需 sysroot + toolchain |
| Linux → macOS | ❌ 不可行 | - | Apple许可限制，需macOS硬件 |
| Windows → Windows ARM | ✅ 可行 | ⭐⭐ 中等 | Qt官方支持，需MSVC |
| macOS → iOS | ✅ 可行 | ⭐⭐⭐ 复杂 | 需完整iOS SDK |

### 2.3 Rust交叉编译

| 目标 | 工具 | 说明 |
|------|------|------|
| Linux → Windows | `x86_64-pc-windows-gnu` + MinGW | 可行，nusb支持 |
| Linux → Linux ARM | `cargo-zigbuild` 或 `cross` | 可行，纯Rust无C依赖 |
| Linux → macOS | ❌ 困难 | 需osxcross或macOS硬件 |

**nusb跨平台优势**：纯Rust实现，无C库依赖（如libusb），交叉编译更简单。

### 2.5 推荐方案

```
方案A：各平台原生编译（推荐）
├── Windows: MSVC + Qt6.5 + Cargo
├── Linux: GCC + Qt6.5 + Cargo
└── macOS: Xcode + Qt6.5 + Cargo

方案B：CI/CD 多平台构建
├── GitHub Actions: windows-latest, ubuntu-latest, macos
└── 或使用 alpine-rustx Docker镜像交叉编译Rust部分

方案C：Windows为主开发平台
├── Windows原生编译
├── 使用Qt for Windows交叉编译Linux版本（需sysroot）
└── macOS版本需macOS硬件
```

---

## 3. 系统架构设计

### 3.1 整体架构

```
┌──────────────────────────────────────────────────────────────┐
│                         Qt6 Application                       │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  QML UI Layer                                          │ │
│  │  ├── main.qml              # 应用入口                   │ │
│  │  ├── MainWindow.qml        # 主窗口框架                 │ │
│  │  ├── DeviceTable.qml       # 设备表格组件               │ │
│  │  ├── DeviceTree.qml        # 设备树视图                 │ │
│  │  ├── Toast.qml             # 通知组件                   │ │
│  │  └── styles/               # 样式定义                   │ │
│  │      ├── theme.qml         # 主题变量                   │ │
│  │      └── qss.qss           # QSS样式表                  │ │
│  └────────────────────────────────────────────────────────┘ │
│                           ↓ QObject bridge                   │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Rust Backend Layer                                    │ │
│  │  ├── main.rs               # 应用入口                   │ │
│  │  ├── usb/                   # USB模块                   │ │
│  │  │   ├── enumerator.rs     # 设备枚举 (nusb)            │ │
│  │  │   ├── hotplug.rs        # 热插拔监听                 │ │
│  │  │   └── models.rs         # 数据模型                   │ │
│  │  └── ui_bridge.rs          # QML ↔ Rust 桥接            │ │
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
│ Hotplug Event     │  Connected / Disconnected
│ Stream (Rust)     │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐    emit deviceChanged()
│ UusbManager      │─────────────────────────┐
│ (QObject)        │                         │
└────────┬─────────┘                         ▼
         │ devices()                 ┌──────────────────┐
         ▼                           │ QML DeviceModel  │
┌──────────────────┐                 │ (QAbstractTableModel)│
│ DeviceInfo List  │                 └────────┬─────────┘
│ (Vec<DeviceInfo>)│                          │
└──────────────────┘                          ▼
                                     ┌──────────────────┐
                                     │ DeviceTable.qml  │
                                     │ (TableView)      │
                                     └──────────────────┘
```

---

## 4. 数据模型设计

### 4.1 Rust 数据模型

```rust
// src/usb/models.rs

use nusb::DeviceInfo as NusbDeviceInfo;
use serde::Serialize;
use std::fmt;

/// USB设备信息（完整结构）
#[derive(Serialize, Clone, Debug)]
pub struct UsbDeviceInfo {
    /// 总线号
    pub bus: u8,
    /// 设备地址
    pub device_address: u8,
    /// 厂商ID
    pub vendor_id: u16,
    /// 产品ID
    pub product_id: u16,
    /// USB设备类
    pub device_class: u8,
    /// 子类
    pub device_subclass: u8,
    /// 协议
    pub device_protocol: u8,
    /// 设备速度
    pub device_speed: DeviceSpeed,
    /// 最大包大小
    pub max_packet_size: u8,
    /// 厂商名称
    pub manufacturer: Option<String>,
    /// 产品名称
    pub product: Option<String>,
    /// 序列号
    pub serial_number: Option<String>,
    /// 配置数量
    pub config_count: u8,
    /// 接口数量
    pub interface_count: u8,
    /// 接口信息
    pub interfaces: Vec<InterfaceInfo>,
    /// 设备路径（平台特定）
    pub path: String,
    /// 端口号路径
    pub port_numbers: Vec<u8>,
}

#[derive(Serialize, Clone, Debug)]
pub enum DeviceSpeed {
    Low,
    Full,
    High,
    Super,
    Unknown,
}

impl fmt::Display for DeviceSpeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceSpeed::Low => write!(f, "Low (1.5 Mbps)"),
            DeviceSpeed::Full => write!(f, "Full (12 Mbps)"),
            DeviceSpeed::High => write!(f, "High (480 Mbps)"),
            DeviceSpeed::Super => write!(f, "Super (5 Gbps)"),
            DeviceSpeed::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct InterfaceInfo {
    pub interface_number: u8,
    pub alternate_setting: u8,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub class_name: String,
    pub endpoints: Vec<EndpointInfo>,
}

#[derive(Serialize, Clone, Debug)]
pub struct EndpointInfo {
    pub endpoint_address: u8,
    pub direction: EndpointDirection,
    pub transfer_type: TransferType,
    pub max_packet_size: u16,
    pub interval: u8,
}

#[derive(Serialize, Clone, Debug)]
pub enum EndpointDirection {
    In,
    Out,
    Control,
}

#[derive(Serialize, Clone, Debug)]
pub enum TransferType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
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
        0x05 => "Physical",
        0x06 => "Image (Printer/Scanner)",
        0x07 => "Printer",
        0x08 => "Mass Storage",
        0x09 => "Hub",
        0x0A => "CDC-Data",
        0x0B => "Smart Card",
        0x0D => "Content Security",
        0x0E => "Video",
        0x0F => "Personal Healthcare",
        0x10 => "Audio/Video",
        0xDC => "Diagnostic Device",
        0xE0 => "Wireless Controller",
        0xEF => "Miscellaneous",
        0xFE => "Application Specific",
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

use nusb;
use crate::usb::models::*;

pub fn list_usb_devices() -> Result<Vec<UsbDeviceInfo>, String> {
    let devices = nusb::list_devices()
        .map_err(|e| format!("nusb error: {}", e))?;

    Ok(devices
        .into_iter()
        .map(convert_device_info)
        .collect())
}

fn convert_device_info(info: NusbDeviceInfo) -> UsbDeviceInfo {
    let desc = info.device_descriptor();
    
    UsbDeviceInfo {
        bus: info.bus(),
        device_address: info.device_address(),
        vendor_id: desc.vendor_id(),
        product_id: desc.product_id(),
        device_class: desc.device_class(),
        device_subclass: desc.device_subclass(),
        device_protocol: desc.device_protocol(),
        device_speed: match info.speed() {
            nusb::Speed::Low => DeviceSpeed::Low,
            nusb::Speed::Full => DeviceSpeed::Full,
            nusb::Speed::High => DeviceSpeed::High,
            nusb::Speed::Super => DeviceSpeed::Super,
            _ => DeviceSpeed::Unknown,
        },
        max_packet_size: desc.max_packet_size(),
        manufacturer: info.manufacturer_string(),
        product: info.product_string(),
        serial_number: info.serial_number(),
        config_count: desc.num_configurations(),
        interface_count: info.interfaces().count() as u8,
        interfaces: collect_interfaces(&info),
        path: info.path().to_string_lossy().to_string(),
        port_numbers: info.port_numbers().to_vec(),
    }
}
```

### 5.2 热插拔监听

```rust
// src/usb/hotplug.rs

use nusb;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

pub enum HotplugEvent {
    Connected(UsbDeviceInfo),
    Disconnected { vendor_id: u16, product_id: u16 },
}

pub struct HotplugWatcher {
    tx: Sender<HotplugEvent>,
    rx: Receiver<HotplugEvent>,
    _thread: thread::JoinHandle<()>,
}

impl HotplugWatcher {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        
        let _thread = thread::spawn(move || {
            let stream = nusb::watch_devices();
            for event in stream {
                match event {
                    nusb::DeviceEvent::Added(info) => {
                        let device_info = convert_device_info(info);
                        let _ = tx.send(HotplugEvent::Connected(device_info));
                    }
                    nusb::DeviceEvent::Removed(device_id) => {
                        let _ = tx.send(HotplugEvent::Disconnected {
                            vendor_id: 0,
                            product_id: 0,
                        });
                    }
                }
            }
        });
        
        HotplugWatcher { tx, rx, _thread }
    }
    
    pub fn receiver(&self) -> &Receiver<HotplugEvent> {
        &self.rx
    }
}
```

### 5.3 QML 桥接对象

```rust
// src/ui_bridge.rs

use qmetaobject::QObject;
use std::sync::Mutex;
use crate::usb::{enumerator, hotplug, models::*};

#[derive(QObject)]
pub struct UsbManager {
    base: qt_base_class!(trait UsbManagerTrait),
    devices: Mutex<Vec<UsbDeviceInfo>>,
    watcher: Mutex<Option<hotplug::HotplugWatcher>>,
}

impl UsbManager {
    pub fn new() -> Self {
        let mut manager = UsbManager {
            base: qt_base_class!(trait UsbManagerTrait).new(),
            devices: Mutex::new(Vec::new()),
            watcher: Mutex::new(None),
        };
        
        // 初始化设备列表
        if let Ok(devices) = enumerator::list_usb_devices() {
            *manager.devices.lock().unwrap() = devices;
        }
        
        // 启动热插拔监听
        manager.start_hotplug_watch();
        
        manager
    }
    
    fn start_hotplug_watch(&self) {
        let watcher = hotplug::HotplugWatcher::new();
        let tx = watcher.receiver().clone();
        
        // 启动后台线程处理事件
        let devices = self.devices.clone();
        std::thread::spawn(move || {
            for event in tx {
                match event {
                    hotplug::HotplugEvent::Connected(device) => {
                        devices.lock().unwrap().push(device);
                        // emit deviceChanged() 通知QML
                    }
                    hotplug::HotplugEvent::Disconnected { .. } => {
                        // 移除设备
                        // emit deviceChanged() 通知QML
                    }
                }
            }
        });
        
        *self.watcher.lock().unwrap() = Some(watcher);
    }
}

#[qt_trait]
pub trait UsbManagerTrait {
    fn devices(&self) -> QVariant;
    fn refresh(&self);
    fn get_device_count(&self) -> i32;
}

impl UsbManagerTrait for UsbManager {
    fn devices(&self) -> QVariant {
        // 返回QML可消费的模型
        todo!()
    }
    
    fn refresh(&self) {
        if let Ok(devices) = enumerator::list_usb_devices() {
            *self.devices.lock().unwrap() = devices;
            // emit devicesChanged()
        }
    }
    
    fn get_device_count(&self) -> i32 {
        self.devices.lock().unwrap().len() as i32
    }
}
```

---

## 6. QML UI 设计

### 6.1 主窗口结构

```qml
// src/qml/MainWindow.qml

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import QtQuick.Controls.Material 2.15

ApplicationWindow {
    id: mainWindow
    visible: true
    width: 1000
    height: 700
    title: "USB设备 vpi pid 查看器"
    
    // 主题颜色（复刻Pico CSS风格）
    property color primaryColor: "#0d6efd"
    property color primaryInverse: "#ffffff"
    property color backgroundColor: "#f8f9fa"
    property color cardShadow: "#333333"
    
    // 深色模式支持
    Material.theme: Material.Light
    Material.accent: primaryColor
    
    // USB管理器（Rust桥接对象）
    property var usbManager: UsbManager {}
    
    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            spacing: 10
            
            Label {
                text: "USB设备查看器"
                font.pixelSize: 18
                font.weight: Font.Medium
                color: primaryColor
            }
            
            Item { Layout.fillWidth: true }
            
            ToolButton {
                text: "刷新"
                onClicked: usbManager.refresh()
            }
            
            ToolButton {
                text: "退出"
                onClicked: Qt.quit()
            }
        }
    }
    
    body: ColumnLayout {
        anchors.fill: parent
        spacing: 10
        padding: 10
        
        // 当前设备表格
        DeviceTable {
            id: currentTable
            title: "当前设备"
            model: usbManager.devices
            Layout.fillWidth: true
            Layout.preferredHeight: parent.height / 2 - 10
        }
        
        // 新增/移除设备（对比视图）
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 10
            
            DeviceTable {
                id: addTable
                title: "新增设备"
                model: usbManager.addedDevices
                Layout.fillWidth: true
            }
            
            DeviceTable {
                id: removeTable
                title: "移除设备"
                model: usbManager.removedDevices
                Layout.fillWidth: true
            }
        }
        
        // 操作按钮组
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            
            Button {
                text: "刷新"
                onClicked: usbManager.refresh()
            }
            
            Button {
                text: "设为基准"
                onClicked: usbManager.setBaseline()
            }
            
            Button {
                text: "复制当前"
                onClicked: copyDevices(usbManager.devices)
            }
            
            Button {
                text: "清空"
                onClicked: usbManager.clear()
            }
            
            Item { Layout.fillWidth: true }
        }
    }
    
    // Toast 通知
    Toast {
        id: toast
        anchors.centerIn: parent
        visible: message !== ""
        message: toastMessage
    }
}
```

### 6.2 设备表格组件

```qml
// src/qml/DeviceTable.qml

import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

ColumnLayout {
    id: root
    property string title: ""
    property var model: []
    
    // 样式
    property color primaryColor: "#0d6efd"
    property color hoverColor: "#0d6efd"
    property color hoverText: "#ffffff"
    
    // 标题（点击复制）
    Label {
        text: root.title
        font.pixelSize: 18
        font.weight: Font.Medium
        color: primaryColor
        MouseArea {
            anchors.fill: parent
            onClicked: copyAll(model)
            cursorShape: Qt.PointingHandCursor
        }
    }
    
    // 表格容器（带内阴影）
    Rectangle {
        Layout.fillWidth: true
        Layout.fillHeight: true
        radius: 8
        color: "#ffffff"
        
        // 内阴影效果
        layer.enabled: true
        layer.effect: ShaderEffect {
            // 简化版内阴影
        }
        
        TableView {
            id: tableView
            anchors.fill: parent
            anchors.margins: 10
            
            model: ListModel {
                // 动态绑定到usbManager
            }
            
            delegate: TableRow {
                background: Rectangle {
                    color: mouseArea.containsMouse ? hoverColor : "transparent"
                    Behavior on color { ColorAnimation { duration: 150 } }
                }
                
                Rectangle {
                    width: tableView.columnWidths[0]
                    height: tableView.rowHeight
                    Text {
                        text: model.product || "Unknown"
                        color: mouseArea.containsMouse ? hoverText : "black"
                    }
                }
                Rectangle {
                    width: tableView.columnWidths[1]
                    height: tableView.rowHeight
                    Text {
                        text: "0x" + model.vendor_id.toString(16).padStart(4, "0")
                        color: mouseArea.containsMouse ? hoverText : "black"
                    }
                }
                Rectangle {
                    width: tableView.columnWidths[2]
                    height: tableView.rowHeight
                    Text {
                        text: "0x" + model.product_id.toString(16).padStart(4, "0")
                        color: mouseArea.containsMouse ? hoverText : "black"
                    }
                }
                
                MouseArea {
                    id: mouseArea
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: copyDevice(model)
                }
            }
            
            columns: [
                TableColumn { title: "设备", width: 300 },
                TableColumn { title: "VID", width: 100 },
                TableColumn { title: "PID", width: 100 }
            ]
        }
    }
}
```

### 6.3 Toast 通知组件

```qml
// src/qml/Toast.qml

import QtQuick 2.15
import QtQuick.Controls 2.15

Popup {
    id: toast
    modal: false
    focus: true
    closePolicy: Popup.NoAutoClose
    
    property string message: ""
    property color primaryColor: "#0d6efd"
    
    EnterTransition: Transition {
        NumberAnimation {
            target: toast
            property: "opacity"
            from: 0
            to: 1
            duration: 160
            delay: 50
        }
    }
    
    ExitTransition: Transition {
        NumberAnimation {
            target: toast
            property: "opacity"
            from: 1
            to: 0
            duration: 160
        }
    }
    
    background: Rectangle {
        radius: 5
        color: primaryColor
        shadow: true  // Qt Quick Controls 2 阴影
        
        Rectangle {
            anchors.fill: parent
            anchors.margins: -24
            radius: 5
            color: primaryColor
            opacity: 0.3
        }
    }
    
    contentItem: Text {
        text: message
        color: "#ffffff"
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        font.pixelSize: 14
    }
}
```

### 6.4 QSS 样式表

```css
/* src/qml/styles/qss.qss */

/* 全局样式 */
ApplicationWindow {
    background: #f8f9fa;
}

/* 按钮样式 */
Button {
    background-color: #0d6efd;
    color: #ffffff;
    border-radius: 5px;
    padding: 8px 16px;
}

Button:hover {
    background-color: #0b5ed7;
}

Button:pressed {
    background-color: #0a58ca;
}

/* 表格样式 */
TableView {
    background-color: #ffffff;
    alternate-background-color: #f8f9fa;
    gridline-color: #dee2e6;
    selection-background-color: #0d6efd;
    selection-color: #ffffff;
}

/* 滚动条样式 */
ScrollBar {
    policy: Qt.AsNeeded;
    width: 8px;
}

ScrollBar::handle {
    background-color: #adb5bd;
    border-radius: 4px;
    min-height: 20px;
}

ScrollBar::handle:hover {
    background-color: #6c757d;
}
```

---

## 7. 项目结构（CXX-Qt 0.8.1）

```
vpid/
├── Cargo.toml                  # Rust依赖 + CXX-Qt配置
├── rust-toolchain.toml         # Rust工具链配置
├── build.rs                    # CXX-Qt构建脚本
├── src/
│   ├── main.rs                 # Rust应用入口
│   ├── lib.rs                  # 库入口（导出QObject）
│   ├── usb/
│   │   ├── mod.rs              # USB模块入口
│   │   ├── enumerator.rs       # 设备枚举 (nusb)
│   │   ├── hotplug.rs          # 热插拔监听
│   │   ├── models.rs           # 数据模型
│   │   └── class_codes.rs      # USB类代码映射
│   ├── ui/
│   │   ├── mod.rs              # UI模块入口
│   │   ├── usb_manager.rs      # CXX-Qt QObject实现
│   │   └── qml_bridge.rs       # QML注册与桥接
│   └── qml/
│       ├── main.qml            # QML应用入口
│       ├── MainWindow.qml      # 主窗口
│       ├── DeviceTable.qml     # 设备表格组件
│       ├── DeviceTree.qml      # 设备树视图（可选）
│       ├── Toast.qml           # 通知组件
│       └── styles/
│           ├── theme.qml       # 主题定义
│           └── qss.qss         # QSS样式表
├── resources/
│   ├── icons/                  # 应用图标
│   │   ├── icon.png
│   │   ├── icon.ico
│   │   └── ...
│   └── translations/           # 翻译文件
└── build/                      # 构建输出
```

---

## 8. 构建配置（纯 Cargo）

### 8.1 Cargo.toml

```toml
[package]
name = "vpid"
version = "2.0.0"
description = "USB设备查看器 (Qt6 + CXX-Qt版)"
edition = "2021"

[dependencies]
# CXX-Qt 0.8.1 (KDAB维护，2026-02稳定)
cxx-qt = "0.8.1"
cxx-qt-lib = "0.8.1"

# USB库
nusb = "0.2"

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 日志
log = "0.4"
env_logger = "0.11"

# 线程安全
parking_lot = "0.12"

[build-dependencies]
cxx-qt-build = "0.8.1"

[features]
default = []
# Windows XP 特殊配置
windows_xp = []

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

### 8.2 build.rs（CXX-Qt构建脚本）

```rust
// build.rs

use cxx_qt_build::CxxQtBuilder;

fn main() {
    let mut builder = CxxQtBuilder::new();
    
    // 注册 QObject 类
    builder = builder.lib_source(
        "src/ui/usb_manager.rs",
        "UsbManager",
    );
    
    // QML 模块注册
    builder = builder.qml_module(
        "com.vpid.Qt",
        "UsbManager",
        "src/ui/usb_manager.rs",
    );
    
    // Windows XP 特殊配置
    #[cfg(all(target_os = "windows", feature = "windows_xp"))]
    {
        // 禁用 OpenGL（XP版本不支持）
        builder = builder.cargo_cfg("cxx_qt_disable_opengl");
        
        // 使用 schannel（Qt XP版本已内置）
        builder = builder.cargo_cfg("cxx_qt_use_schannel");
    }
    
    builder.build();
}
```

### 8.3 Windows XP 编译命令

```bash
# Windows XP (x64)
cargo build --release --features windows_xp --target x86_64-pc-windows-msvc

# Windows 10+ (x64) - 使用官方 Qt
cargo build --release --target x86_64-pc-windows-msvc

# Linux (x64)
cargo build --release --target x86_64-unknown-linux-gnu

# Linux ARM64
cargo build --release --target aarch64-unknown-linux-gnu
```

### 8.4 部署脚本（Windows XP）

```powershell
# deploy-xp.ps1

# 1. 复制主程序
Copy-Item target\x86_64-pc-windows-msvc\release\vpid.exe .\dist\

# 2. 复制 Qt DLL（第三方XP版本）
Copy-Item "C:\Qt\6.5\xp\bin\Qt6Core.dll" .\dist\
Copy-Item "C:\Qt\6.5\xp\bin\Qt6Quick.dll" .\dist\
Copy-Item "C:\Qt\6.5\xp\bin\Qt6QuickControls2.dll" .\dist\
Copy-Item "C:\Qt\6.5\xp\bin\Qt6Qml.dll" .\dist\

# 3. 复制 QML 资源
Copy-Item -Recurse src\qml\ .\dist\qml\

# 4. 复制应用图标
Copy-Item resources\icons\icon.ico .\dist\

# 5. 生成依赖清单
dumpbin /dependents .\dist\vpid.exe > .\dist\dependencies.txt
```

---

## 9. 功能对照表

| 当前功能 | Qt6实现 | 状态 |
|----------|---------|------|
| HID设备列表 | nusb全USB设备列表 | 待实现 |
| 设备信息展示（VID/PID/名称） | TableView | 待实现 |
| 设备对比（新增/移除） | 两个TableView | 待实现 |
| 设为基准 | Rust状态管理 | 待实现 |
| 复制到剪贴板 | QClipboard | 待实现 |
| Toast通知 | Popup + Animation | 待实现 |
| 手动刷新 | 按钮 + Rust调用 | 待实现 |
| 热插拔自动检测 | nusb watch_devices | 待实现 |
| 退出应用 | Qt.quit() | 待实现 |

---

## 10. 实施计划（更新版）

### Phase 1: 环境搭建与 XP 验证（2天）
- [ ] 下载第三方 Qt6.5 for XP (https://github.com/pangshuai16/qt6_5_for_xp)
- [ ] 配置 CXX-Qt 0.8.1 开发环境
- [ ] 验证 XP Qt 与 CXX-Qt 兼容性
- [ ] 创建项目骨架（Cargo + build.rs）
- [ ] 测试纯 Cargo 构建流程

### Phase 2: Rust 后端（2天）
- [ ] 集成 nusb，实现设备枚举
- [ ] 实现热插拔监听
- [ ] 实现 CXX-Qt `#[cxx_qt::bridge]` 定义
- [ ] 实现 `UsbManager` QObject 类
- [ ] QML 模型绑定（`QVariantList` / `QQmlListProperty`）
- [ ] 单元测试

### Phase 3: QML UI（3天）
- [ ] 创建主窗口框架
- [ ] 实现设备表格组件
- [ ] 实现设备树视图
- [ ] 实现 Toast 通知
- [ ] 添加按钮操作（刷新、设为基准、复制、清空）
- [ ] **XP 适配**：禁用 OpenGL 效果，使用 2D 渲染

### Phase 4: 样式美化（1天）
- [ ] 实现 QSS 样式表
- [ ] 复刻 Pico CSS 卡片风格
- [ ] 添加悬停动画（使用 `Behavior` 而非 OpenGL）
- [ ] 深色模式支持

### Phase 5: 测试验证（2天）
- [ ] **Windows XP 测试**（关键）
  - [ ] 启动验证
  - [ ] USB 设备枚举
  - [ ] 热插拔检测
  - [ ] 对比功能
  - [ ] 剪贴板复制
- [ ] Windows 10+ 测试
- [ ] Linux 测试
- [ ] 功能回归测试
- [ ] 性能优化

### Phase 6: 跨平台构建（1天）
- [ ] 配置 GitHub Actions Matrix
- [ ] Windows XP Release 构建
- [ ] Windows 10+ Release 构建
- [ ] Linux Release 构建
- [ ] 打包分发脚本

---

## 11. 风险与注意事项

### 11.1 Windows XP 特殊风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| **第三方 Qt 稳定性** | 可能存 Bug | 充分测试，准备回退方案 |
| **仅 Release 构建** | 无法调试 | 使用 Release 带符号表，或远程调试 |
| **OpenGL 禁用** | 部分 QML 效果不可用 | 使用 2D 渲染，避免 ShaderEffect |
| **schannel 替代 OpenSSL** | 与 nusb 无冲突 | nusb 使用 WinUSB，不依赖 SSL |
| **YY-Thunks 兼容性** | 部分 API 可能不兼容 | 避免使用 Windows 10 特有 API |

### 11.2 驱动要求

nusb 在 Windows 上使用 WinUSB 驱动。对于非HID设备，可能需要：
- 使用 **WCID 描述符** 自动安装驱动
- 或使用 **Zadig** 手动安装 WinUSB
- 或使用 **libwdi** 打包驱动

### 11.3 Linux 权限

nusb 在 Linux 上使用 usbfs，可能需要：
- udev 规则配置
- 或用户加入 plugdev 组

### 11.4 macOS 权限

macOS 上可能需要：
- 启用"允许系统扩展"
- 或签署应用

### 11.5 CXX-Qt 版本锁定

```toml
# 锁定版本，避免未来不兼容
cxx-qt = "=0.8.1"
cxx-qt-lib = "=0.8.1"
cxx-qt-build = "=0.8.1"
```

---

**文档状态**: ✅ 设计完成（已更新 Windows XP 兼容方案）
**下一步**: 用户确认后，使用 `writing-plans` 技能创建详细实施计划
