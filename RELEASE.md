# 发布指南（本地脚本，不依赖 GitHub Actions）

v2.0.0 起为**纯 Rust 主线**：发布走本地固化脚本（构建 + 硬化 + 审计），
验证通过后手动发布到 GitHub Release（API 直传）。

## 一、构建（Windows 主产物）

```bash
python rust/scripts/build_release.py
```

产物输出到 `dist/`：

| 平台 | 架构 | 产物 | 体积 | 工具链 |
|------|------|------|------|--------|
| Windows | x64 | `vpid_viewer_windows_x64.exe` | ~0.26 MB | Rust MSVC 1.85+ |

常用变体：

```bash
python rust/scripts/build_release.py --no-build   # 仅复制 + 硬化 + 验证（用已有产物）
python rust/scripts/build_release.py --out X      # 指定输出路径
```

构建完成后自动跑 `rust/scripts/verify_release.py` 8 项审计：
导出表空（杀软敏感）/ `.rsrc` 段 / 清单（asInvoker/comctl v6/PerMonitorV2）/
版本资源（VERSIONINFO 含 BuildDate）/ 图标（RT_ICON+RT_GROUP_ICON）/
PE 校验和非零（`editbin /RELEASE`）/ 子系统版本 ≥ 6.0。

## 二、发布（GitHub Release API 直传）

```bash
python rust/scripts/release_github.py \
  --tag v2.0.0 \
  --title "v2.0.0" \
  --notes-file RELEASE_NOTES_v2.0.0.md
```

脚本会：
1. 推送当前分支到 origin
2. 打 annotated tag（`git tag -a`），先推 tag
3. 用 GitHub API 创建/更新 Release（凭据走 git-credential-manager）
4. 上传 `dist/` 下产物（当前仅 `vpid_viewer_windows_x64.exe`；
   Linux 产物待 GTK3 后端落地后追加）

Release 说明按版本归档为 `RELEASE_NOTES_vX.Y.Z.md`。

### 版本号约定（语义化版本）

- `vX.Y.Z`：X=大版本，Y=新功能，Z=修复。
- v2.0.0 = 从 C++/SOUI5 切换到纯 Rust + Win32 原生 UI 的**主版本跃升**。
- 历史 tag `v1.2.0` 属旧 Python/Fyne 版（已跳号）；`v22`~`v40` 为更早
  GitHub Actions `run_number` 风格 tag，均已废弃。

### 发布前检查清单

1. `python rust/scripts/build_release.py` 全绿（审计 8/8 PASS）
2. `cargo test` 全过（行为等价性）
3. `git status` 干净（或已提交）
4. Windows 目标机：Win7+（Rust MSVC 下限；XP 由旧 C++ v1.x 承担）

## 三、构建细节

- **纯 Rust**：`cargo build --release`，workspace 3 crate；
  release profile `opt-level=s` + `lto=true` + `codegen-units=1` + `panic="abort"` + `strip=true`。
- **发布硬化**（编译期 `build.rs` + winres）：VERSIONINFO（FileVersion/ProductVersion 取
  `CARGO_PKG_VERSION`，含 BuildDate）、清单（asInvoker + comctl32 v6 + PerMonitorV2 DPI +
  supportedOS）、主线图标；`editbin /RELEASE` 写 PE 校验和。
- **UPX 禁用**：任何 UPX 压缩级别在 360 杀软下实测被查杀（沿用旧版结论）。
- **产物目录**：`rust/target/`（构建）+ `dist/`（发布命名，git 忽略）。

相关文档：[README.md](README.md)（从源码构建）、
[rust/docs/PLATFORM_EXTENSION.md](rust/docs/PLATFORM_EXTENSION.md)（多平台扩展）。
