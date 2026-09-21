## 🦀 zqlcrab v0.1.1

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.1 Release Highlights:
- ⚡ **Zero-Copy Autocompletion Engine & Keystroke Optimization**:
  - Eliminated UI freezes during SQL editing by switching to zero-copy schema cache reads under read lock.
  - Implemented zero-heap-allocation case-insensitive token matching and a 64-item cap to maintain a snappy 120 FPS.
  - Prioritized completion ranking: Keywords > Tables & Views > Built-in Functions > Snippets > Global Columns.
- 🛡️ **Native macOS Status Bar Memory Leak Fix**:
  - Resolved Objective-C MRC retention leak in `NSMenuItem` and `NSString` references during status bar menu updates.
- 💡 **Instant Dot Syntax Column Autocomplete**:
  - Typing `.` after any table name (e.g. `SELECT users.`) immediately triggers the full list of columns for that table, even when no column prefix character has been entered yet.
  - Smart identifier parsing with automatic stripping of quotes (`` ` ``, `"`, `[]`).
  - Asynchronous background schema prefetching keeps completion snappy and responsive without blocking the UI.
- 🎯 **Run Selected SQL Query Range**:
  - When text is highlighted or selected in the SQL Query Console, clicking **Run** (or pressing `⌘↵` / `Ctrl+Enter`) executes only the selected SQL fragment.
  - Automatically falls back to full-editor execution when no text is selected.
  - Selected query support extends to visual **EXPLAIN** query plan analysis.
- 🏗️ **Cross-Platform Layered Architecture (Web / WASM + HTTP Gateway)**:
  - Clean separation between core database models, UI presentation, and network transport adapters.
  - Introduced `RemoteHttpAdapter` and gateway protocol specifications, enabling deployment as a Web/WASM frontend connected to a remote database gateway alongside native desktop direct socket connections.
  - Added comprehensive architectural blueprint in `docs/architecture-web-wasm.md`.
- 💻 **Zedis-Style System Tray Integration**:
  - Revamped tray menu with live connection indicators, latency metrics, and quick profile switching.

---

<details>
<summary><b>Prior Releases (v0.1.0 Initial Release)</b></summary>

- ⚡ **GPU Accelerated 120 FPS UI**: Native GPUI desktop application with instant startup and obsidian/light theme support.
- 🔌 **Multi-Engine Relational Database Connectivity**: Full async support for PostgreSQL, MySQL, and SQLite.
- 📊 **Interactive Data Grid**: Virtualized data grid with in-place cell editing, clone, insert, delete, and atomic SQL changeset transaction script review.
- 💻 **SQL Query Console**: Multi-tab query sessions, SQL formatter (`sqlformat`), and visual `EXPLAIN` query execution plan tree viewer.
- 🧭 **Visual Table & Index Designer**: Interactive schema creation with real-time dialect DDL preview, primary key, auto-increment, and index designer.
- 🖥️ **macOS Native Integration**: Native macOS menu bar, status bar item, Retina DMG installer, and cross-platform keyboard shortcuts.
- 📦 **Cross-Platform Release Artifacts**: Standalone packages for macOS (Apple Silicon & Intel DMG + tarball), Linux (x86_64 tarball), and Windows (x86_64 portable zip).

</details>
