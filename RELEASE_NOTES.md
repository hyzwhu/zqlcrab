## 🦀 zqlcrab v0.1.3

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.3 Release Highlights:
- 🚀 **Visual Data & Schema Export Wizard & Table Dump (数据导出向导与全量转储引擎)**:
  - **Multi-Format Serialization**: Seamlessly export tables and query slices into SQL Script (Full Dump / DDL / Inserts), CSV, TSV, JSON Array, NDJSON (stream), and GitHub Flavored Markdown (GFM).
  - **Comprehensive Dump Options**: Multi-dialect SQL dumps support `DROP TABLE IF EXISTS` (`CASCADE` for PG), dialect-aware transaction boundaries (`BEGIN TRANSACTION` / `START TRANSACTION`), configurable batch multi-row `INSERT` sizes, and engine identifier quoting.
  - **Granular Filter & Scope**: Choose between Structure & Data, Data Only, or Structure Only (DDL); apply arbitrary `WHERE` conditions and row limits directly in the export wizard.
  - **Live Dynamic Preview**: Instant 20-row preview panel renders real-time serialization results with truncation indicators without memory overhead.
  - **Flexible Destinations**: Export directly to local disk with timestamped filenames (`rfd::FileDialog` / native file manager reveal) or copy formatted content straight to the system clipboard.
  - **Seamless Context Navigation**: Launch the wizard directly via the table right-click context menu in the sidebar (`Export Data / Dump...`).
- 🛠️ **Visual Schema Evolution & Online ALTER TABLE (表结构在线可视化重构与 DDL 迁移)**:
  - **Interactive Structure Editor**: Toggle inline structure editing directly inside `SchemaViewer` with live status indicators (`+ ADD`, `DROP`, `KEEP`), field renaming, data type adjustments, primary key / auto-increment toggling, nullability, default values, and comments.
  - **Multi-Dialect DDL Diff Engine**: Built-in `generate_alter_table_plan` intelligently analyzes baseline versus edited schema to synthesize atomic migration scripts across MySQL (`ADD/MODIFY/CHANGE/DROP`), PostgreSQL (`ADD/TYPE/SET NOT NULL/COMMENT`), and SQLite (native `ADD COLUMN` or safe 12-step table recreation migration).
  - **Atomic Review & Execution**: Review generated DDL, column-level operation counts, and safety warnings through `SqlReviewModal` before applying changes atomically with automatic schema cache reloading.
- 🔄 **Bidirectional DDL Editing & Schema Synchronization (建表 DDL 双向编辑与语法自动同步)**:
  - **Editable DDL Editor**: In the Create Table dialog, SQL is directly editable and accepts arbitrary pasted `CREATE TABLE` statements across SQLite, PostgreSQL, and MySQL dialects.
  - **Intelligent Reverse Parser**: Built-in `parse_create_table_sql` engine automatically parses table names, column types (including parametrized types like `DECIMAL(10, 2)`), primary keys, auto-increment, constraints, default values, comments, and indexes, populating the interactive visual designer forms in real time.
  - **Real-time Bidirectional Sync**: Modifying columns or indexes immediately regenerates pristine DDL; editing custom DDL directly executes without loss of custom definitions.
- ⚡ **Table Lifecycle Operations in Sidebar (数据表生命周期右键快捷操作)**:
  - **Show CREATE TABLE (DDL)**: View and extract native DDL statements directly into a dedicated query tab (`DDL: <table_name>`).
  - **Copy INSERT Template**: Instantly assemble a schema-aware `INSERT INTO` placeholder template omitting auto-increment columns and copy to clipboard.
- 📊 **Accurate Affected-Row Counts (影响行数按实际变更显示)**:
  - Saving a grid change reloads the table with `SELECT` instead of running the editor's `DELETE` or `UPDATE` a second time.
  - `BEGIN`, `COMMIT`, and `ROLLBACK` are left out of the affected-row total, so a delete script reports the rows the `DELETE` changed.
  - A `SELECT` that returns no rows keeps its columns instead of showing "0 row(s) affected".
