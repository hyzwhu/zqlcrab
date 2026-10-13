## 🦀 zqlcrab v0.1.12

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.12 Release Highlights:
- 🛠️ **Fix Infinite Loading Spinner in Storage Capacity Analyzer & Maintenance Wizard (修复库表维护与存储容量分析向导无限转圈)**:
  - **GPUI Task Lifecycle & RAII Cancellation Fix (`.detach()`)**:
    - Resolved a critical GPUI background task lifecycle bug where asynchronous queries spawned via `cx.spawn(...)` without `.detach()` were immediately cancelled and dropped by RAII cleanup when the enclosing statement finished.
    - Ensured that `refresh_maintenance_metrics`, `run_maintenance`, `start_dump_execution`, `execute_restore_batch`, and `start_transfer_execution` run to completion and properly update UI loading states (`is_loading = false`).
  - **12s Timeout & Graceful Metadata Fallback**:
    - Wrapped catalog metrics retrieval in a 12-second asynchronous timeout (`tokio::time::timeout`), guaranteeing the wizard UI never freezes indefinitely on network latency or catalog locks.
    - Added automatic fallback to cached schema table metadata when system catalog access is restricted or tables are empty.
  - **Multi-Engine Dialect & Pragma Compatibility**:
    - Corrected SQLite storage metrics query using standard table-valued functions (`FROM pragma_page_count(), pragma_page_size(), pragma_freelist_count()`), resolving the `no such function: PRAGMA_PAGE_SIZE` query failure.
    - Expanded PostgreSQL storage scanning across all non-system namespaces and partitioned tables (`relkind IN ('r', 'p')`).
