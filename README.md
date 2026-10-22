# USB 设备管理器 (vpid_viewer)

跨平台 USB 设备查看和管理工具：**Windows（XP 及以上）与 Linux（CentOS 7+ /
Ubuntu 12.04+）共享同一套 SOUI5 UI 层**，平台差异只收敛在 USB 扫描与
设备插拔通知。

## 核心逻辑

1. **程序启动**：自动扫描一次 USB 设备，并将扫描结果设为基准列表
2. **每次扫描**：扫描结果直接显示在"全部 USB 设备"中，与基准列表比对——
   新增设备显示在"新增设备"，减少的设备显示在"移除设备"
3. **设为基准**：点击【重置】按钮时，将当前"全部 USB 设备"列表设定为新的
   基准列表（清空变更记录）
4. **退出**：标题栏 `X` 与应用栏最右"退出"按钮共用 `MainDlg::OnClose()`——
   置 `closing_` 阻止在途扫描回填、停设备通知、停自动刷新定时器、销毁窗口

## 功能

- 原生 USB 设备扫描（Windows SetupAPI + 注册表兜底 / Linux libusb-1.0）
- 显示 VID / PID / 设备名称 / 路径
- 基准比对：新增设备（绿色）/ 移除设备（红色）
- 自动刷新（100 ms 间隔，可由 udev 事件触发更快响应）
- 复制设备信息到剪贴板
- Windows XP 兼容（32 位静态链接，零 DLL 依赖）
- Linux 多架构（amd64 / arm64），单文件自包含
- CLI 自检：`--scan` 打印设备列表（便于 CI/脚本验证）

## 支持平台

| 平台 | 架构 | 支持 | UI 层 | 说明 |
|------|------|------|------|------|
| **Windows** | x86 (32 位) | ✅ | SOUI5（GDI 渲染） | XP 及以上，全静态链接 |
| **Linux** | x86_64 / aarch64 | ✅ | SOUI5（swinx + Cairo） | glibc 2.17 及以上 |

> **统一 UI 架构**：Windows 与 Linux 使用**同一份** SOUI5 UI 代码与 XML
> 资源（`src/ui/`），平台差异全部收敛到 `src/core/` 的扫描器与设备通知实现。

## 技术栈

- **语言**: C++17
- **构建**: CMake ≥ 3.16 + **Ninja**（Windows 产物必须用 Ninja——
  XP 兼容三层机制依赖链接顺序完全可控，VS 生成器不满足）
- **UI 层（跨平台）**: **SOUI5.3** DirectUI 框架（全局环境 `D:/DevTools/SOUI`，含 XP 补丁，
  版本 5.3.1.2）
  - Windows 渲染：`Render_Gdi` + `ImgDecoder_Stb`（XP 兼容，零外部依赖）
  - Linux 渲染：swinx（xcb + Cairo，SOUI5 官方跨平台方案）
  - Windows：**VC-LTL5 msvcrt 模式**（CRT 重定向到 XP 自带 `msvcrt.dll`）
    + YY-Thunks + MASM IAT stub 三层机制，子系统版本 5.01，
    **导入表零致命 Vista+ API**（见 `XP_COMPATIBILITY.md`）
  - **UI 资源内嵌**：Windows 编入 PE 资源段；Linux 编入 ELF
    （`_binary_soui_coff_o_*` 弱符号引用）——三平台均单文件自包含
- **USB 扫描**:
  - Windows: SetupAPI（设备枚举）+ 注册表兜底（`src/core/windows_scanner.cpp`）
  - Linux: libusb-1.0（vendored 于 `third_party/libusb`，源码静态链接）
- **设备插拔通知**（`device_notifier.h` 抽象 + 平台实现）:
  - Windows: `RegisterDeviceNotification` + `WM_DEVICECHANGE`
  - Linux: udev monitor（`dlopen` 动态加载，缺失时降级为 100 ms 定时轮询）
- **发布**: 本地固化脚本 `tools/build_release.py` 一次构建三平台 +
  `tools/release_github.py` API 直传 GitHub Release（不依赖 GitHub Actions）

## 下载与安装

