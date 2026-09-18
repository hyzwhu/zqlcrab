# CrabStudio (zqlcrab)

A high-performance, cross-platform relational database desktop client built in Rust using GPUI and [gpui-kit](https://github.com/longbridge/gpui-kit). Designed for software engineers, database administrators, and data practitioners who value speed, responsiveness, and a polished visual experience.

---

## Highlights & Features

- ⚡ **Blazing Fast GPU-Accelerated UI**: Powered by GPUI and `gpui-kit`, delivering instant 120 FPS rendering, zero webview bloat, and minimal memory footprint.
- 🌐 **True Cross-Platform Support**: Seamlessly runs on **macOS**, **Linux** (X11 & Wayland), and **Windows**.
- 🔌 **Multi-Engine Relational Database Connectivity**:
  - **SQLite** (bundled with WAL mode, in-memory databases, and custom file paths)
  - **PostgreSQL** (high-performance asynchronous driver with SSL and pool support)
  - **MySQL** (asynchronous connection pooling and schema reflection)
- 🗄️ **Persistent Connection Profiles**:
  - Save and organize connection configurations securely on local disk (`~/.config/zqlcrab/connections.json` or platform equivalent).
  - One-click profile switching, live connection testing, latency ping tracking, and graceful disconnection.
- 💻 **Interactive SQL Console**:
  - Multi-line editor with keyboard execution shortcuts (`⌘↵` / `Ctrl+Enter`).
  - Execution duration timer and returned row counter.
  - Formatted query error reporting and diagnostics.
- 📊 **Tabular Data Grid**:
  - Inspect query results and table contents with column type headers, row indices, and numeric/text formatting.
  - Clean empty states and scrollable data regions.
- 🔍 **Database Schema & DDL Inspector**:
  - Column specifications (data types, primary keys, auto-increment badges, nullability, default values).
  - Index definitions (index name, unique flags, indexed column names).
  - Generated native `CREATE TABLE` DDL statement viewer with syntax contrast.
- 🎨 **Design System Driven Aesthetics**:
  - Implements the complete design specifications defined in `DESIGN.md`.
  - Obsidian & Slate dark theme palette (`#0F172A`, `#1E293B`, `#334155`) with Electric Ocean interactive accents (`#0EA5E9`, `#38BDF8`).
  - Strict typography scale, 8px spacing grid, and subtle elevation tokens.

---

## Architectural Overview

CrabStudio separates low-level asynchronous database drivers from GPUI's retained presentation layer:

```
┌─────────────────────────────────────────────────────────────┐
│                 GPUI Presentation Layer                     │
│  (CrabStudioApp, Sidebar, QueryConsole, DataGrid, Schema)   │
└──────────────────────────────┬──────────────────────────────┘
                               │ cx.spawn (Async tasks)
┌──────────────────────────────▼──────────────────────────────┐
│                    ActiveConnection Handle                  │
│       (Thread-safe Arc<Mutex<Box<dyn DatabaseAdapter>>>)    │
└──────────────────────────────┬──────────────────────────────┘
                               │ Asynchronous operations
        ┌──────────────────────┼──────────────────────┐
        ▼                      ▼                      ▼
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│ SqliteAdapter │      │PostgresAdapter│      │ MysqlAdapter  │
│  (rusqlite)   │      │(tokio-postgres│      │ (mysql_async) │
└───────────────┘      └───────────────┘      └───────────────┘
```

- **`DatabaseAdapter` Trait**: Unified abstraction for connection lifecycle, query execution, metadata discovery, and DDL generation.
- **`ActiveConnection`**: Thread-safe active handle holding connection metadata and server ping health.
- **`ConnectionManager`**: Encapsulates disk serialization and active session registry.
- **`CrabStudioApp`**: Root GPUI entity managing window state, asynchronous command execution, workspace tab switching, and dialog overlays.

---

## Installation & Building

### Prerequisites

Ensure you have Rust (edition 2024 / 1.85+) and Cargo installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Platform-Specific Setup

#### macOS
No additional system dependencies are required. Build directly:
```bash
cargo build --release
```

#### Linux (Ubuntu / Debian / Fedora / Arch)
Install standard GPUI and graphics build dependencies:

```bash
# Ubuntu / Debian
sudo apt-get update && sudo apt-get install -y \
    build-essential \
    libfontconfig1-dev \
    libfreetype6-dev \
    libx11-dev \
    libx11-xcb-dev \
    libxkbcommon-x11-dev \
    libwayland-dev \
    libgl1-mesa-dev \
    pkg-config

# Fedora
sudo dnf install -y \
    fontconfig-devel \
    freetype-devel \
    libX11-devel \
    libxkbcommon-x11-devel \
    wayland-devel \
    mesa-libGL-devel

# Arch Linux
sudo pacman -S fontconfig freetype2 libx11 libxkbcommon wayland mesa
```

#### Windows
Install Visual Studio C++ Build Tools and Windows SDK.

---

## Running the Application

### Development Mode

```bash
cargo run
```

When started for the first time, CrabStudio automatically boots with an in-memory SQLite demo database pre-populated with sample tables (`users`, `products`) so you can explore the interface immediately.

### Running Automated Tests

```bash
cargo test
```

---

## Design System

All user interface styling, layout geometries, colors, and component contracts are defined in [`DESIGN.md`](./DESIGN.md) and validated via `designmd`:

```bash
designmd lint DESIGN.md
```

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `⌘ + ↵` (Mac) / `Ctrl + Enter` (Win/Linux) | Run SQL Query in Console |
| `Esc` | Close New Connection Dialog |

---

## License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
