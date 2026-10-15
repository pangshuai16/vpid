# v1.3.1 — Windows XP 退出残留修复

v1.3.0 已解决 XP **加载**问题（零 Vista+ 导入），但在 XP 上点退出后，
进程仍残留在任务管理器中（窗口已关闭、内存占用逐渐变小、进程却不终止）。
Win10/11 上从不复现。本版彻底修复。

## 现象

窗口关闭后进程残留。内存占用会**逐渐变小**（GDI/窗口资源已释放）但进程不退出，
只能在任务管理器里手动结束。

## 定位过程：四轮，靠内建退出日志收敛

v1.3.0 时代 Win11 无法复现，只能盲猜。本版加了内建退出诊断日志
（`%TEMP%\vpid_exit.log`，环境变量 `VPID_NOTRACE=1` 可关），把 XP 上的
退出序列打成时间线后一轮锁定。**结论：第一轮、第二轮的假设都是错的。**

| 轮次 | 假设 | XP 日志判决 |
|---|---|---|
| 1 | 扫描线程 UAF（持 `this` 读写成员） | 部分成立——UAF 确实在，但不是残留原因 |
| 2 | `WaitForSingleObject` 2s 超时后线程仍存活 | 日志显示 `join: result=0`，线程已干净退出 |
| 3 | `OleUninitialize` × 剪贴板所有权死锁 | 日志中 `clipboard` 出现 0 次（从未复制），假设证伪 |
| 4 | 卡在 `~SApplication()` 的 SOUI 收尾 | **确认**——日志止于 `SApplication::Run returned` |

## 根因

`SApplication::Run()` 正常返回后，`_tWinMain` 栈展开要依次析构
`MainDlg` / `SAppCfg` / `SApplication`，其中 `~SApplication()` 执行一串
SOUI 收尾（`SResProviderMgr::RemoveAll`、`_DestroySingletons` 销毁
`SNotifyCenter`/`SHostMgr`/`SUiDef`/`STimerGenerator`/`SWindowMgr`、
`SRichEdit::UninitTextService`、`SNativeWndHelper::Uninit`），
在 XP 的 msvcrt 环境下挂死。之后 CRT 还会跑静态析构与堆回收，
那是 XP 时代另一片高风险区。

日志证据：`~MainDlg` 已完成（其打点已出现），`~SAppCfg` 仅 4 个 `delete`，
故唯一可能就在这串 SOUI 收尾里。

## 修法

`theApp.Run()` 返回后**直接 `::ExitProcess()`**，跳过全部析构与 CRT 退出清理。

安全性依据——**到这一步没有任何资源需要程序自己归还**：

- 窗口已在 `OnClose` 中 `DestroyWindow`
- 扫描线程已在 `ShutdownScanThread` 中 join（`WaitForSingleObject`，限时 15s）
- 消息循环已通过 `PostQuitMessage` 正常退出
- 剩余内存、GDI、句柄、剪贴板所有权、COM/OLE 状态全部由操作系统在进程终止时回收

`ExitProcess` 还额外不执行 C++ 静态析构与 `atexit` 回调
（`exit()` 会跑它们）——这避开了 XP 上 msvcrt 堆回收/TLS 清理的另一处风险。
它仍会执行各模块的 `DLL_PROCESS_DETACH(fDllUnloading=TRUE)`，
这是所有从不调用 `OleUninitialize` 的 Win32 程序的正常安全路径。

配套的两处改动（XP 上已实测有效，予以保留）：

- **移除 `OleUninitialize()`**：进程终止时 OLE 状态由 OS 回收，无需本线程收尾。
- **扫描可取消**：新增 `RequestScanCancel()`/`ScanCancelRequested()`
  （文件静态 `std::atomic<bool>`，`scanUsbDevices()` 入口复位），
  枚举循环顶部检查；退出时取消并**跳过昂贵的 `DIGCF_ALLCLASSES` 全枚举兜底**。
  `OnClose` 顺序为：请求取消 → `DestroyWindow`（UI 立即消失）→ join 线程 → `PostQuitMessage`。

## 诊断日志（新增）

`src/common/trace.h`（header-only，仅用 NT 3.1 时代 API，不影响 XP 门禁）写入
`%TEMP%\vpid_exit.log`，记录 OS 版本、每次扫描的路径与耗时、退出各里程碑。
用于 XP 等无法直接调试的环境定位退出问题；`VPID_NOTRACE=1` 可关闭。
日志确认无害后建议移除。

## 已知行为

- XP 上 `GUID_DEVINTERFACE_USB_DEVICE` 主路径返回空，每次都落到
  `DIGCF_ALLCLASSES` 全枚举。XP 实测单次扫描 86–237ms（n=6 设备），
  Win11 约 50ms（n=18）。自动刷新周期 100ms 下界面仍流畅。
- 退出返回码为 1（`_tWinMain` 原返回值），与 v1.3.0 一致。

## 回归

- `Fatal Vista+ APIs: 0`（XP 硬门禁）
- Windows x86：**1,626,624 B**
- Linux amd64 8,290,408 B / arm64 7,890,384 B，GLIBC 2.17，内嵌资源 OK
- Win11 退出压力测试 8 轮（存活 0.02–2.0s，覆盖扫描在途与空闲）
  全部退出成功，耗时 33–58ms（均值 39.5ms），无残留
- XP 实机退出无残留（本次发版依据）

## 产物

| 文件 | 说明 |
|---|---|
| `vpid_viewer_windows_x86.exe` | Windows XP SP2+ ~ Win11，32 位，单文件自包含 |
| `vpid_viewer_linux_amd64` | Linux x86-64，GLIBC ≥ 2.17 |
| `vpid_viewer_linux_arm64` | Linux aarch64，GLIBC ≥ 2.17 |