从 [Releases](https://github.com/pangshuai16/vpid/releases) 页面下载
对应平台的可执行文件：

- **Windows**: `vpid_viewer_windows_x86.exe`（~1.5 MB，XP 兼容，单文件免安装）
- **Linux amd64**: `vpid_viewer_linux_amd64`（~8.3 MB，单文件自包含）
- **Linux arm64**: `vpid_viewer_linux_arm64`（~7.9 MB，单文件自包含）

Linux 使用：

```bash
chmod +x vpid_viewer_linux_amd64
./vpid_viewer_linux_amd64
```

## 从源码构建

### 一键 Release 构建（推荐）

Windows 宿主上一次产出三个平台产物（需 VS2022 BuildTools + zig 0.17.0 在 PATH）：

```bash
python tools/build_release.py
# 产物：dist/vpid_viewer_windows_x86.exe
#       dist/vpid_viewer_linux_amd64
#       dist/vpid_viewer_linux_arm64
#       （自动验证 PE/ELF/glibc 版本/UI 资源内嵌）
python tools/build_release.py --only windows     # 只构建 Windows
python tools/build_release.py --only linux-x64   # 只构建 Linux amd64
python tools/build_release.py --only linux-arm64 # 只构建 Linux arm64
```

发布到 GitHub Release（无需 Actions）：

```bash
python tools/release_github.py \
  --tag v1.3.2 \
  --title "v1.3.2" \
  --notes-file RELEASE_NOTES_v1.3.2.md
```

完整发布流程见 [RELEASE.md](RELEASE.md)，跨平台兼容性细节见
[XP_COMPATIBILITY.md](XP_COMPATIBILITY.md)。

### 手动构建（分平台）

```bash
# Windows (MSVC + SOUI5，XP SP2 兼容；在 vcvarsall.bat x86 之后运行)
#   * 必须用 Ninja 生成器：XP 三层机制依赖链接顺序可控——VS 生成器会把 SDK
#     kernel32.lib 硬编码到 YY-Thunks 之前，_WeakAlias 失效（见 XP_COMPATIBILITY.md §2.4）。
#   * 构建后建议跑导入审计（应为零致命 Vista+ API）：
#       python tools/check_imports.py build/vpid_viewer.exe
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build

# Linux 原生 (SOUI5 + swinx；需 cairo/fontconfig/xcb 开发包，本机 Linux 环境)
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build -j$(nproc)

# Linux 交叉编译（Windows 宿主上用 zig v0.17.0，glibc floor 2.17）
#   * libusb 已 vendored（third_party/libusb）；libudev 运行时 dlopen 降级，
#     无需系统开发包。
#   * 系统库 libxcb/libXau/libuuid 已入库（tools/linux-sysroot/），无需联网。
#   * 3 个 EXITCODE 是 pixman 交叉探测必需（tryRun 在交叉编译下要求预置退出码）。
cmake -S . -B build-zig -G Ninja \
  -DCMAKE_TOOLCHAIN_FILE=$PWD/tools/zig-linux-x86_64.cmake \
  -DCMAKE_BUILD_TYPE=Release \
  -DTOOLCHAIN_SUPPORTS_ATTRIBUTE_CONSTRUCTOR_EXITCODE=0 \
  -DTOOLCHAIN_SUPPORTS_ATTRIBUTE_DESTRUCTOR_EXITCODE=0 \
  -DASM_LEADING_UNDERSCORE_EXITCODE=1
cmake --build build-zig --config Release
# 产物：build-zig/bin/vpid_viewer（ELF x86-64，glibc <= 2.17 可运行，已 strip）
# arm64：换用 tools/zig-linux-aarch64.cmake，产物 ELF aarch64。
```

## 项目结构

```
CMakeLists.txt                # 顶层构建（core + ui 跨平台共享）
src/
  common/
    constants.h               # 常量配置（含自动刷新间隔、kAppVersion）
  core/                       # ★ 跨平台业务核心（无 UI 框架依赖）
    device_info.h/.cpp        # USB 设备数据模型
    device_scanner.h/.cpp     # 扫描器抽象接口 + createScanner() 工厂
    device_comparer.h/.cpp    # 基准比对（新增/移除）
    device_notifier.h         # 设备插拔通知抽象
    windows_scanner.h/.cpp    # Windows 扫描器（SetupAPI + 注册表）
    linux_scanner.h/.cpp      # Linux 扫描器（libusb）
    windows_notifier.cpp      # Windows 设备通知（RegisterDeviceNotification）
    linux_notifier.cpp        # Linux 设备通知（udev monitor）
  ui/                         # ★ 跨平台 SOUI5 UI 层（Windows + Linux 共用）
    main.cpp                  # 应用入口（SApplication）
    MainDlg.h/.cpp            # 主窗口（SHostWnd 派生，事件/业务逻辑）
    uires.rc2                 # Windows：uires 编入 exe PE 资源
    uires/                    # SOUI UI 资源
      uidef/init.xml          # 全局定义（字体/单位/样式）
      values/                 # color / skin / string / template
      xml/dlg_main.xml        # 主窗口布局
      uires.idx               # 资源索引（应用资源名 → 文件路径映射）
platform/
  resources/                  # 平台专属资源（Windows app.rc/ico/manifest + 系统皮肤）
    app.rc / app.ico / app.manifest
    theme_sys_res.rc2 / theme_sys_res/
tools/                        # 构建 / 验证 / 发布脚本
  build_release.py            # 三平台一键构建入口
  verify_release.py           # 产物验证（PE/ELF/glibc/UI 资源）
  gen_linux_coff_res.py       # Linux UI 资源 COFF 内嵌
  verify_coff_res.py          # COFF 资源解析校验
  release_github.py           # GitHub Release API 直传
  fetch_linux_sysroot.py      # 刷新 linux sysroot（可选）
  pin_timestamp.py            # 固定 PE 时间戳（可复现构建）
  zig-linux-{x86_64,aarch64}.cmake  # zig 交叉工具链文件
  linux-sysroot/              # 交叉链接用系统库（已入库，离线可复现）
assets/                       # 图标资源（仓库文档 / 发布用）
third_party/
  soui/                       # SOUI5 vendored 源码（含 swinx + XP 兼容补丁）
  libusb/                     # libusb-1.0 vendored 源码
dist/                         # 构建产物输出目录（git 忽略）
```

## Linux 权限注意事项

在 Linux 上运行可能需要 USB 访问权限：

```bash
# 临时方案（每次重启后需要）
sudo chmod 666 /dev/bus/usb/*/*

# 永久方案（需要重启）
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

- 布局/样式：改 `src/ui/uires/xml/dlg_main.xml` 与 `values/`
  （Windows 需重新编译 RC 资源，Linux 需重跑 `tools/gen_linux_coff_res.py`）
- 交互逻辑：改 `src/ui/MainDlg.h/.cpp`（跨平台共享）
- 版本号：`platform/resources/app.rc` 的 `VS_VERSION_INFO` 与
  `src/common/constants.h` 的 `kAppVersion` 保持同步

### 贡献

欢迎提交 Issue 和 PR。

## 许可证

本项目使用 MIT 许可证。

外部依赖注意：SOUI5 与 swinx 均为"个人免费、商用需授权"的许可，商用前请
联系作者获取授权。
