# v1.0.5 — Linux 单二进制自包含

## 重点：Linux 产物不再需要 `uires/` 资源目录

此前 Linux 产物必须与 `uires/` 目录并存（以 tar.gz 形式分发），
不能单文件直接运行。本版把 UI 资源**内嵌进 ELF**，做到真正的单文件：

```
chmod +x vpid_viewer_linux_amd64
./vpid_viewer_linux_amd64
```

### 实现方式

复刻 SOUI/swinx 官方的 windres 机制（`.rc` → COFF 资源对象 → 嵌入 ELF）：

1. 由系统皮肤与应用 `uires` 的 `.rc2` 清单生成标准 `.rc`；
2. `rc.exe` → `.res`，`cvtres.exe` → COFF 对象（本机无 MinGW `windres`，用
   Windows SDK 的 `rc` + MSVC 的 `cvtres` 等价实现）；
3. 离线应用 COFF 重定位并把 `.rsrc$01`（目录）与 `.rsrc$02`（数据）合并为
   单个 `.rsrc` 段，包成最小 COFF 对象（可被 swinx 直接解析，无需链接器）；
4. 以 `.incbin` 内联汇编把该对象作为数据嵌入 ELF；
5. swinx 侧以**弱符号**引用 `_binary_soui_coff_o_start/_end` 完成解析
   （不依赖 `--export-dynamic`，避免全量导出符号）。

## 修复

- **Linux 窗口图标缺失**：从 `app-icon.ico` 提供 `APP_ICON` 资源，
  不再出现 `APP_ICON not found`。

## 产物

| 文件 | 说明 |
|---|---|
| `vpid_viewer_windows_x86.exe` | Windows x86（XP 兼容，单文件） |
| `vpid_viewer_linux_amd64` | Linux x86-64，**单文件自包含** |
| `vpid_viewer_linux_arm64` | Linux aarch64，**单文件自包含** |

Linux 产物仍保持 glibc floor 2.17（CentOS 7 / Ubuntu 12.04+ 可运行），
动态依赖仅基础系统库（libc/libm/libpthread/libdl/librt/libxcb/libuuid）。
