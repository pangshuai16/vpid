# 跨平台兼容性方案

本文档说明 vpid_viewer 如何在**Windows XP 及以上**与**Linux（CentOS 7+ /
Ubuntu 12.04+）**上做到统一 UI 层、单文件自包含。

> ✅ **XP 兼容性状态（2026-10-14 最终方案，零 Vista+ 导入）**：
>
> 采用 **VS2022 v143 + Ninja 生成器 + VC-LTL5 + YY-Thunks + MASM IAT stub**
> 四层通用机制，导入表已实现**零 Vista+ API**，`dumpbin /imports` 验证通过。
> 产物 `dist/vpid_viewer_windows_x86.exe`（1,618,944 字节，1.54MB）：
>
> - 机器类型 x86，子系统版本 5.01（XP）
> - 导入 DLL 仅 10 个 XP 原生库：`kernel32` / `user32` / `gdi32` / `advapi32` /
>   `setupapi` / `shell32` / `shlwapi` / `version` / `imm32` / `msimg32`
>   （另有 `ntdll` / `ole32`，XP 原生）
> - 无 `GetFileSizeEx` / `SetFilePointerEx` / `FlsAlloc` / `CompareStringEx` 等
>   任何 Vista+ API
>
> 三层消除机制（详见 2.4 节）：
> 1. **VC-LTL5**：CRT 重定向到 msvcrt.dll（XP 自带），规避 libcpmt/ucrt 的
>    Vista+ 依赖
> 2. **YY-Thunks kernel32.Lib（`_WeakAlias` 成员）+ YY-Thunks obj**：覆盖 7 个
>    Vista+ API 的直接调用（Fls*4 / CompareStringEx / FindFirstFileExW /
>    InitializeCriticalSectionEx），链接器解析到弱别名 thunk 而非 SDK 导入条目
> 3. **`src/xp_iat_stubs.asm`（MASM）**：为 YY-Thunks 未提供 thunk 的 2 个 API
>    （GetFileSizeEx / SetFilePointerEx）定义 `__imp__Xxx@N` IAT 槽数据符号，
>    指向 `src/xp_compat_stubs.cpp` 的本地 stub（回退 XP 的
>    GetFileSize / SetFilePointer）。.obj 优先于 .lib 处理 → 符号已定义 →
>    kernel32.lib 导入成员不会被拉入
>
> 该方案完全通用（无项目定制的 PE 二进制补丁），仅依赖社区标准工具
> VC-LTL5 + YY-Thunks + 一个 20 行 MASM 文件。

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
python tools/check_imports.py dist/vpid_viewer_windows_x86.exe
```

确认零致命 Vista+ API，导入 DLL 仅为 XP 自带系统库：`kernel32.dll` /
`user32.dll` / `gdi32.dll` / `advapi32.dll` / `setupapi.dll` / `shell32.dll` /
`shlwapi.dll` / `version.dll` / `imm32.dll` / `msimg32.dll` 等。

### 2.4 三层导入消除机制（2026-10-14 完整方案）

构建配置：**Ninja 生成器 + VC-LTL5 + YY-Thunks + MASM IAT stub**。
MSVC v143（VS2022）+ `/MT` 静态 CRT + `_WIN32_WINNT=0x0501`。

#### 第 1 层：VC-LTL5（CRT 重定向）

`VC-LTL helper for cmake.cmake` + `WindowsTargetPlatformMinVersion=5.1.2600.0`：
UCRT/VCRuntime 静态库重定向到 XP 自带的 `msvcrt.dll`。消除 CRT 本身对
Vista+ API 的绝大多数引用。

#### 第 2 层：YY-Thunks kernel32.Lib + obj（7 个 API）

```cmake
# target_link_libraries 最前排（必须早于 CMake 标准库里的 SDK kernel32.lib）：
"D:/DevTools/YY-Thunks/Lib/5.1.2600.0/x86/kernel32.Lib"
# YY-Thunks thunk 实现：
"D:/DevTools/YY-Thunks/Lib/5.1.2600.0/x86/YY_Thunks_for_5.1.2600.0.obj"
```

YY-Thunks 的 `kernel32.Lib` 内含 **`_WeakAlias` 归档成员**（每 API 一个
.obj），把 `_FlsAlloc@4` / `_CompareStringEx@12` 等**直接调用**弱别名到 obj
里的 thunk 实现（运行时检测真实 API 是否存在，存在则转发、不存在则回退）。
因为符号在导入库成员里已解析为弱别名，SDK `kernel32.lib` 的 short-import
成员不会被拉入 → 不创建致命 IAT 导入。

覆盖：`FlsAlloc` / `FlsFree` / `FlsGetValue` / `FlsSetValue` /
`CompareStringEx` / `FindFirstFileExW` / `InitializeCriticalSectionEx`。

#### 第 3 层：MASM IAT stub（剩余 2 个 API）

YY-Thunks 不为 `GetFileSizeEx` / `SetFilePointerEx` 提供 thunk（无
`_WeakAlias` 成员），而 CRT 内部通过 **IAT 间接引用**
（`call dword ptr [__imp__GetFileSizeEx@8]`）它们——weak symbol 机制覆盖不了
IAT 引用。解法是在自己的 .obj 里**定义 IAT 槽符号**，抢在导入库之前解析：

`src/xp_iat_stubs.asm`（MASM，`.model flat` 不加语言装饰，符号原样输出）：

```asm
externdef _GetFileSizeEx_stub@8:proc
externdef _SetFilePointerEx_stub@20:proc

