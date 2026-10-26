# v1.3.3 — 兼容性优先 / 杀软友好型发布

本版继续保留 v1.3.2 的退出行为，不做功能改动，只优化发布产物的 Windows
兼容性、可复现性与杀软友好性。

## 本版变更

1. **PE 时间戳固定为常量 `0x64B0D000`（2022-01-01 UTC）**
   - 替代全零时间戳，避免部分安全审计 / 杀软把 `0` 时间戳视为异常。
   - 同源码同参数仍可复现构建哈希。

2. **固定时间戳后强制重算 PE CheckSum**
   - `tools/pin_timestamp.py` 现会按 PE 规范重算 CheckSum。
   - 发布门禁会拒绝 CheckSum 为 0 的产物。

3. **移除旧系统不友好的 DPI 清单声明**
   - `platform/resources/app.manifest` 不再声明 `dpiAwareness / PerMonitorV2`。
   - 保留基础 `dpiAware=true/pm`、`asInvoker`、`supportedOS`，降低 XP/旧系统兼容风险。

4. **强化发布验证门禁**
   - Windows 产物必须是 PE32 x86、GUI 子系统、`DYNAMIC_BASE`。
   - 拒绝 `HIGH_ENTROPY_VA`。
   - 拒绝 VC SxS / UCRT / vcruntime 依赖。
   - 拒绝空壳 Authenticode 证书目录。
   - 校验 section 结构和 manifest 关键兼容声明。

## 兼容性验证

- `Fatal Vista+ APIs: 0`
- Windows 产物：无 `ucrtbase.dll`、无 `vcruntime*`、无 `Microsoft.VC*.CRT`。
- PE headers：固定时间戳、非零 CheckSum、`5.01` 子系统、`0x8140` DLL 特性。
- Linux ELF：GLIBC floor 仍为 `2.17`。

## 产物

| 文件 | 说明 |
|---|---|
| `vpid_viewer_windows_x86.exe` | Windows XP SP2+ ~ Win11，32 位，单文件自包含 |
| `vpid_viewer_linux_amd64` | Linux x86-64，GLIBC ≥ 2.17 |
| `vpid_viewer_linux_arm64` | Linux aarch64，GLIBC ≥ 2.17 |
