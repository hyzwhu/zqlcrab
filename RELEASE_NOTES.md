## 🦀 zqlcrab v0.1.1

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.1 Release Highlights:
- 📑 **Multi-Tab Query Sessions (多 SQL 查询会话标签页)**:
  - Complete multi-tab SQL workspace architecture: each query tab encapsulates an independent SQL buffer, execution results, error diagnostic banner, execution metrics, and visual EXPLAIN query plan.
  - Asynchronous background query execution isolation: running long or intensive queries tracks execution by session UUID, routing results safely to the originating tab even if the user switches active tabs during query execution.
  - Native tab management: interactive Tab Strip with execution spinner indicators, error alerts, close `[×]` buttons, and quick new tab `[+]` button.
  - Keyboard shortcuts: `⌘T` / `Ctrl+T` to instantly create new query tabs, and `⌘W` / `Ctrl+W` to close active tabs with adjacent tab auto-focusing and safe reset protection.
  - Seamless sidebar integration: right-clicking any table and selecting **Query in Console** opens a dedicated `{table}.sql` tab without overwriting active work in other query tabs.
  - Live editor settings synchronization across all open tabs simultaneously.
- 🔄 **Real-Time GitHub Releases Update Checker**:
  - Connected the **Check for Updates** button under **Settings -> Appearance** to live GitHub Releases REST API (`/repos/hyzwhu/zqlcrab/releases/latest`).
  - Added robust semantic version comparison logic with prerelease handling.
  - Interactive status badges: displays **NEW v{version}** badge with direct **View Release** navigation when updates are available, or a verified up-to-date timestamp.
- 🛠️ **Live SQL Editor Customization & Real-Time Settings Synchronization**:
  - Full real-time synchronization for all configuration options under **Settings -> Editor**: Font Family, Font Size, Tab Size (indentation width), Line Numbers gutter, Word Wrapping, Format on Run, and Bracket / Quote Auto-closing.
  - Custom font family and font size apply directly to the query console editor without requiring application restarts or tab reloads.
- 🚨 **Connection Error Diagnostic Dialog & Overflow Protection**:
  - Clear modal diagnostic error alert whenever a database connection fails, equipped with **Retry**, **Edit Connection** (pre-fills profile settings for instant correction), and **Copy Error**.
  - Bounded layout constraints and dynamic long-token wrapping (`wrap_error_text`) preventing unbroken error messages or URLs from overflowing modal boundaries across all dialogs.
- 🧹 **Comprehensive Memory Optimization & Leak Fixes**:
  - Integrated `mimalloc` global allocator for efficient query allocation recycling and lower RSS footprint under heavy query loads.
  - Aligned tray memory metrics with macOS Activity Monitor (`PhysFootprint`).
  - Resolved Objective-C MRC retention leaks in `NSMenuItem` and `NSString` references during macOS status bar menu updates (including Quick Connect submenus).
  - Gracefully teardown and disconnect previous database connection pools upon profile switching.
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
