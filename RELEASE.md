# 发布指南（本地脚本，不再依赖 GitHub Actions）

本项目已从 GitHub Actions 迁移到**本地固化编译脚本**。一次命令构建全部平台
产物，验证通过后手动发布到 GitHub Release（API 直传，无 CI）。

## 一、构建（三平台一次性产出）

```bash
python tools/build_release.py
```

产物输出到 `dist/`：

| 平台 | 架构 | 产物 | 工具链 |
|------|------|------|--------|
| Windows | x86 | `vpid_viewer_windows_x86.exe` | VS2022 BuildTools + MSVC，XP 兼容，1.5MB 级 |
| Linux | amd64 | `vpid_viewer_linux_amd64.tar.gz` | zig 交叉，glibc floor 2.17；含 UI 资源目录 |
| Linux | arm64 | `vpid_viewer_linux_arm64.tar.gz` | zig 交叉，glibc floor 2.17；含 UI 资源目录 |

> **Linux 产物是 tar.gz 包**：ELF 无 PE 资源段，运行时从 `<exe_dir>/uires/` 加载
> UI 资源（XML 布局 + 系统皮肤），因此打成单文件包发布。解压即用：
> ```bash
> tar xzf vpid_viewer_linux_amd64.tar.gz
> ./vpid_viewer_linux_amd64
> ```

常用变体：

```bash
python tools/build_release.py --only windows        # 只构建 Windows
python tools/build_release.py --only linux-x64      # 只构建 Linux amd64
python tools/build_release.py --only linux-arm64    # 只构建 Linux arm64
python tools/build_release.py --no-verify           # 跳过产物验证
python tools/build_release.py --dist out/           # 指定输出目录
```

构建完成后自动跑 `tools/verify_release.py` 校验每个产物：
- Windows：PE 有效、x86 架构
- Linux ELF：架构正确、glibc 版本符号 ≤ 2.17、动态依赖仅基础系统库
- Linux tar.gz 包：内含 ELF + `uires/` 资源目录（应用 uires.idx + 系统皮肤 + 布局 XML）

## 二、发布（GitHub Release API 直传）

```bash
python tools/release_github.py --tag v1.0.0 --title "v1.0.0" --notes-file RELEASE_NOTES.md
```

脚本会：
1. 推送当前分支到 origin
2. 打 annotated tag（`git tag -a`），先推 tag
3. 用 GitHub API 创建/更新 Release（凭据走 git-credential-manager）
4. 上传 `dist/` 下三个产物，校验大小

### 版本号约定（语义化版本）

- `vX.Y.Z`：X=大版本，Y=新功能，Z=修复。首个独立版本为 `v1.0.0`。
- 仓库历史 tag 为 `v22`~`v40`（GitHub Actions run_number 风格），已废弃。

### 发布前检查清单

1. `python tools/build_release.py` 全绿（无 FAIL）
2. `python tools/verify_release.py` 全部通过
3. `git status` 干净（或已提交）
4. 有 README 提到的运行依赖（Windows XP 需注意 /MT；Linux 需 USB 权限）

## 三、其他

- **Windows 构建细节**：MSVC `/Os /Gy /GR- /EHsc- /OPT:REF /GL` + `/MT` 静态运行时
  + `/SUBSYSTEM:WINDOWS,5.01`（XP 兼容），strip 后 ~1.5MB，360 实测放行（无 packer 特征）。
- **Linux glibc floor**：zig `-target x86_64-linux-gnu.2.17`，产物兼容
  CentOS 7 / RHEL 7 / Ubuntu 12.04+。系统库 libxcb/libXau/libuuid 从
  `tools/linux-sysroot/{lib,lib-arm64}/` 链接（已入库，离线可复现）。
- **构建目录**：`.build-release/`（脚本专用，git 忽略）。
- 相关文档：[XP_COMPATIBILITY.md](XP_COMPATIBILITY.md)（跨平台兼容性）、
  [README.md](README.md)（从源码构建）。