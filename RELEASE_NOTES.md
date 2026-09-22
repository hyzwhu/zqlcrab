## 🦀 zqlcrab v0.1.2

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.2 Release Highlights:
- 🗑️ **DataGrid Row Delete Commits by Primary Key (数据浏览按主键删除行)**:
  - Clicking Delete Row opens the SQL review immediately instead of only marking the row in memory.
  - PostgreSQL column metadata now includes primary keys, so the generated `DELETE` targets the key instead of every column.
  - A `DELETE` or `UPDATE` that matches zero rows rolls the transaction back instead of reporting success.
- 📥 **Comprehensive Data Import Wizard for CSV, TSV & SQL (全面支持 CSV/TSV/SQL 数据导入向导)**:
  - Added native file picker (`rfd`) and file path input supporting `.csv`, `.tsv`, and `.sql` data formats.
  - Automatic CSV format sniffer with delimiter detection (comma, tab, semicolon, pipe), charset encoding detection (UTF-8, GBK/GB18030, UTF-16, Latin-1), and header row recognition.
  - Live data preview table with intelligent column-to-target field auto-mapping (exact, snake_case, camelCase, case-insensitive).
  - High-performance chunked batch insertion with real-time throughput metrics (rows/sec, progress percentage, elapsed time).
  - Configurable error policies: skip row, abort on error, or isolated row-by-row fallback insertion with downloadable error logs.
  - Accessible via Sidebar table context menu ("Import Data...") and DataGrid toolbar.
- 🚀 **Multi-Statement Batch Script Execution & Rich PG Diagnostics (多语句批量执行与PostgreSQL诊断信息)**:
  - Added robust SQL tokenizer `split_sql_statements` properly handling single quotes, double quotes, MySQL backticks, PostgreSQL dollar quotes (`$$...$$`), and nested comments.
  - Enabled sequential multi-statement execution across PostgreSQL, MySQL, and SQLite adapters without failing on semicolons or prepared statement limitations.
  - Implemented detailed PostgreSQL diagnostic error formatting (`format_pg_error`), unpacking server severity, error message, detail, hint, table, and column rather than obscure `db error`.
  - Added UTF-8 character boundary safe truncation (`truncate_sql_snippet`) across all adapters, completely resolving panics when executing SQL scripts containing Chinese comments and multi-byte characters.
- 🏷️ **Dialect-Aware Column Data Type Selection & Presets (新建表方言数据类型下拉选择与预设)**:
  - Added dialect-aware data type selectors in `CreateTableModal` with dedicated dropdown menus populated with standard types for PostgreSQL, MySQL, and SQLite while retaining direct text editability for custom types and length parameters.
  - Enhanced column header quick presets tailored to the active database family.
- 🛡️ **Modal Overlay Event Occlusion & Click Penetration Elimination (遮罩层全局事件隔离防穿透)**:
  - Added GPUI `.occlude()` to all modal backdrops throughout the application, ensuring upper-layer dialog interactions strictly block mouse events from penetrating down to the underlying DataGrid cells, toolbar action buttons, editor buffers, and workspace navigation tabs.
  - Comprehensive coverage across all dialog flows: New Connection Modal (`ConnectionDialog`), Connection Error Diagnostic Modal (`ConnectionErrorDialog`), Table Visual Designer (`CreateTableModal`), Destructive Actions Confirmation (`ConfirmDialog`), Atomic SQL Changeset Review (`SqlReviewModal`), and Full Cell Value Inspector modal (`DataGrid`).
- 💻 **Mac-Native Monochrome Silhouette Status Bar Tray (系统托盘剪影与高清比例重构)**:
  - Converted the macOS status bar tray icon into a template monochrome silhouette (`[image setTemplate: YES]`), automatically adapting to macOS light and dark menu bars with native system styling.
  - Tight bounding-box cropped icon assets (`tray-icon.png` and `tray-icon@2x.png`) with proportional dimensions (`22.0pt × 16.0pt`) for crisp rendering on Retina displays without edge clipping.
- 📐 **Pixel-Perfect Colon-Aligned Tray Metrics (托盘指标按冒号精准对齐)**:
  - Rebuilt the status bar menu metric rows (`Active:`, `DB:`, `Ping:`, `Memory:`) using native AppKit dual-column `NSView` items.
  - Right-aligned metric labels and left-aligned metric values perfectly aligned at the colon `:`, eliminating proportional font spacing misalignments.
  - Real-time memory footprint tracking via macOS Darwin kernel `TASK_VM_INFO` physical footprint API matching Activity Monitor memory metrics.
- 🔄 **Remote Gateway Protocol & Architectural Refinement**:
  - Bumped server protocol and mock client specifications to v0.1.2.
