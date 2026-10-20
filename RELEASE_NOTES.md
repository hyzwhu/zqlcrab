## 🦀 zqlcrab v0.1.19

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.19 Release Highlights:
- ⚡ **Database Index Advisor & Health Diagnostics Engine (数据库索引优化诊断与智能推荐顾问)**:
  - **Comprehensive Index Health Audit & Anomaly Detection**:
    - **Exact Duplicate Index Detection**: Automatically flags identical index definitions sharing the same columns and order, calculating redundant write and storage overhead.
    - **Prefix-Covered Redundant Index Detection**: Identifies non-unique indexes whose column sets form a strict left-prefix of wider composite indexes (e.g. `(user_id)` covered by `(user_id, created_at)`), pinpointing safe-to-drop redundant indexes.
    - **Unindexed Foreign Key Scanning**: Deeply inspects foreign key constraints against existing indexes to detect missing coverage that could cause cascading locks or full table scans on parent `DELETE`/`UPDATE` operations.
    - **Index Saturation & Write-Amplification Warnings**: Warns when table index count exceeds optimal thresholds (>7 indexes) to prevent write-throughput degradation.
    - **Dynamic Health Scoring Gauge (0-100)**: Computes a weighted table index health score categorized into `Excellent (优秀)`, `Good (良好)`, `Fair (一般)`, and `Poor (需优化)` with color-coded gauge metrics.
  - **Query-Driven Missing Index Recommendation Engine**:
    - Analyzes active SQL queries directly from the editor, parsing `WHERE` equality filters, range conditions (`<`, `>`, `BETWEEN`, `LIKE`), and `ORDER BY` sorting clauses.
    - Synthesizes optimal B-tree composite index column order: Equality columns first -> Range columns second -> Sort columns third.
    - Avoids duplicate recommendations if existing indexes already cover the candidate prefix.
  - **Interactive Advisory Modal Dialog (`IndexAdvisorModal`)**:
    - Dual-tab navigation between **Table Health Audit (`HealthAudit`)** and **Query Advisor (`QueryAdvisor`)**.
    - Rich visual KPI cards: Health score gauge, total indexes, redundant/duplicate index count, and unindexed foreign keys.
    - Remediation cards with severity badges (`CRITICAL`, `WARNING`, `INFO`), problem explanations, impact statements, and monospaced DDL preview.
    - Existing table index catalog with index name, composite column lists, and index type badges (`PRIMARY`, `UNIQUE`, `INDEX`).
  - **Dialect-Aware DDL Generation & 1-Click Execution**:
    - Renders dialect-specific `CREATE INDEX` and `DROP INDEX` statements for PostgreSQL (with `CONCURRENTLY` support), MySQL, and SQLite.
    - **1-Click Copy**: Copy individual issue remediation DDL or the entire unified remediation plan.
    - **Open in Console**: Automatically opens a new query tab populated with the synthesized remediation script for inspection and immediate execution.
  - **Full Desktop Integration & Bilingual Localization**:
    - Table context menu entry in the navigation sidebar: `Index Advisor & Health Diagnostics... (索引优化与体检顾问...)`.
    - Query Console toolbar quick-access button: `⚡ Advisor (索引顾问)`.
    - Complete English and 简体中文 translation keys and unit tests.
    - All 190 tests passing with zero warnings or failures.
