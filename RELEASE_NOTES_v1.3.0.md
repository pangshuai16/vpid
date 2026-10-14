# v1.3.0 — Windows XP 兼容：三层通用机制，零 Vista+ 导入

## 背景

v143 (VS2022) 工具链的默认 CRT 与 Windows SDK 导入库会把一批 Vista+ API
写进 PE 导入表（`FlsAlloc`/`FlsFree`/`FlsGetValue`/`FlsSetValue`、`CompareStringEx`、
`FindFirstFileExW`、`InitializeCriticalSectionEx`、`GetFileSizeEx`、`SetFilePointerEx`
等，共 17 个）。XP 的 `kernel32.dll` 没有这些函数，加载器解析导入表时直接拒绝
执行——**即使这些 API 运行时从不被调用**。

一个关键反证：早先 UPX 时代的构建带 9 个同类致命导入却能在 XP 正常运行，
说明问题只是"加载器解析失败"，而非功能缺陷。因此本版在**链接期**根治，
不引入任何项目定制的 PE 二进制补丁。

## 解决方案：链接期三层机制（全部通用）

| 层 | 机制 | 消除 |
|---|---|---|
| 1 | **VC-LTL5**（msvcrt 模式，`WindowsTargetPlatformMinVersion=5.1.2600.0`）把 CRT 重定向到 XP 自带的 `msvcrt.dll` | 8 个 |
| 2 | **YY-Thunks kernel32.Lib** 排链接最前：内含 600+ 个 `_WeakAlias` 归档成员，把 Vista+ API 的直接调用弱别名到 thunk 实现 | 7 个 |
| 3 | **`src/xp_iat_stubs.asm`**（MASM）：为 YY-Thunks 未覆盖的 `GetFileSizeEx` / `SetFilePointerEx` 定义 `__imp_` IAT 槽数据符号，利用链接器"同名符号先到先得"使 SDK 导入成员不被拉入 | 最后 2 个 |

第 3 层针对的是 CRT 经 `call dword ptr [__imp__GetFileSizeEx@8]` 的**间接引用**——
YY-Thunks 的 weak symbol 只能覆盖函数直接调用，覆盖不了 IAT 数据槽。
IAT 槽指向 `src/xp_compat_stubs.cpp` 的本地 stub（用 XP 原生的
`GetFileSize` / `SetFilePointer` + 高低位合成实现等价语义，即使运行时真被调用也正确）。

**前提：Ninja 生成器。** VS 生成器把 SDK `kernel32.lib` 硬编码进
`AdditionalDependencies` 末尾，`/LIBPATH`、`link_directories(BEFORE)`、
`/NODEFAULTLIB` 均无法让 YY-Thunks 版本排到它前面；Ninja 生成器的
`LINK_LIBRARIES` 顺序完全可控。

## 验证

- `tools/check_imports.py`：**致命 Vista+ API = 0**
- 构建日志：`msvcrt Mode : 5.1.2600.0`（VC-LTL5 正确落在 msvcrt 模式）
- 13 个导入 DLL 全部 XP 原生；`kernel32` 含 `TlsAlloc`/`TlsGetValue`/`TlsSetValue`
  （YY-Thunks 的 Fls→Tls 回退证据）
- PE：`machine=x86` + `subsystem=5.01` + RT_MANIFEST 正确嵌入
- GUI 冒烟启动正常；`--scan` 实机识别 23 个设备

## 构建与发布链路变更

- `tools/build_release.py`：Windows 分支 **VS 生成器 → Ninja**（XP 机制的前置条件）
- 新增 **XP 导入硬门禁**：`tools/check_imports.py` 发现致命 API 时退出码非 0，
  `build_release.py` 与 `verify_release.py` 均调用它——不达标直接卡住发布
- 删除探索期脚本 `tools/build_ninja_xp.py` / `build_ninja_xp.ps1` / `verify_pe.bat`
- `.gitignore`：本地构建目录统一为 `build*/`

## 兼容层补丁（vendored SOUI）

- `SOUI/src/helper/SAppDir.cpp`：`IFileOpenDialog` / `FOS_PICKFOLDERS`（Vista+）
  以 `#if (_WIN32_WINNT >= 0x0600)` 门控，XP 下回退 `CFolderDialog`（功能等价）
- `utilities/src/helper/STls.cpp`：XP 目标下补 `InitOnceExecuteOnce` 前向声明

## 已知限制

`msvcrt` 模式下 CRT locale 无法指定 `.utf8`（VC-LTL5 官方 Bug 60——XP 自带的
`msvcrt.dll` 是 2001 年实现，不支持该区域名）。本项目不依赖 CRT locale：
源码 UTF-8，文本转换一律走宽字符 API（`MultiByteToWideChar(CP_UTF8, ...)`），
因此不受此限制影响。

## 产物

| 文件 | 体积 | 说明 |
|---|---|---|
| `vpid_viewer_windows_x86.exe` | 1,618,944 B (1.54 MiB) | Windows x86，**XP SP2 兼容**，单文件 |
| `vpid_viewer_linux_amd64` | 8,290,408 B (7.91 MiB) | Linux x86-64，单文件自包含 |
| `vpid_viewer_linux_arm64` | 7,890,384 B (7.52 MiB) | Linux aarch64，单文件自包含 |

Linux 产物保持 glibc floor 2.17（CentOS 7 / RHEL 7 / Ubuntu 12.04+ 可运行），
UI 资源内嵌 ELF，无需外置 `uires/`；动态依赖仅基础系统库
（libc / libm / libpthread / libdl / librt / libxcb / libuuid）。
