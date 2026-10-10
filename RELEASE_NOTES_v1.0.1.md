# vpid_viewer v1.0.1

**修复：Linux 版 GUI 静默退出（实测 arm64 无响应无进程）**

## 根因

Linux ELF 产物没有 PE 资源段，但代码写死了 `RESTYPE_PE`（从 PE 资源加载 UI）。
swinx 的 COFF 资源嵌入机制（windres 编 .rc → `_binary_soui_coff_o_*` 符号）未启用，
导致 `FindResource` 全部失败 → SOUI 初始化失败 → **静默 `return -1` 退出**
（无报错、无窗口、无进程，与实测现象完全一致）。

## 修复

- **main.cpp**：Linux 分支改用**文件资源提供者**（`SetSysResFile`/`SetAppResFile`），
  从 exe 同目录 `uires/` 加载 UI 资源；Windows 保持 PE 资源不变
- **新增 `src/ui/uires/uires.idx`**：应用资源索引（布局/主题 XML，资源名与
  uires.rc2 一致，SOUI 资源查找大小写不敏感）
- **Linux 产物改为 tar.gz 包**（单文件发布）：内含 ELF + `uires/` 资源目录
  （应用 XML + 系统皮肤），解压即用

## 产物

| 文件 | 说明 |
|------|------|
| `vpid_viewer_windows_x86.exe` | Windows x86，1.5MB，XP 兼容（不变） |
| `vpid_viewer_linux_amd64.tar.gz` | Linux amd64，解压即用（内含 uires/） |
| `vpid_viewer_linux_arm64.tar.gz` | Linux arm64，解压即用（内含 uires/） |

## 使用

```bash
# Linux：解压后直接运行（保持 uires/ 与 ELF 同目录）
tar xzf vpid_viewer_linux_arm64.tar.gz
./vpid_viewer_linux_arm64
```

## 其他说明

- glibc floor 2.17（CentOS 7 / RHEL 7 / Ubuntu 12.04+ 兼容）保持不变
- 新增 `tools/verify_release.py` 对 tar.gz 包做结构校验（ELF/uires.idx/皮肤/布局）