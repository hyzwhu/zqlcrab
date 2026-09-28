## 🦀 zqlcrab v0.1.7

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.7 Release Highlights:
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
