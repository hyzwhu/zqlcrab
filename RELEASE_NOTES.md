## 🦀 zqlcrab v0.1.8

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.8 Release Highlights:
- 🚀 **Cross-Database Data Transfer Wizard (跨库数据传输与迁移向导)**:
  - **Heterogeneous Cross-Engine Migration Engine**:
    - Seamlessly migrate database table structures (DDL) and data records (DML) across different database engines and active connections (SQLite, MySQL, PostgreSQL).
    - Intelligent cross-dialect type translation matrix mapping data types accurately (e.g. SQLite `INTEGER/REAL/TEXT/BLOB` <-> MySQL `INT/DOUBLE/VARCHAR/LONGBLOB` <-> PostgreSQL `INTEGER/DOUBLE PRECISION/TEXT/BYTEA/BOOLEAN`).
  - **4-Step Intuitive Visual Transfer Wizard**:
    - **1. Source & Target Selection**: Interactive source and target connection and database pickers with safeguards preventing identical source/target execution.
    - **2. Tables & Mapping**: Full table checklist with live search filter, batch selection controls (`Select All`, `Deselect All`), and per-table or batch scope toggle badges (`Structure & Data`, `Structure Only`, `Data Only`).
    - **3. Options & Performance Tuning**:
      - Automated `CREATE TABLE` and `DROP TABLE IF EXISTS` options with primary key and auto-increment preservation.
      - `TRUNCATE / DELETE` target table data before insertion.
      - Temporary foreign key constraint toggling (`PRAGMA foreign_keys = OFF/ON`, `SET FOREIGN_KEY_CHECKS = 0/1`, `SET CONSTRAINTS ALL DEFERRED`).
      - Transactional batch execution and chunked multi-row `INSERT` batch size selection (100, 200, 500, 1000 rows).
      - Fault-tolerant error handling (`Continue on error & log failures` vs immediate abort).
    - **4. Live Execution, Progress Bar & Real-Time Audit Log**:
      - Real-time progress bar with percentage, table index counters, and transferred row counts.
      - Color-coded scrollable audit log stream classifying `[INFO]`, `[DDL]`, `[DATA]`, `[OK]`, `[WARN]`, and `[ERR]` entries.
      - Abort transfer support with instant cancellation.
      - Post-execution summary card with elapsed time, completed tables, rows written, error counts, and one-click log copy to clipboard.
  - **Pervasive Entry Points**:
    - Accessible directly from Database Explorer header actions (`Transfer Database...` / `ArrowRight` icon).
    - Table context menu action ("Transfer Table..." / "传输数据表...").
  - **Complete Bilingual Localization**:
    - Comprehensive English and Simplified Chinese (简体中文) translations across all wizard steps, options, table badges, and audit log labels.
