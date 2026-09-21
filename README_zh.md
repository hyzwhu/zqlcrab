<p align="center">
  <img src="assets/logo.png" width="160" height="160" alt="zqlcrab Logo" />
</p>

<h1 align="center">zqlcrab</h1>

<p align="center">
  <strong>基于 Rust & GPUI 构建的高性能、轻量级、GPU 加速的关系型数据库桌面客户端</strong>
</p>

<p align="center">
  <a href="https://github.com/hyzwhu/zqlcrab/releases"><img src="https://img.shields.io/github/v/release/hyzwhu/zqlcrab?include_prereleases&color=orange" alt="Release"></a>
  <a href="LICENSE-APACHE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/rust-2024%20edition-lightgrey.svg?logo=rust" alt="Rust 2024">
  <img src="https://img.shields.io/badge/UI-GPUI%20120%20FPS-cyan.svg" alt="GPUI">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg" alt="Platforms">
</p>

<p align="center">
  <a href="README.md">English</a> | <strong>简体中文</strong>
</p>

---

**zqlcrab** 是一款专为开发者、DBA 和数据工程师打造的现代关系型数据库桌面客户端，采用 Rust 语言与 [GPUI](https://github.com/zed-industries/zed) / [gpui-kit](https://github.com/longbridge/gpui-kit) 框架深度打造。告别 Electron 与 Webview 的高内存占用与输入延迟，畅享极速启动与零延迟的丝滑体验。

---

## ✨ 核心特性

- ⚡ **GPU 加速 120 FPS 原生 UI**：基于 Zed 的 GPUI 渲染框架，启动毫秒级响应，超低内存占用。
- 🔌 **多引擎关系型数据库支持**：
  - **SQLite**：内置原生支持 WAL 模式、内存数据库与本地文件库。
  - **PostgreSQL**：高性能异步驱动，支持 TLS/SSL 安全连接与架构模式（Schema）。
  - **MySQL**：全面支持 MySQL 5.7/8.x 与兼容引擎，包含安全连接与完备的类型映射。
- 📊 **高性能数据表格 (Data Grid)**：
  - 虚拟滚动轻松承载数十万行数据。
  - 支持原地单元格编辑，智能收集修改（Changeset），并可生成差异 SQL 执行事务保存。
  - 支持新增行、复制行、删除行等便捷操作。
- 💻 **现代化 SQL 查询控制台**：
  - 多标签页独立查询会话。
  - 集成一键 SQL 格式化与美化。
  - 支持可视化 `EXPLAIN` 执行计划分析。
  - 自动记录查询历史，支持关键字过滤、一键重载与重新执行。
- 🧭 **交互式表结构查看器 (Schema Designer)**：
  - 实时查看字段名、数据类型、主键、可空约束与默认值。
  - 直观呈现索引与外键定义。
- 🎨 **自适应主题与系统菜单**：
  - 深度优化的暗色（Dark）与亮色（Light）模式。
  - 完备的 macOS 原生应用菜单与状态栏托盘菜单（Status Item）。
  - 支持跨平台的完整快捷键映射。

---

## 🚀 快速上手与安装

### 预编译二进制下载

前往 [GitHub Releases](https://github.com/hyzwhu/zqlcrab/releases) 下载适合您操作系统的最新版本：

- **macOS**：提供 Apple Silicon (`aarch64`) 与 Intel (`x86_64`) 的 `.dmg` 安装包与 `.tar.gz` 压缩包。
- **Linux**：提供 `x86_64-unknown-linux-gnu` 的 `.tar.gz` 独立运行包。
- **Windows**：提供 `x86_64-pc-windows-msvc` 的 `.zip` 便携安装包。

---

### 从源码编译运行

#### 环境要求
- 安装 Rust 工具链（推荐 1.85+，支持 2024 edition）：
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

#### 1. 克隆代码仓库
```bash
git clone https://github.com/hyzwhu/zqlcrab.git
cd zqlcrab
```

#### 2. 系统依赖与安装配置

##### macOS
下载 Release 中的 `.dmg` 镜像打开后，将 `zqlcrab` 拖拽到 `Applications` 文件夹即可完成安装。

> **⚠️ macOS 首次打开提示“应用已损坏，无法打开”解决方式：**  
> 这是由于开源免签应用触发了 macOS Gatekeeper 隔离保护机制。可打开「终端」(Terminal) 执行以下命令解除隔离：
> ```bash
> sudo xattr -rd com.apple.quarantine /Applications/zqlcrab.app
> ```
> 或在挂载的 DMG 安装窗口中直接打开「复制修复命令.txt」文件复制命令。

源码编译：
```bash
cargo build --release
```

##### Linux (Ubuntu / Debian / Arch / Fedora)
安装 X11、Wayland 与字体开发库依赖：
```bash
# Ubuntu / Debian
sudo apt-get update && sudo apt-get install -y \
    pkg-config libssl-dev libx11-dev libxcb1-dev \
    libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libxkbcommon-dev libxkbcommon-x11-dev libasound2-dev \
    libfontconfig1-dev libfreetype6-dev libvulkan1

# Arch Linux
sudo pacman -S fontconfig freetype2 libx11 libxkbcommon wayland mesa
```

##### Windows
安装 [Visual Studio C++ 生成工具](https://visualstudio.microsoft.com/zh-hans/visual-cpp-build-tools/) 并勾选 Windows SDK 组件。

#### 3. 运行应用
```bash
cargo run --release
```

---

## ⌨️ 常用快捷键与菜单操作

`zqlcrab` 提供全套 macOS 原生菜单栏与系统托盘菜单，并支持高效快捷键操作：

### 导航与连接管理

| macOS 快捷键 | Windows / Linux 快捷键 | 功能描述 |
| :--- | :--- | :--- |
| `⇧ + ⌘ + N` | `Ctrl + Shift + N` | 打开新建连接对话框 |
| `⌘ + T` | `Ctrl + T` | 新建查询标签页 / 切换到控制台 |
| `⌘ + R` | `Ctrl + R` | 刷新数据库表与架构结构 |
| `⌘ + 1` | `Ctrl + 1` | 切换到 **SQL 控制台** 标签页 |
| `⌘ + 2` | `Ctrl + 2` | 切换到 **数据表格** 标签页 |
| `⌘ + 3` | `Ctrl + 3` | 切换到 **表结构** 标签页 |
| `⌘ + 4` | `Ctrl + 4` | 切换到 **查询历史** 标签页 |

### SQL 控制台与查询

| macOS 快捷键 | Windows / Linux 快捷键 | 功能描述 |
| :--- | :--- | :--- |
| `⌘ + ↵` | `Ctrl + Enter` | **执行当前 SQL 查询** |
| `⌥ + ⇧ + F` | `Alt + Shift + F` | **格式化 SQL 语句** |
| `⌘ + ⇧ + E` | `Ctrl + Shift + E` | **执行计划分析** (`EXPLAIN`) |

### 数据表格编辑

| macOS 快捷键 | Windows / Linux 快捷键 | 功能描述 |
| :--- | :--- | :--- |
| `⌘ + S` | `Ctrl + S` | **保存修改**（审查并提交变更） |
| `⌥ + N` | `Alt + N` | 添加新数据行 |
| `⌥ + D` | `Alt + D` | 复制选中行 |
| `⌥ + ⌫` | `Alt + Backspace` | 标记/删除选中行 |

### 窗口与视图控制

| macOS 快捷键 | Windows / Linux 快捷键 | 功能描述 |
| :--- | :--- | :--- |
| `⌘ + ,` | `Ctrl + ,` | 打开设置对话框 |
| `⌘ + B` | `Ctrl + B` | 显示 / 隐藏左侧活动导航栏 |
| `⌘ + J` | `Ctrl + J` | 显示 / 隐藏底部状态栏 |
| `⌃ + ⌘ + F` | `F11` | 切换全屏模式 |
| `⌘ + W` | `Ctrl + W` | 关闭当前窗口 / 标签页 |
| `⌘ + Q` | `Ctrl + Q` | **退出应用程序** |

---

## 🏗️ 架构概览

```text
zqlcrab/
├── Cargo.toml               # 项目配置与依赖清单
├── assets/                  # 官方图标、DMG 引导背景、Info.plist 等资源
├── src/
│   ├── main.rs              # 原生窗口生命周期、托盘与全局快捷键入口
│   ├── settings.rs          # 用户配置管理与持久化序列化
│   ├── db/                  # 统一数据库访问层抽象与适配器
│   │   ├── mod.rs           # 连接管理器、事务生命周期与驱动抽象
│   │   ├── explain.rs       # EXPLAIN 执行计划格式化引擎
│   │   ├── changeset.rs     # 数据表格变更跟踪与 SQL 差异生成
│   │   ├── sqlite.rs        # SQLite 驱动支持
│   │   ├── postgres.rs      # PostgreSQL 异步客户端适配器
│   │   ├── mysql.rs         # MySQL 异步客户端适配器
│   │   └── types.rs         # 统一数据类型与元数据定义
│   └── ui/                  # GPUI 视图与状态机
│       ├── app.rs           # 根应用程序视图、全局分发与模态框
│       ├── theme.rs         # 配色体系与主题定义
│       └── components/      # 数据表格、控制台、表结构设计器、连接管理
└── .github/workflows/       # 自动化 CI 与跨平台 Release 发布流程
```

---

## 🤝 参与贡献

欢迎提交 Issue 与 Pull Request！
1. Fork 本仓库
2. 新建功能分支 (`git checkout -b feature/amazing-feature`)
3. 确保测试通过 (`cargo test`)
4. 提交更改 (`git commit -m 'feat: add amazing feature'`)
5. 推送分支 (`git push origin feature/amazing-feature`)
6. 发起 Pull Request

---

## 📄 开源许可证

本项目基于 [Apache 2.0](LICENSE-APACHE) 许可证开源。