public __imp__GetFileSizeEx@8
public __imp__SetFilePointerEx@20

.data
__imp__GetFileSizeEx@8    dd _GetFileSizeEx_stub@8
__imp__SetFilePointerEx@20 dd _SetFilePointerEx_stub@20
```

`src/xp_compat_stubs.cpp` 提供 stub 实现（回退 XP API）：

```cpp
extern "C" BOOL WINAPI GetFileSizeEx_stub(HANDLE hFile, PLARGE_INTEGER lpFileSize)
{
    DWORD hi = 0;
    DWORD lo = GetFileSize(hFile, &hi);   // XP 有
    if (lo == INVALID_FILE_SIZE && GetLastError() != NO_ERROR) return FALSE;
    if (lpFileSize) { lpFileSize->LowPart = lo; lpFileSize->HighPart = hi; }
    return TRUE;
}
```

链接顺序原理：**同名符号先到先得**——我们自己的 .obj 在链接命令行最前面
（CMake 目标自身源文件），`__imp__GetFileSizeEx@8` 已被定义为数据符号 →
`kernel32.lib` 的导入库成员（同时含 `_GetFileSizeEx@8` stub 与
`__imp__GetFileSizeEx@8` IAT）根本不会被检索 → 无致命导入。

> ⚠️ C++ 侧函数名**不能带前导下划线**：`extern "C"` 的 x86 装饰会再加一个
> `_`，`_Foo` 会变成 `__Foo@N`，与 asm 引用不匹配。用 `Foo` → `_Foo@N`。
>
> ⚠️ MASM 必须用 `.model flat`（**不带** `stdcall`/`C` 语言模型）：带语言模型
> 会给所有符号加下划线装饰，`__imp__X` 会变成 `___imp__X`。

#### 踩坑记录（已废弃的失败方案）

| 方案 | 失败原因 |
|------|---------|
| `#pragma comment(linker, "/ALTERNATENAME:__imp__GetFileSizeEx@8=...")` | `__imp_` 符号在 kernel32.lib 中**有定义**，不是未解析符号，alternatename 不触发 |
| `/NODEFAULTLIB:kernel32.lib` | CMake VS 生成器把 `kernel32.lib` 硬编码进 vcxproj `AdditionalDependencies`，NODEFAULTLIB 管不了 |
| VS 生成器 + `link_directories(BEFORE)` | VS 生成器的 `AdditionalLibraryDirectories` 覆盖 `LIB` 环境变量，BEFORE 失效 |
| VS 生成器 + 替换 `CMAKE_CXX_STANDARD_LIBRARIES` | VS 生成器独立硬编码默认库，无视该变量 |
| C++ 定义 `__imp_Foo` 函数指针变量 | C 链接装饰导致符号名不匹配（`___imp_` 三下划线） |
| PE 二进制补丁（`tools/pe_patch_xp.py`，已删） | 项目定制、非通用，且杀软风险 |
| UPX 压缩 | 360 实测误杀（2026-10-09）；且 UPX 时代版本虽能跑 XP，但不可发布 |

**Ninja 生成器是本方案的前提**：它把 `CMAKE_CXX_STANDARD_LIBRARIES` 与
`target_link_libraries` 原样拼进 `LINK_LIBRARIES`，链接顺序完全可控；
而 VS 生成器会独立硬编码 `AdditionalDependencies`/`AdditionalLibraryDirectories`。

#### 构建命令

```bash
# Windows x86 单平台（内部处理 vcvarsall x86 + Ninja，构建后自动跑 XP 导入硬门禁）
python tools/build_release.py --only windows

# 三平台统一（发布用）
python tools/build_release.py
```

> 探索期脚本 `tools/build_ninja_xp.py`、`tools/build_ninja_xp.ps1`、`tools/verify_pe.bat`
> 已删除：Ninja 构建逻辑并入 `tools/build_release.py`，导入审计并入 `tools/check_imports.py`
> （并作为硬门禁被 `tools/build_release.py` 与 `tools/verify_release.py` 调用——
> 发现致命 Vista+ API 时退出码非 0，直接卡住发布）。

### 2.5 体积与杀软

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
