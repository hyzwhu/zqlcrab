## 🦀 zqlcrab v0.1.9

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.9 Release Highlights:
- 🚀 **Visual Storage Capacity Analyzer & Table Maintenance Diagnostics Wizard (库表存储容量分析与智能运维体检向导)**:
  - **Multi-Engine Physical Storage Allocation Analytics**:
    - Multi-dialect engine support inspecting physical disk footprints across SQLite, MySQL, and PostgreSQL.
    - Deep metrics collection inspecting total physical table size, raw data space, index B-Tree allocations, free/bloat reclaimable space, row counts, and average row size.
    - PostgreSQL support for vacuum diagnostics: dead tuple counts (`n_dead_tup`), `last_vacuum`, and `last_analyze` timestamps.
    - MySQL `information_schema.TABLES` metrics inspection (`DATA_LENGTH`, `INDEX_LENGTH`, `DATA_FREE`, `AVG_ROW_LENGTH`, `ENGINE`).
    - SQLite physical page breakdown (`PRAGMA page_count`, `PRAGMA page_size`, `PRAGMA freelist_count`).
  - **2-Tab Interactive Diagnostics Experience**:
    - **Tab 1: Storage Capacity Analysis (存储容量深度分析)**:
      - 6 KPI summary overview cards: Total Database Footprint, Table Data Space, Index Space, Reclaimable Bloat / Free, Inspected Tables, and Total Row Records.
      - Dual-color progress bars visually illustrating the data vs. index allocation ratio for each table.
      - Search filter input and batch selection controls (`Select All`, `Deselect All`).
      - "Maintain Selected Tables..." action button jumping directly to Diagnostics with pre-selected tables.
    - **Tab 2: Maintenance & Diagnostics Wizard (智能运维与健康体检)**:
      - Dialect-specific operational maintenance routines:
        - `Optimize / Vacuum`: Reclaim unused disk space, defragment pages, and shrink database files (`OPTIMIZE TABLE`, `VACUUM (ANALYZE)`, `VACUUM FULL`, `VACUUM`).
        - `Integrity Check`: Verify structural and B-Tree page consistency for corruption (`CHECK TABLE ... EXTENDED`, `ANALYZE VERBOSE`, `PRAGMA integrity_check`).
        - `Analyze Statistics`: Collect and refresh optimizer query distribution statistics (`ANALYZE TABLE`, `ANALYZE`, `PRAGMA optimize`).
        - `Rebuild Indexes`: Reconstruct fragmented or damaged index trees (`REINDEX TABLE`, `REINDEX`).
        - `Checksum Verification`: Calculate cryptographic row checksums or quick page consistency checks (`CHECKSUM TABLE ... EXTENDED`, `PRAGMA quick_check`).
      - Dialect-tailored configuration switches: PostgreSQL `VACUUM FULL` toggle and MySQL `EXTENDED` check toggle.
      - Real-time progress bar with percentage, table counter, and active table indicator.
      - Live color-coded audit log stream with status badges (`[OK]`, `[WARN]`, `[ERR]`), execution durations, and one-click log copy to clipboard.
      - Post-run execution summary report with passed/failed counts and total elapsed duration.
  - **Seamless Explorer Entry Points**:
    - Database Explorer header action (`Storage & Maintenance Wizard...` with `Gauge` icon).
    - Table context menu action ("Maintain & Optimize Table..." / "库表维护与体检...").
  - **Lexical Query Safety Compatibility**:
    - Extended `QuerySafetyValidator::is_result_set_query` to properly recognize maintenance result commands (`CHECK`, `OPTIMIZE`, `ANALYZE`, `REPAIR`, `CHECKSUM`) returning tabular diagnostic rows across database drivers.
  - **Complete Bilingual Localization**:
    - Comprehensive English and Simplified Chinese (简体中文) translations across all KPI cards, table breakdown columns, action cards, options, and status badges.
