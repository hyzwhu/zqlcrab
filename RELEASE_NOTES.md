## 🦀 zqlcrab v0.1.7

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.7 Release Highlights:
- 📦 **Database Dump & Multi-Table SQL Backup Wizard (整库与多表 SQL 转储备份向导)**:
  - **Multi-Dialect Dump Engine**: Generates self-contained, standard `.sql` backup dumps for SQLite, PostgreSQL, and MySQL.
  - **Flexible Scope Selection**: Support for Full Database / Multi-Table dumps with granular scope control (`Structure & Data`, `Structure Only / DDL`, `Data Only / DML`).
  - **Dialect-Aware Foreign Key Protections & Atomic Transactions**:
    - Automatically controls foreign key constraint checks across engines (`PRAGMA foreign_keys = OFF/ON` for SQLite, `SET FOREIGN_KEY_CHECKS = 0/1` for MySQL, `SET CONSTRAINTS ALL DEFERRED` for PostgreSQL).
    - Configurable transactional wrapping (`BEGIN ... COMMIT`) and optional `DROP TABLE IF EXISTS` generation.
  - **Chunked & Batched DML Inserts**: Configurable batch insertion sizes (100, 200, 500, 1000 rows per `INSERT INTO`) to prevent query memory overflow and ensure maximum ingestion throughput.
  - **Versatile Output Destinations**:
    - Direct export to local `.sql` files via native system file dialogs (`rfd`).
    - One-click copy to system clipboard with visual toast indicator.
    - Direct inspection and execution in a dedicated Query Console editor tab (`dump_backup.sql`).
  - **Pervasive Entry Points**: Accessible from Database Explorer header actions, database item toolbar (`Download` icon), and table context menu ("Dump Table (SQL)..." / "转储表结构与数据 (SQL)...").

