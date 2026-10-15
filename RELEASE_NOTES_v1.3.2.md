# v1.3.2 — 移除退出诊断插桩

v1.3.1 为了在 XP 上定位退出残留问题，加了一套内建退出诊断日志
（`src/common/trace.h`，写 `%TEMP%\vpid_exit.log`）。定位已完成、修复已验证，
本版把这批临时插桩全部撤掉，恢复正式产物应有的干净状态。

**退出行为与 v1.3.1 完全一致，没有任何功能变化。** 修复逻辑一行未改，
只是删掉了记录退出序列的打点。

## 本版变更

1. **删除 `src/common/trace.h`** 及其在 3 个源文件中的全部调用点：
   `main.cpp`（5 处，含启动时的 `GetVersion` 打点）、`MainDlg.cpp`（8 处）、
   `windows_scanner.cpp`（1 处）。顺带移除了仅为诊断存在的
   `GetVersion()` 调用与 `DWORD64` 计时变量。
2. **修正 `platform/resources/app.rc` 的 `FILEVERSION` / `PRODUCTVERSION`**。
   此前一直停留在 `1,3,0,0`（v1.2.x 时代遗留），与 `FileVersion` /
   `ProductVersion` 字符串不同步。资源管理器"属性"里显示的版本号来自
   `FILEVERSION`，因此之前看到的是错的。现在四处统一为 `1,3,2,0` / `1.3.2.0`。
3. `kAppVersion` `1.3.1` → `1.3.2`；`README.md` / `RELEASE.md` 的发布命令示例同步。

## 保留不动的部分

以下 v1.3.1 引入的修复全部保留，这是 XP 退出正常的实际原因：

- `theApp.Run()` 返回后直接 `::ExitProcess()`，跳过 SOUI 析构与 CRT 退出清理
- 不调用 `OleUninitialize()`
- 扫描可取消（`RequestScanCancel()` / `ScanCancelRequested()`）
- `OnClose` 顺序：请求取消 → `DestroyWindow` → join → `PostQuitMessage`
- 扫描线程上下文只持 `HWND`，不持 `this`

## 产物体积

| 产物 | v1.3.1 | v1.3.2 | 变化 |
|---|---|---|---|
| `vpid_viewer_windows_x86.exe` | 1,626,624 B | **1,620,480 B** | −6,144 B |
| `vpid_viewer_linux_amd64` | 8,290,408 B | **8,291,608 B** | +1,200 B |
| `vpid_viewer_linux_arm64` | 7,890,384 B | **7,891,632 B** | +1,248 B |

Windows 侧因删掉插桩代码而缩小；Linux 侧略增是因为不再需要 `trace.h` 的
空实现分支，但版本号文本变长、以及本次修正了 `FILEVERSION`/`PRODUCTVERSION`
（这两个数值此前是 `1,3,0,0`，资源段内容随之变化）。

## 回归

- `Fatal Vista+ APIs: 0`（XP 硬门禁）
- `verify_release.py` 全平台通过：PE x86 / ELF amd64+arm64 GLIBC 2.17 /
  DT_NEEDED 白名单 / 内嵌资源段
- 产物内 `vpid_exit.log`、`VPID_NOTRACE` 及全部里程碑字面量均已消失
- 产物内 `1.3.2.0` 存在（UTF-16 ×2 = FileVersion + ProductVersion），
  `1.3.1.0` 已消失
- Win11 退出压力测试 8 轮（存活 0.02–2.0s，覆盖扫描在途与空闲）：
  全部正常退出、无残留，退出耗时 19.5–110.0ms（均值 40.7ms），返回码 1 与 v1.3.1 一致
- XP 实机退出无残留（v1.3.1 已验证，本版退出路径未改动）

## 产物

| 文件 | 说明 |
|---|---|
| `vpid_viewer_windows_x86.exe` | Windows XP SP2+ ~ Win11，32 位，单文件自包含 |
| `vpid_viewer_linux_amd64` | Linux x86-64，GLIBC ≥ 2.17 |
| `vpid_viewer_linux_arm64` | Linux aarch64，GLIBC ≥ 2.17 |

> 若日后在 XP 上再次遇到退出类问题，插桩模板与完整方法论已沉淀到
> 本地 `xp-build` skill（`references/xp_exit_clean_exit.md` +
> `scripts/trace_exit_template.h`）。
