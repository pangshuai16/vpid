# 自动发布指南

## GitHub Action 自动发布

本项目配置了 GitHub Action 用于自动编译和发布多平台可执行文件（C++ 原生实现）。

## 工作流配置

### build.yml (分支构建)
- **触发**: 所有分支推送（除 main 外）
- **构建平台**: Windows x86, Linux x64/arm64
- **目的**: 快速验证代码变更

### release.yml (发布构建)
- **触发**: main 分支推送
- **构建平台**: Windows x86, Linux x64/arm64
- **目的**: 创建完整 Release

## 使用步骤

### 自动发布（main 分支推送）

```bash
git add .
git commit -m "发布新版本"
git push origin main
```

推送后 GitHub Actions 将自动：
1. 构建全部平台矩阵
2. 创建新的 GitHub Release
3. 上传所有可执行文件

### 查看构建状态

- 打开 GitHub 仓库的 "Actions" 页面
- 查看对应工作流
- 等待构建完成

### 获取发布文件

构建完成后：
- 打开仓库的 "Releases" 页面
- 找到最新的版本发布（版本号为 `v${github.run_number}`）
- 下载对应平台的可执行文件

## 构建平台说明

| 平台 | 架构 | 文件名 | CI 环境 |
|------|------|--------|---------|
| Windows | x86 | `vpid_viewer_windows_x86.exe` | Windows 2022 + MSVC (VS2022) x86，/MT 静态链接，XP 兼容 |
| Linux | x64 | `vpid_viewer_linux_amd64` | ubuntu-22.04 + SOUI5/swinx（Cairo/xcb/libudev） |
| Linux | arm64 | `vpid_viewer_linux_arm64` | ubuntu-22.04 QEMU（`linux/arm64`）+ SOUI5/swinx |

## 相关文档

- 跨平台兼容性方案与 CI/CD 关键配置（Windows XP / Linux glibc 兼容、Release 配置要点）: [XP_COMPATIBILITY.md](XP_COMPATIBILITY.md)
- 本地构建测试与开发环境: 见 [README.md](README.md) 的「从源码构建」章节
- Linux 运行需要 USB 访问权限: 见 [README.md](README.md) 的「Linux 权限注意事项」章节