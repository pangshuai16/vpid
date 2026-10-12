# v1.1.0 — 正式版：应用内退出 + 调试输出清零

## 新增

- **应用内"退出"按钮**：应用栏最右侧新增"退出"（幽灵样式，不抢主按钮视觉）。
  与标题栏 `X` 共用同一收口 `MainDlg::OnClose()`——置 `closing_` 阻止在途扫描回填、
  停设备通知、停自动刷新定时器、销毁窗口，两条出口不重复维护清理逻辑。
  动机：部分 Linux 桌面（GNOME 面板最小化、某些 WM 隐藏非客户区按钮）下点到
  标题栏 `X` 不顺手，运维也习惯直接在界面里找"退出"。

## 清理（调试内容全部移除）

正式版不带任何诊断脚手架，默认启动 stdout / stderr 均为 0 字节：

| 移除项 | 位置 |
|---|---|
| `diag()` 启动追踪（`[vpid] start _tWinMain` / `DoConfig OK` / `ShowWindow OK` …） | `src/ui/main.cpp` |
| `VPID_DIAG` 环境变量开关及其残留注释 | `src/ui/main.cpp` |
| `vpid_linux_diag.txt` 诊断日志落盘 | `src/ui/main.cpp` |
| `vpid_scan_result.txt`（`--scan` 的磁盘兜底输出） | `src/ui/main.cpp` |
| `GetExeDirA()`（仅 diag 使用的窄字符路径版） | `src/ui/main.cpp` |
| 宽/窄字符路径一致性检查、`uires.idx` 存在性探测 | `src/ui/main.cpp` |
| `SAppCfg::SetLog(TRUE, 2, "vpid")` 日志配置 | `src/ui/main.cpp` |
| `load so failed` 缺库打印（`VPID_DIAG` 守卫一并移除） | `SOUI/src/control/SRichEdit.cpp` |
| `load so failed` 模块加载打印 | `utilities/include/com-loader.hpp` |

失败路径（OLE 初始化失败、资源加载失败、窗口创建失败）现在**静默返回**，
不再输出任何诊断信息。

保留项：`--scan` / `/scan` CLI 自检模式仍在（需显式传参才触发，正常使用时
不可见），但已删掉它在用户目录落地的 `vpid_scan_result.txt`，结果只走 stdout。

## 版本

- `platform/resources/app.rc`：`FILEVERSION` / `PRODUCTVERSION` /
  `FileVersion` / `ProductVersion` 由 `1.0.0.0` → `1.1.0.0`
  （资源管理器"属性→详细信息"读的是这里）。
- `src/common/constants.h` 的 `kAppVersion` 由失效的 `"2.0.0"` 对齐为 `"1.1.0"`
  （目前未在 UI 展示，预留给将来的"关于"对话框；已加注释标明与 app.rc 同步）。

## 产物

| 文件 | 体积 | 说明 |
|---|---|---|
| `vpid_viewer_windows_x86.exe` | 1,505,280 B (1.44 MiB) | Windows x86，XP 兼容，单文件 |
| `vpid_viewer_linux_amd64` | 8,290,408 B (7.91 MiB) | Linux x86-64，单文件自包含 |
| `vpid_viewer_linux_arm64` | 7,890,384 B (7.53 MiB) | Linux aarch64，单文件自包含 |

Linux 产物保持 glibc floor 2.17（CentOS 7 / Ubuntu 12.04+ 可运行），
UI 资源内嵌 ELF，无需外置 `uires/` 目录；动态依赖仅基础系统库
（libc / libm / libpthread / libdl / librt / libxcb / libuuid）。
