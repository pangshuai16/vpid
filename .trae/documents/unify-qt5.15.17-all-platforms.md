# 统一使用 Qt 5.15.17 全平台重构计划

## 摘要
将项目所有平台（Linux x86_64/arm64、Windows x86/x64、macOS x86_64/arm64）的 Qt 依赖统一为社区构建的 Qt 5.15.17，替换当前 CI 中混杂的 Qt 5.15.2 和 Qt 5.15.17 方案。

## 当前状态分析

### 已完成的工作
- QML 文件已从 Qt6 语法降级到 Qt 5.15 兼容语法
- Cargo.toml 已更新为 qmetaobject 0.2.10
- Rust 源代码已适配 Qt5

### 存在的问题
1. **Qt 版本不一致**：Linux/macOS CI 使用 Qt 5.15.2（install-qt-action），Windows 使用 Qt 5.15.17
2. **Windows 构建架构不匹配**：CI 下载 x86_64 的 Qt 但构建 i686（x86）目标
3. **缺少 arm64 Linux 的 Qt 安装步骤**：当前 Linux ARM64 交叉编译 job 没有安装 Qt
4. **macOS 使用 install-qt-action 的 5.15.2**：需要替换为统一的 5.15.17

### 可用资源
- Qt 5.15.17 社区构建仓库：https://github.com/cxxzhang/qt5.15.17/releases/tag/qt5.15.17-21280744304
- 各平台资源文件：
  - Linux x64: `qt5.15.17_linux_x64.tar.xz`
  - Linux arm64: `qt5.15.17_linux_arm64.tar.xz`
  - Windows x86: `qt5.15.17_windows_x86.zip`
  - Windows x64: `qt5.15.17_windows_x64.zip`
  - macOS x64: `qt5.15.17_macos_x64.zip`
  - macOS arm64: `qt5.15.17_macos_arm64.zip`
  - macOS universal: `qt5.15.17_macos_universal.zip`

## 提议的变更

### 1. GitHub Actions Build 工作流 (.github/workflows/build.yml)

#### Linux x86_64 Job
- **移除**：`jurplel/install-qt-action@v4` 步骤
- **新增**：下载并解压 `qt5.15.17_linux_x64.tar.xz` 到 `~/Qt/5.15.17`
- **设置环境变量**：`QTDIR=~/Qt/5.15.17`, `PATH=$QTDIR/bin:$PATH`
- **修改打包脚本**：从新的 Qt 路径复制库文件

#### Linux ARM64 Job
- **新增**：下载并解压 `qt5.15.17_linux_arm64.tar.xz`
- **新增**：设置 Qt 环境变量
- **注意**：交叉编译时可能需要设置 `QT_INCLUDE_PATH` 和 `QT_LIBRARY_PATH` 让 qmetaobject 找到 Qt

#### Windows x86 Job
- **修改**：下载 `qt5.15.17_windows_x86.zip`（而非之前的 x86_64 MinGW 版本）
- **修改**：解压后设置 `Qt5_DIR` 指向正确的 cmake 目录
- **移除**：`rust-lld` 链接器配置（使用 MSVC 默认链接器）
- **修改**：构建目标改为 `x86_64-pc-windows-msvc` 或保持 `i686-pc-windows-msvc` 但确保 Qt 架构匹配

#### macOS Jobs
- **移除**：`jurplel/install-qt-action@v4` 步骤
- **新增**：下载对应架构的 Qt 5.15.17 zip（x86_64 用 `macos_x64.zip`，arm64 用 `macos_arm64.zip`）
- **新增**：解压到 `~/Qt/5.15.17`
- **设置环境变量**：`QTDIR` 和 `PATH`

#### Lint Job
- **移除**：`jurplel/install-qt-action@v4`
- **新增**：下载 Linux x64 Qt 5.15.17

### 2. GitHub Actions Release 工作流 (.github/workflows/release.yml)
- 同步上述所有 build.yml 的变更

### 3. Cargo 配置 (.cargo/config.toml)
- **移除**或**注释掉** Windows i686 的 `rust-lld` 链接器配置（因为 Qt 是 MSVC 构建的）
- 确认 Linux 的 `lld` 链接器配置与 Qt 5.15.17 兼容

### 4. README.md 更新
- 更新技术栈描述：统一为 Qt 5.15.17
- 更新构建说明

## 假设与决策

1. **Qt 构建类型假设**：假设 cxxzhang 提供的 Qt 5.15.17 是动态库构建（非静态），因为文件大小约 380MB，符合动态库特征。

2. **Windows 架构决策**：
   - 选项 A：构建 x86 (i686) 应用，使用 `qt5.15.17_windows_x86.zip`
   - 选项 B：构建 x64 应用，使用 `qt5.15.17_windows_x64.zip`，放弃 XP 兼容但简化构建
   - **建议**：选项 A，保持 XP 兼容性，但确保 Qt 架构与应用架构一致

3. **macOS 架构决策**：
   - 分别下载 x64 和 arm64 的 Qt 包，而非 universal（节省下载时间）

4. **Linux ARM64 Qt 路径**：交叉编译时，qmetaobject 的 build.rs 需要找到 Qt 的头文件和库。可能需要：
   - 设置 `QT_INCLUDE_PATH` 和 `QT_LIBRARY_PATH` 环境变量
   - 或修改 `.cargo/config.toml` 传递这些变量

## 验证步骤

1. 每个 CI job 都添加 `qmake --version` 步骤验证 Qt 版本
2. 构建成功后检查链接的 Qt 库版本
3. Linux x86_64 本地测试运行
4. 检查所有 job 的 artifact 是否正确打包 Qt 依赖

## 实施顺序

1. 更新 `.github/workflows/build.yml` — 所有 platform jobs
2. 更新 `.github/workflows/release.yml` — 同步变更
3. 更新 `.cargo/config.toml` — 移除不兼容的链接器配置
4. 更新 `README.md`
5. 验证编译
