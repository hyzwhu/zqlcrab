## 🦀 zqlcrab v0.1.7

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.7 Release Highlights:
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

