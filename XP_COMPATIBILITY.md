# 跨平台兼容性方案

本文档说明 vpid_viewer 如何在**Windows XP 及以上**与**Linux（CentOS 7+ /
Ubuntu 12.04+）**上做到统一 UI 层、单文件自包含。

## 1. 总览

| 维度 | Windows | Linux |
|------|---------|-------|
| 架构 | x86 (32 位) | x86_64 / aarch64 |
| 运行环境 | XP ~ Windows 11 | CentOS 7 / RHEL 7 / Ubuntu 12.04+ |
| 工具链 | VS2022 BuildTools + MSVC 19.44 | zig 0.17.0（Windows 宿主交叉） |
| UI 层 | SOUI5.3（GDI 渲染） | SOUI5.3（swinx + Cairo） |
| UI 资源 | 编入 PE 资源段 | 编入 ELF（`_binary_soui_coff_o_*` 弱符号） |
| 产物形态 | 单文件 exe，1.5 MB | 单文件 ELF，7.9–8.3 MB |
| 依赖 | 零运行时依赖（`/MT` 静态 CRT） | 仅基础系统库（libc/libm/libxcb/…） |

**统一 UI 层**：Windows 与 Linux 共用 `src/ui/` 的同一份 SOUI5 源码与
XML 布局（`src/ui/uires/`），平台差异全部收敛在 `src/core/` 的扫描器与
设备通知实现。

## 2. Windows XP 兼容性

### 2.1 工具链与编译选项

- **MSVC 19.44（VS2022 BuildTools）**，x86 target
- **静态 CRT**：`/MT`——XP 目标机无 VCRUNTIME
- **子系统版本**：`/SUBSYSTEM:WINDOWS,5.01`
- **XP 工具集宏**：`_USING_V110_SDK71_`（经 SOUI5 的 `SOUI_XP_TOOLSET` 宏启用），
  抑制新 SDK 中会引入 Vista+ 依赖的默认定义
- **Windows API 版本**：`_WIN32_WINNT=0x0501` / `WINVER=0x0501`

### 2.2 渲染后端约束

- 必须使用 SOUI5 的 `Render_Gdi` 后端（不用 Skia/D2D，避免 Vista+ API）
- 图像解码用 `ImgDecoder_Stb`（stb_image，纯 C，零依赖）
- 静态链接 `SOUI_ENABLE_CORE_LIB=ON` + `SOUI_ENABLE_COM_LIB=ON`

### 2.3 导入表清理

以下会破坏 XP 兼容的 API 已消除：

| 引入点 | 处理 |
|--------|------|
| `GetTickCount64` | SOUI5 源码内置 XP 兼容实现（< 64 位计数回退到 `QueryPerformanceCounter`） |
| `GetNumaHighestNodeNumber` / `GetNumaProcessorNumber` | 改用原生 `CreateThread`，不链接 UCRT 并发库 concrt |
| `GetExeDirA` 等宽/窄字符 API | v1.1.0 起全部清理为宽字符路径 |

**验证方法**：

```bash
dumpbin /imports vpid_viewer_windows_x86.exe
```

确认导入 DLL 仅为 XP 自带系统库：`kernel32.dll` / `user32.dll` / `gdi32.dll` /
`advapi32.dll` / `setupapi.dll` / `shell32.dll` / `comctl32.dll` 等。

### 2.4 体积与杀软

- 编译优化：`/Os /Gy /GR- /EHsc- /OPT:REF`（`/GR-` 前提：全库无 `dynamic_cast`/`typeid`；
  `/EHsc-` 前提：全库无 `throw`）
- **无 packer**：`DllCharacteristics = HIGH_ENTROPY_VA | NX_COMPAT | DYNAMIC_BASE`，
  静态链接后 strip，360 杀软实测放行
- **UPX 禁用**：`--best` / `--lzma` / 极端压缩在 360 下实测被查杀（2026-10-09
  中性文件名对照确认拦的是 UPX 压缩本身）
- 版本信息嵌入：`platform/resources/app.rc` 中 `VS_VERSION_INFO`（1.1.0.0）+
  `platform/resources/app.manifest`（`asInvoker` + DPI 感知）

