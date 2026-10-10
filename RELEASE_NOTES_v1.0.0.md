# vpid_viewer v1.0.0

USB 设备管理器——C++/SOUI 原生实现，单文件免安装，XP 兼容。

## 本版亮点（首版独立发布）

- **全平台原生 C++/SOUI**：Windows x86（MSVC，XP 兼容）+ Linux amd64/arm64（zig 交叉编译，glibc floor 2.17，兼容 CentOS 7 / RHEL 7 / Ubuntu 12.04+）
- **Linux 交叉编译打通**（zig 0.17.0，Windows 宿主一次产出双架构 ELF）：
  - glibc 版本符号 ≤ 2.17，同一产物在旧版 RHEL/CentOS/Ubuntu 均可运行
  - DT_NEEDED 仅基础系统库（libc/pthread/dl/rt/m + libxcb/libuuid），无 RPATH 污染
  - 5.2MB (amd64) / 4.9MB (arm64)，链接期 strip
- **libusb vendored**：Linux USB 扫描从源码静态链接（第三_party/libusb 1.0.27），不依赖系统 libusb 开发包
- **发布流程本地化**：不再依赖 GitHub Actions，`tools/build_release.py` 一条命令构建三平台 + 自动验证（PE/ELF/glibc 版本），`tools/release_github.py` API 直传 GitHub Release

## 功能

- USB 设备枚举与实时监控（插拔自动刷新）
- 设备详情查看（VID/PID、厂商、序列号等）
- 设备对比（多设备差异高亮）
- Windows：SetupAPI + 注册表兜底扫描；Linux：libusb + udev 通知（缺失时降级轮询）
- 高分屏 DPI 感知、系统皮肤、跨平台 XML 布局（SOUI5）
- CLI 自检模式：`--scan` 打印设备列表（便于 CI/脚本验证）

## 产物

| 文件 | 平台 | 说明 |
|------|------|------|
| `vpid_viewer_windows_x86.exe` | Windows x86 | 1.5MB，XP~Win10 兼容，无 packer 特征（360 实测放行） |
| `vpid_viewer_linux_amd64` | Linux amd64 | 5.2MB，glibc ≥ 2.17 |
| `vpid_viewer_linux_arm64` | Linux arm64 | 4.9MB，glibc ≥ 2.17；aarch64 仅需 root 运行 |

## Linux 运行注意

- 需要 USB 访问权限：`sudo chmod 666 /dev/bus/usb/*/*` 或配置 udev 规则
- 运行需要 X11（xcb）与基础字体库，桌面发行版自带