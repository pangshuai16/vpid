# 发布指南（本地脚本，不依赖 GitHub Actions）

本项目已从 GitHub Actions 迁移到**本地固化编译脚本**：一条命令构建三平台
产物，验证通过后手动发布到 GitHub Release（API 直传）。

## 一、构建（三平台一次性产出）

```bash
python tools/build_release.py
```

产物输出到 `dist/`：

| 平台 | 架构 | 产物 | 体积 | 工具链 |
|------|------|------|------|--------|
| Windows | x86 | `vpid_viewer_windows_x86.exe` | ~1.5 MB | VS2022 BuildTools + MSVC 19.44 |
| Linux | amd64 | `vpid_viewer_linux_amd64` | ~8.3 MB | zig 0.17.0 交叉编译 |
| Linux | arm64 | `vpid_viewer_linux_arm64` | ~7.9 MB | zig 0.17.0 交叉编译 |

三个产物均为**单文件自包含**——UI 资源（XML 布局 + 系统皮肤）已内嵌进
ELF / PE，运行时**无需外置 `uires/` 目录**：

```bash
# Linux 单文件直接运行（示例）
chmod +x vpid_viewer_linux_amd64
./vpid_viewer_linux_amd64
```

常用变体：

```bash
python tools/build_release.py --only windows        # 只构建 Windows
python tools/build_release.py --only linux-x64      # 只构建 Linux amd64
python tools/build_release.py --only linux-arm64    # 只构建 Linux arm64
python tools/build_release.py --no-verify           # 跳过产物验证
python tools/build_release.py --dist out/           # 指定输出目录
```

构建完成后自动跑 `tools/verify_release.py` 校验每个产物：
- Windows：PE32 x86、GUI 子系统、DYNAMIC_BASE、无 HIGH_ENTROPY_VA、PE 时间戳固定并重算 CheckSum（构建可复现）
- Linux ELF：架构正确、glibc 版本符号 ≤ 2.17、动态依赖仅基础系统库
  （libc / libm / libpthread / libdl / librt / libxcb / libuuid）
- Linux ELF：内嵌 UI 资源段（`_binary_soui_coff_o_start/_end` 弱符号引用）

## 二、发布（GitHub Release API 直传）

```bash
python tools/release_github.py \
  --tag v1.3.3 \
  --title "v1.3.3" \
  --notes-file RELEASE_NOTES_v1.3.3.md
```

脚本会：
1. 推送当前分支到 origin
2. 打 annotated tag（`git tag -a`），先推 tag
3. 用 GitHub API 创建/更新 Release（凭据走 git-credential-manager）
4. 上传 `dist/` 下三个单文件产物，校验大小

Release 说明按版本归档为 `RELEASE_NOTES_vX.Y.Z.md`（`RELEASE_NOTES_v1.0.0.md`
~ `RELEASE_NOTES_v1.3.0.md`）。`--notes-file` 传对应版本的文件。

> 注意：仓库里历史 tag `v1.2.0` 属于**旧 Python/Fyne 版项目**（2026-05 发布，
> 资产 8.9 MB，Changelog 指向 `vpid_fyne` 仓库），当前 C++ 项目已跳过该版本号。

### 版本号约定（语义化版本）

- `vX.Y.Z`：X=大版本，Y=新功能，Z=修复。首个独立版本为 `v1.0.0`。
- 仓库历史 tag 为 `v22`~`v40`（旧 GitHub Actions `run_number` 风格），已废弃。

### 发布前检查清单

1. `python tools/build_release.py` 全绿（无 FAIL；Windows 分支内置 XP 导入硬门禁）
2. `python tools/verify_release.py` 全部通过（含 Windows XP 致命导入检查）
3. `git status` 干净（或已提交）
4. Windows 目标机注意：VC-LTL5 msvcrt 模式（CRT 走系统自带 `msvcrt.dll`），
   XP SP2 裸机可运行；Linux 目标机注意：需 USB 访问权限

## 三、构建细节

- **Windows**：**Ninja 生成器**（XP 三层机制的前置条件，见 `XP_COMPATIBILITY.md` §2.4）
  + MSVC `/Os /Gy /GR- /EHsc- /OPT:REF` + `/SUBSYSTEM:WINDOWS,5.01`。
  CRT 走 **VC-LTL5 msvcrt 模式**（`WindowsTargetPlatformMinVersion=5.1.2600.0`，
  重定向到 XP 自带 `msvcrt.dll`）+ YY-Thunks（Vista+ Win32 API 弱别名）+
  `src/xp_iat_stubs.asm`（`__imp_` IAT 槽兜底）→ **零致命 Vista+ 导入**。
  构建后自动跑 `tools/check_imports.py` 硬门禁（有致命 API 即失败）；
  `tools/pin_timestamp.py` 固定 PE 时间戳并重算 PE CheckSum 实现可复现构建。
- **Linux**：zig 0.17.0 `-target x86_64-linux-gnu.2.17` / `aarch64-linux-gnu.2.17`，
  glibc floor 2.17 → CentOS 7 / RHEL 7 / Ubuntu 12.04+ 可运行。
  系统库 `libxcb` / `libXau` / `libuuid` 从 `tools/linux-sysroot/{lib,lib-arm64}/`
  链接（已入库，离线可复现）。
- **UPX 禁用**：任何 UPX 压缩级别（含 `--best`）在 360 杀软下实测被查杀；
  当前发布策略仅依赖编译期体积优化 + strip，无 packer 特征。
- **构建目录**：`.build-release/`（脚本专用，git 忽略）。

相关文档：[XP_COMPATIBILITY.md](XP_COMPATIBILITY.md)（跨平台兼容性）、
[README.md](README.md)（从源码构建）。