## 3. Linux 兼容性

### 3.1 glibc floor 2.17

zig `-target x86_64-linux-gnu.2.17`（amd64）与
`aarch64-linux-gnu.2.17`（arm64）确保 glibc 版本符号 ≤ 2.17：

```bash
# 验证：提取产物中所有 GLIBC_X.Y.Z 版本符号
python -c "import re;d=open('dist/vpid_viewer_linux_amd64','rb').read();\
print(sorted({m.group() for m in re.finditer(rb'GLIBC_[0-9.]+' ,d)}))"
```

实测最高版本为 `GLIBC_2.17`（amd64）与 `GLIBC_2.17`（arm64），
兼容 CentOS 7 / RHEL 7 / Ubuntu 12.04+。

### 3.2 交叉编译（Windows 宿主一次产出双架构）

- **zig 0.17.0**（官方直链下载，装到 `C:/tools/`）
- 工具链文件：
  - `tools/zig-linux-x86_64.cmake`
  - `tools/zig-linux-aarch64.cmake`
- **编译器内联 argv**：`zig;cc;-target;x86_64-linux-gnu.2.17`（Windows cmd 会
  拆坏 `;` 分号列表，故工具链文件采用直接列表而非依赖变量展开）
- **`CMAKE_AR` 用 wrapper**：`tools/zig-ar.cmd`（同上原因）
- **禁用 PCH**：`CMAKE_DISABLE_PRECOMPILE_HEADERS ON`（SOUI5 跨平台 PCH 未适配 zig）
- **禁用 RPATH**：`CMAKE_SKIP_BUILD_RPATH ON`（避免链接期路径污染）
- **pixman 交叉探测预置退出码**（try_run 在交叉编译下要求预置退出码）：
  - `-DTOOLCHAIN_SUPPORTS_ATTRIBUTE_CONSTRUCTOR_EXITCODE=0`
  - `-DTOOLCHAIN_SUPPORTS_ATTRIBUTE_DESTRUCTOR_EXITCODE=0`
  - `-DASM_LEADING_UNDERSCORE_EXITCODE=1`

### 3.3 系统库 sysroot

Linux 产物的运行时依赖仅基础系统库：`libc` / `libm` / `libpthread` /
`libdl` / `librt` / `libxcb` / `libuuid`。

链接期需要的 `.so` stub 从 `tools/linux-sysroot/{lib,lib-arm64}/` 取
（`libxcb.so` / `libXau.so` / `libuuid.so` 等，从 Ubuntu deb 提取，
已入库离线可复现）：

- amd64：`tools/linux-sysroot/lib/`
- arm64：`tools/linux-sysroot/lib-arm64/`

刷新：`python tools/fetch_linux_sysroot.py`（走 `archive.ubuntu.com` 与
`ports.ubuntu.com` 官方源）。

### 3.4 USB 扫描与设备通知

- **扫描**：`libusb-1.0` 源码 vendored（`third_party/libusb` 1.0.27），静态链接
- **通知**：`udev` 动态加载（`dlopen`），系统缺失时降级为 100ms 定时轮询
  （无硬依赖）
- **权限**：`sudo chmod 666 /dev/bus/usb/*/*` 或 udev 规则

### 3.5 UI 资源内嵌

Linux 产物**不再是 tar.gz**（自 v1.0.5 起改为单文件自包含），链路：

1. `.rc2`（`src/ui/uires.rc2` + `platform/resources/theme_sys_res.rc2`）
2. 转标准 `.rc`（资源名/类型全大写）
3. `rc.exe` → `.res`，`cvtres.exe` → COFF 对象
4. 离线应用 COFF 重定位，合并 `.rsrc$01` + `.rsrc$02` → 单 `.rsrc` 段
5. 最小 COFF 对象 `.incbin` 内嵌进 ELF
6. swinx `resapi.cpp` 以**弱符号**引用 `_binary_soui_coff_o_start/_end`
   完成解析（不依赖 `--export-dynamic`，避免全量符号导出）