- 📥 **SQL Script Restore & Batch Execution Wizard (SQL 脚本恢复与批量执行器)**:
  - **Intelligent SQL Script Tokenizer & Statement Splitter**: Robust multi-line parser that accurately separates batch SQL statements while ignoring semicolons inside string literals (`'`, `"`, ``` ` ```), escaped quotes (`\'`, `''`), line comments (`--`, `#`), and block comments (`/* ... */`).
  - **Static Script Inspection & Metrics**: Instant pre-execution analysis displaying statement counts, DDL vs DML breakdown, file size, and statement previews.
  - **Configurable Fault-Tolerance Policies**:
    - **Stop on First Error**: Halts execution immediately upon encountering an error to protect data integrity.
    - **Continue & Log Errors**: Skips failing statements and proceeds with the rest of the batch for maximum recovery.
  - **Live Progress & Real-Time Audit Log**:
    - Real-time progress bar with completed percentage, statement counter, and live SQL statement preview.
    - Live execution audit log with duration benchmarks and formatted error details.
  - **Integrated Trigger**: Accessible directly from Database Explorer header actions (`Upload` icon / "Restore from SQL...").

- 🔄 **Visual Table Data Diff & Bidirectional Synchronization Wizard (可视化数据对比与双向同步向导)**:
  - **Key-Indexed Row & Cell Discrepancy Matrix**: Side-by-side reconciliation of records between two tables, categorizing discrepancies into `+ ADD`, `- DROP`, `~ MODIFY`, and `= SAME`.
  - **Deep Cell-Level Value Inspector**: Inspects field-by-field differences across all SQL data types (Strings, Integers, Floats, Booleans, Nulls, Timestamps) with visual `source ➔ target` transition pills.
  - **Multi-Dialect DML Synchronization Engine**:
    - Generates dialect-accurate `INSERT INTO`, `UPDATE ... SET ... WHERE`, and `DELETE FROM` statements for PostgreSQL, MySQL, and SQLite.
    - Type-safe identifier escaping and literal serialization handling `NULL`, escaped quotes, and byte arrays.
  - **Bidirectional Synchronization & Table Swapping**: Easily switch direction (`Source ➔ Target` vs `Target ➔ Source`) and swap tables (`⇄`) to sync between live, backup, staging, or development tables.
  - **Safe Mode & Transaction Controls**:
    - Non-destructive Safe Mode comments out `DELETE` operations by default (`-- [SAFE MODE SKIPPED]`) to prevent accidental data loss.
    - Transactional wrapping (`BEGIN; ... COMMIT;`) ensures all generated DML executes atomically.
  - **Multi-Tab Filtering & Status Analytics**: Filter row discrepancies instantly by `Differences Only`, `All Rows`, `Added Only`, `Modified Only`, and `Deleted Only`. Real-time metrics breakdown showing total added, modified, deleted, and identical counts.
  - **Console Integration & One-Click Copy**: Copy generated DML scripts to clipboard or open directly in a new Query Console editor tab (`data_sync.sql`) for testing and immediate execution.
  - **Pervasive Explorer & Toolbar Access**: Accessible directly from Table Data Grid toolbar (`Diff`), Table Structure Schema Viewer (`Data Diff`), and Database Explorer Sidebar context menu ("Compare Data..." / "数据对比与同步向导...").
  - **Full Bilingual Localization**: Complete English and Simplified Chinese (简体中文) support across all modal UI elements, badges, tooltips, and menu actions.

- 📊 **Visual ER Diagram & Table Relationship Graph (可视化实体关系 ER 拓扑图谱)**:
  - **Multi-Engine Foreign Key & Relationship Discovery**:
    - **PostgreSQL**: Introspects `information_schema.table_constraints`, `key_column_usage`, and `referential_constraints` for both single and composite foreign key constraints.
    - **MySQL**: Automatically queries schema-wide `KEY_COLUMN_USAGE` and `REFERENTIAL_CONSTRAINTS`.
    - **SQLite**: Dynamic table schema introspection via `PRAGMA foreign_key_list`, grouping composite foreign key columns and target references.
  - **Topology Layout & 2D Canvas Engine (`ErDiagramGraph`)**:
    - Automatic 2D arrangement with connection-degree hub positioning, legible multi-column alignment, and collision-free spatial bounds calculation.
    - Graph cardinality detection: Automatically identifies `1:N`, `1:1`, and `N:1` relationships based on uniqueness and primary key constraints.
  - **Interactive Table Cards (`ErDiagramView`)**:
    - Displays schema definitions, row count estimates, table types, and scrollable column lists.
    - Highlighted key badges: Gold `PK` for Primary Keys, Cyan `FK` for Foreign Key links, and detailed data type tags.
    - Direct actions: Jump directly from any table card to `DataGrid` (View Data) or `SchemaViewer` (Inspect Schema).
    - Table focus and relationship dimming: Click any table to focus its direct relations while dimming unrelated tables.
  - **Mermaid Markdown ER Export**:
    - One-click copy of the active ER diagram in standard Mermaid markdown syntax (`erDiagram ...`) for documentation, PR descriptions, and team architecture wikis.
  - **Search & Quick Zoom Controls**:
    - Live table filter input to search and isolate specific tables on large canvases.
    - Zoom toolbar: Zoom In, Zoom Out (40%–200%), and 100% Reset controls.
  - **Seamless Workspace Integration & Bilingual Localization**:
    - First-class workspace tab (`ER Diagram` / `ER 关系图`) accessible from titlebar tab strip, sidebar context menu ("View in ER Diagram" / "在 ER 图谱中查看"), and auto-refresh on schema updates.
    - Complete English and Simplified Chinese (简体中文) localization support.

- 🔄 **Visual Schema Diff & Multi-Dialect Migration Script Generator (可视化表结构对比与差异迁移脚本生成器)**:
  - **Side-by-Side Schema Discrepancy Matrix**: Interactive visual comparison of table definitions (columns, data types, nullability, default values, primary keys, and indexes) with color-coded status badges: `+ ADD`, `- DROP`, `~ MODIFY`, and `= IDENTICAL`.
  - **Multi-Dialect DDL Migration Engine**:
    - **PostgreSQL**: Incremental `ADD COLUMN`, `ALTER COLUMN TYPE`, `ALTER COLUMN SET/DROP NOT NULL`, `ALTER COLUMN SET/DROP DEFAULT`, `DROP COLUMN`, and `CREATE/DROP INDEX` statements.
    - **MySQL**: Dialect-accurate `ADD COLUMN`, `MODIFY COLUMN`, `DROP COLUMN`, `CREATE INDEX`, and `DROP INDEX` generation.
    - **SQLite**: In-place additions and drops with automated safe 12-step table-rebuild templates and cautionary warnings for complex constraint modifications unsupported in-place.
  - **Bidirectional Synchronization**: Instant direction switching (`Source ➔ Target` vs `Target ➔ Source`) or one-click table swapping (`⇄`) to easily sync development, staging, and production environments.
  - **Safety Protections**:
    - **Safe Mode**: Omits destructive `DROP` statements by default (`-- [SAFE MODE SKIPPED]`) to safeguard against accidental data loss.
    - **Transactional Wrapping**: Optional transaction enclosure (`BEGIN; ... COMMIT;`) for atomic migrations.
  - **Granular Filter Tabs**: Filter comparison items by `Differences Only` (default), `All Items`, `Columns Only`, and `Indexes Only`.
  - **Seamless Actionable Workflows**: One-click clipboard copy with visual feedback, and direct loading into SQL Console (`schema_migration.sql`) for immediate review, customization, or execution.
  - **Integrated Entry Points**: Accessible directly from Database Explorer table context menus ("Compare Structure..." / "结构对比与迁移向导...") and Schema Viewer toolbar (`Diff`).
  - **Bilingual Localization (i18n)**: Comprehensive English and Simplified Chinese (简体中文) translations across all wizard selectors, status tags, option pills, and tooltips.

- 🛠️ **Improvements & Fixes**:
  - **Data Grid**: Resolved an issue where delete row button was unresponsive and improved data result resolution.
