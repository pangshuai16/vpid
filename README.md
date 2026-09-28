# USB 设备管理器 (vpid_viewer)

跨平台 USB 设备查看和管理工具：**Windows (XP 及以上) 与 Linux 共享同一套 SOUI5 UI 层**，平台差异只收敛在 USB 扫描 / 设备插拔通知。

## 核心逻辑（必须严格遵循）

1. **程序启动**：自动扫描一次 USB 设备，并将扫描结果设为基准列表
2. **每次扫描**：扫描的 USB 设备列表直接显示在"全部USB设备"中，并与基准列表进行比对——新增的 USB 设备显示在"新增设备"，减少的 USB 设备显示在"移除设备"
3. **设为基准**：点击【重置】按钮时，将当前"全部USB设备"列表设定为新的基准列表（清空变更记录）

## 功能

- 原生 USB 设备扫描（Windows SetupAPI + 注册表兜底 / Linux libusb）
- 显示 VID / PID / 设备名称 / 路径
- 基准比对：新增设备（绿色）/ 移除设备（红色）
- 自动刷新（100 ms 间隔）
- 复制设备信息到剪贴板
- Windows XP 兼容（32 位静态链接，零 DLL 依赖）
- Linux 多架构支持（x64 / arm64）

## 支持平台

| 平台 | 架构 | 支持 | UI 层 | 说明 |
|------|------|------|------|------|
| **Windows** | x86 (32位) | ✅ | SOUI5（GDI 渲染） | Windows XP 及以上，全静态链接 |
| **Linux** | x64 / arm64 | ✅ | SOUI5（swinx + Cairo） | glibc 2.28 及以上 |

> **统一 UI 架构**：Windows 与 Linux 使用**同一份** SOUI5 UI 代码与 XML 资源
> （`src/ui/`），平台差异全部收敛到 `src/core/` 的扫描器与设备通知实现。

## 技术栈

- **语言**: C++17
- **构建**: CMake (≥3.16) + Ninja / Visual Studio
- **UI 层（跨平台）**: **SOUI5** DirectUI 框架（vendored 于 `third_party/soui`，5.3.x）
  - Windows 渲染：`Render_Gdi` + `ImgDecoder_Stb`（XP 兼容，零外部依赖）
  - Linux 渲染：swinx（xcb + Cairo，SOUI5 官方跨平台方案）
  - 静态链接 `/MT`（Windows），子系统版本 5.1，导入表无 Vista+ API（XP 可运行）
  - Windows UI 资源（XML 布局 + 系统皮肤 PNG）全部编入 exe PE 资源，单文件自包含；
    Linux 从本地资源目录加载
- **USB 扫描**:
  - Windows: SetupAPI（设备枚举）+ 注册表兜底（`windows_scanner.cpp`）
  - Linux: libusb-1.0（`linux_scanner.cpp`）
- **设备插拔通知**（`device_notifier.h` 抽象 + 平台实现）:
  - Windows: `RegisterDeviceNotification` + `WM_DEVICECHANGE`
  - Linux: udev monitor（libudev，动态加载，缺失时降级为 100ms 定时轮询）
- **CI/CD**: GitHub Actions（Windows 2022 + MSVC / Ubuntu 22.04 + SOUI5+swinx / GTK3 回退）

> **可选回退**：旧实现保留在 `src/platform/legacy/`，可通过编译选项切换：
> - `-DVPID_UI_STYLE=WIN32`：MinGW + 原生 Win32 UI（Windows）
> - `-DVPID_UI_STYLE=GTK`：GTK3 UI（Linux）

## 下载与安装

