# 部署文档 / Deployment Guide

本文档详细说明了如何在本地环境中部署和运行 SRT Editor 项目。

## 系统要求

- macOS 10.15+ / Windows 10+ / Linux
- Node.js 18+
- Rust 1.70+
- CMake 3.15+（用于编译 Whisper.cpp）

## 前置依赖安装

### 1. 安装 PNPM

PNPM 是本项目使用的包管理器。

**macOS (使用 Homebrew):**
```bash
brew install pnpm
```

**使用 npm:**
```bash
sudo npm install -g pnpm
```

**使用 Corepack (Node.js 16.13+):**
```bash
sudo corepack enable
```

验证安装：
```bash
pnpm --version
```

### 2. 安装 Rust

Tauri 需要 Rust 来构建应用程序的后端。

**所有平台 (使用 rustup):**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

安装完成后，重新加载环境变量：
```bash
source $HOME/.cargo/env
```

验证安装：
```bash
rustc --version
cargo --version
```

### 3. 安装 CMake

**重要：** 本项目使用 `whisper-rs` 进行语音识别，该库依赖 Whisper.cpp，需要 CMake 来编译 C++ 代码。

**macOS (使用 Homebrew):**
```bash
brew install cmake
```

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install cmake build-essential
```

**Windows:**
从 [CMake 官网](https://cmake.org/download/) 下载安装程序。

验证安装：
```bash
cmake --version
```

### 4. 安装 Tauri 系统依赖

请参考 [Tauri 官方文档](https://tauri.app/start/prerequisites/) 安装平台特定的依赖。

**macOS:**
```bash
xcode-select --install
```

**Ubuntu/Debian:**
```bash
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

## 项目安装

### 1. 克隆项目

```bash
git clone <repository-url>
cd srt-editor
```

### 2. 安装依赖

如果遇到 electron 下载失败的问题，使用以下命令跳过：

```bash
ELECTRON_SKIP_BINARY_DOWNLOAD=1 pnpm install
```

或者：

```bash
pnpm install --no-optional
```

正常安装：
```bash
pnpm install
```

## 运行项目

### 开发模式

启动开发服务器（包含热重载）：

```bash
pnpm tauri dev
```

或者使用项目配置的开发脚本：

```bash
pnpm dev
```

### 构建生产版本

构建应用程序：

```bash
pnpm tauri build
```

构建完成后，可执行文件将位于 `src-tauri/target/release/` 目录中。

## 常见问题

### 1. electron 下载失败

**问题：** 安装依赖时 electron 下载超时或失败。

**解决方案：** 本项目使用 Tauri 而非 Electron，可以跳过 electron 的安装：

```bash
ELECTRON_SKIP_BINARY_DOWNLOAD=1 pnpm install
```

### 2. CMake 未找到

**问题：** 运行 `pnpm tauri dev` 时出现 `is 'cmake' not installed?` 错误。

**原因：** Whisper.cpp 需要 CMake 来编译 C++ 代码。

**解决方案：** 安装 CMake：

```bash
# macOS
brew install cmake

# Ubuntu/Debian
sudo apt install cmake
```

### 3. Rust 未找到

**问题：** 运行时出现 `cargo: command not found` 或 `rustc: command not found`。

**解决方案：**
1. 确保已安装 Rust
2. 重新加载环境变量：
```bash
source $HOME/.cargo/env
```
3. 或重启终端

### 4. 权限错误

**问题：** npm/corepack 安装时出现 EACCES 权限错误。

**解决方案：**
- 使用 `sudo` 命令
- 或使用 Homebrew 等包管理器安装

## 测试

运行单元测试：

```bash
pnpm test
```

检查 Rust 代码：

```bash
pnpm check
```

## 版本更新

1. 更新版本号：
```bash
pnpm bump [x.y.z]
```

2. 更新 Cargo.lock：
```bash
pnpm check
```

3. 提交并推送代码

## 技术栈

- **前端：** Vue 3 + TypeScript + Vite
- **后端：** Rust + Tauri
- **UI 框架：** Element Plus
- **语音识别：** Whisper.cpp (通过 whisper-rs)
- **音频处理：** Symphonia + Howler.js
- **波形显示：** WaveSurfer.js

## 目录结构

```
srt-editor/
├── src/                 # Vue 前端源代码
├── src-tauri/          # Tauri Rust 后端代码
├── public/             # 静态资源
├── tests/              # 测试文件
├── package.json        # Node.js 依赖配置
└── pnpm-lock.yaml      # PNPM 锁文件
```

## 更多信息

- [Tauri 官方文档](https://tauri.app/)
- [Vue 3 文档](https://vuejs.org/)
- [Whisper.cpp 项目](https://github.com/ggerganov/whisper.cpp)

## 获取帮助

如果遇到问题，请：
1. 检查本文档的常见问题部分
2. 查看项目的 GitHub Issues
3. 参考 Tauri 官方文档的故障排除指南