工具：`tools/gen_linux_coff_res.py`（生成 + 合并）、`tools/verify_coff_res.py`
（校验解析）。

**体积代价**：资源对象约 +114 KB（amd64 从 ~5.2 MB 增到 ~8.3 MB），
换来单文件发布（无外置 `uires/`）。

## 4. 构建入口

**一键脚本**（`tools/build_release.py`）：

```bash
python tools/build_release.py                    # 全平台
python tools/build_release.py --only windows     # 仅 Windows
python tools/build_release.py --only linux-x64   # 仅 Linux amd64
python tools/build_release.py --only linux-arm64 # 仅 Linux arm64
```

产物输出到 `dist/`，自动跑 `tools/verify_release.py` 校验（PE 有效、
ELF 架构正确、glibc 符号 ≤ 2.17、动态依赖仅基础库、UI 资源内嵌完整）。

完整发布流程见 [RELEASE.md](RELEASE.md)。

## 5. 兼容性检查清单

### Windows XP

- [x] `/SUBSYSTEM:WINDOWS,5.01` + 静态 CRT（`/MT`）
- [x] `_USING_V110_SDK71_`（`SOUI_XP_TOOLSET`）
- [x] `Render_Gdi` 渲染后端（无 Skia/D2D）
- [x] `GetTickCount64` / Numa* 等 Vista+ API 已消除
- [x] `DllCharacteristics` 含 `HIGH_ENTROPY_VA | NX_COMPAT | DYNAMIC_BASE`
- [x] 无 packer（UPX 禁用），360 实测放行

### Linux

- [x] zig `-target *-linux-gnu.2.17`，glibc 版本符号 ≤ 2.17
- [x] DT_NEEDED 仅基础系统库（libc/libm/libpthread/libdl/librt/libxcb/libuuid）
- [x] 无 RPATH 污染（`CMAKE_SKIP_BUILD_RPATH ON`）
- [x] libusb 源码 vendored（无系统开发包依赖）
- [x] udev 动态加载（缺失时降级轮询）
- [x] 单文件自包含（UI 资源内嵌 ELF）
- [x] amd64 + arm64 双架构（zig 交叉，Windows 宿主一次产出）

## 6. 常见问题

### Q: 为什么 Linux 走 zig 交叉编译而不是原生 Linux gcc？

- GitHub CI runner 无法稳定访问 gitee.com（`gnutls_handshake() failed`），
  SOUI5 曾作为 submodule 时构建必失败 → 改为 vendoring（`third_party/soui`）
- GitHub Actions arm64 QEMU 作业 360 分钟超时未跑通
- zig 单一工具链 + `-target gnu.2.17` 天然锁 glibc floor，一个 Windows
  宿主即可产出双架构，无环境漂移

### Q: 为什么不用 UPX 压缩？

UPX 任何压缩级别（含 `--best` + 去特征）在 360 下实测被查杀（2026-10-09
中性文件名对照确认拦的是 UPX 压缩本身、与文件名无关）。当前策略：
MSVC 体积优化（`/Os /Gy /GR- /EHsc- /OPT:REF`）+ strip 已把 Windows 产物
从 2.32 MB 压到 1.52 MB，无 packer 特征，安全性等同。

### Q: 为什么 Windows 保持 32 位？

- XP 仅有 32 位系统，i386 产物同时兼容 32/64 位 Windows，无额外打包成本
- SOUI5 在 x86 下渲染路径与体积最优（无 x64 冗余导入）

### Q: 如何验证产物兼容性？

```bash
# Windows：检查导入表无 Vista+ API
dumpbin /imports dist/vpid_viewer_windows_x86.exe

# Linux：检查 glibc 版本符号
objdump -T dist/vpid_viewer_linux_amd64 | grep GLIBC_

# Linux：检查动态依赖
readelf -d dist/vpid_viewer_linux_amd64 | grep NEEDED

# Linux：检查 UI 资源内嵌
strings dist/vpid_viewer_linux_amd64 | grep -c '_binary_soui_coff_o'

# 综合校验（构建后自动跑）
python tools/verify_release.py
```