从 [Releases](https://github.com/pangshuai16/vpid_fyne/releases) 页面下载对应平台的可执行文件：

- **Windows**: `vpid_viewer_windows_x86.exe`（XP 兼容，单文件免安装）
- **Linux**: `vpid_viewer_linux_amd64` 或 `vpid_viewer_linux_arm64`

## 从源码构建

```bash
# Windows (MSVC + SOUI5，XP 兼容；需 VS2017+ 或 CMake+Ninja 在 vcvarsall x86 环境中)
cmake -S . -B build -G "Ninja" -DCMAKE_BUILD_TYPE=Release -DVPID_UI_STYLE=SOUI
cmake --build build

# 或一键构建脚本（VS2017 x86 + 内置 CMake/Ninja）
build_msvc_soui.bat Release

# Linux (SOUI5 + swinx；需 cairo/fontconfig/xcb/libudev 开发包)
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DVPID_UI_STYLE=SOUI
cmake --build build -j$(nproc)

# 旧方案（可选）：MinGW 原生 Win32 UI
cmake -S . -B build -G "MinGW Makefiles" -DCMAKE_BUILD_TYPE=Release -DVPID_UI_STYLE=WIN32
cmake --build build -j$(nproc)

# 旧方案（可选）：Linux GTK3 UI
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DVPID_UI_STYLE=GTK
cmake --build build -j$(nproc)
```

产物输出到 `build/vpid_viewer.exe` (Windows) 或 `build/vpid_viewer` (Linux)。

## Windows XP 兼容说明

- 构建：`/SUBSYSTEM:WINDOWS,5.01` + 静态 CRT（`/MT`）+ `SOUI_XP_TOOLSET`（`_USING_V110_SDK71_`）
- 渲染后端必须用 `Render_Gdi`（不用 Skia/D2D，避免引入 Vista+ API）
- 已消除会破坏 XP 的导入：`GetTickCount64`（SOUI 源码内置 XP 兼容实现）、
  `GetNumaHighestNodeNumber`/`GetNumaProcessorNumber`（改用原生 `CreateThread`，
  不链接 UCRT 并发库 concrt）
- 验证：`dumpbin /imports vpid_viewer.exe` 确认导入 DLL 均为 XP 自带系统库

## 项目结构

```
CMakeLists.txt                # 顶层构建（core + ui 跨平台，平台实现按分支选择）
src/
  common/
    constants.h               # 常量配置（含自动刷新间隔）
  core/                       # ★ 跨平台业务核心（无 UI 框架依赖）
    device_info.h/.cpp        # USB 设备数据模型
    device_scanner.h/.cpp     # 扫描器抽象接口 + createScanner() 工厂
    device_comparer.h/.cpp    # 基准比对（新增/移除）
    device_notifier.h         # 设备插拔通知抽象（跨平台）
    windows_scanner.h/.cpp    # Windows 扫描器（SetupAPI + 注册表）
    linux_scanner.h/.cpp      # Linux 扫描器（libusb）
    windows_notifier.cpp      # Windows 设备通知（RegisterDeviceNotification）
    linux_notifier.cpp        # Linux 设备通知（udev monitor）
  ui/                         # ★ 跨平台 SOUI5 UI 层（Windows + Linux 共用）
    main.cpp                  # 应用入口（SApplication）
    MainDlg.h/.cpp            # 主窗口（SHostWnd 派生，事件/业务逻辑）
    uires/                    # SOUI UI 资源（XML 布局 + 颜色/字体表）
      uidef/init.xml          # 全局定义（字体/单位/样式）
      values/                 # color / skin / string / template
      xml/dlg_main.xml        # 主窗口布局（与原版 1:1 复刻）
    uires.rc2                 # Windows：uires 编入 exe PE 资源
platform/
  resources/                  # 平台专属资源（Windows app.rc/ico/manifest + 系统皮肤）
    app.rc / app.ico / app.manifest
    theme_sys_res.rc2 / theme_sys_res/
src/platform/legacy/         # 旧 UI 回退（可选，非默认）
    win32/main.cpp            # MinGW 原生 Win32 UI
    gtk/main.cpp              # Linux GTK3 UI
third_party/soui/             # SOUI5 vendored 源码（含 swinx submodule + XP 兼容补丁）
build_msvc_soui.bat           # MSVC+SOUI 一键构建（VS2017 x86）
assets/                       # 仓库文档 / 发布用图标
.github/workflows/
  build.yml                   # 分支构建（Windows MSVC + Linux SOUI + GTK 回退）
  release.yml                 # main 分支发布构建 + GitHub Release
```

## GitHub Actions 工作流

- **build.yml**：非 main 分支推送时构建验证（Windows x86 MSVC + Linux SOUI5/swinx + Linux GTK 回退）
- **release.yml**：main 分支推送时构建全部平台并自动创建 GitHub Release

自动发布的详细操作见 [RELEASE.md](RELEASE.md)，跨平台兼容性方案与 CI/CD 配置见 [XP_COMPATIBILITY.md](XP_COMPATIBILITY.md)。

## Linux 权限注意事项

在 Linux 上运行可能需要 USB 访问权限：

```bash
# 临时方案 (每次重启后需要)
sudo chmod 666 /dev/bus/usb/*/*

# 永久方案 (需要重启)
sudo usermod -aG plugdev $USER
# 或创建 udev 规则
echo 'SUBSYSTEM=="usb", MODE="0666", GROUP="plugdev"' | sudo tee /etc/udev/rules.d/99-usb.rules
sudo udevadm control --reload-rules
```

## 开发指南

### 添加新平台支持（如 macOS）

1. 在 `src/core/` 中实现扫描器（继承 `Scanner`）与设备通知（继承 `DeviceNotifier`）
2. IO/渲染层按 SOUI5 + swinx 官方流程对接（macOS 用 Cocoa + CGContext）
3. 在 `CMakeLists.txt` 增加平台分支（UI 层 `src/ui/` 无需改动）

### UI 修改

- 布局/样式：改 `src/ui/uires/xml/dlg_main.xml` 与 `values/`（Windows 需重新编译 RC 资源）
- 交互逻辑：改 `src/ui/MainDlg.h/.cpp`（跨平台共享）

### 贡献

欢迎提交 Issue 和 PR！

## 许可证

本项目使用 MIT 许可证。
外部依赖取得注意：SOUI5 与 swinx 均为"个人免费、商用需授权"的许可，商用前请联系作者获取授权。