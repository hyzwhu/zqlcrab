## 🦀 zqlcrab v0.1.16

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.16 Release Highlights:
- 🛡️ **Comprehensive Memory & Resource Leak Elimination (全栈内存与资源泄漏隐患彻底根除与优化)**:
  - **PostgreSQL Connection Task Lifecycle Management**:
    - Tracked background connection poller `JoinHandle` in `PostgresAdapter`.
    - Proactively aborted poller tasks on `disconnect()` and implemented RAII `Drop` to guarantee no lingering Tokio tasks or unclosed sockets.
  - **MySQL Connection Pool Async Teardown via RAII**:
    - Implemented `Drop for MysqlAdapter` to safely and asynchronously disconnect unclosed connection pools on the Tokio runtime when dropped.
  - **Guaranteed Connection Teardown in Error Paths**:
    - Fixed `ConnectionManager::test_config` to ensure `disconnect().await` is executed on both success and error branches.
    - Tracked temporary connections during cross-database data transfers (`start_transfer_execution`) to guarantee source/target temporary connections are always closed.
  - **Generation-Token Loop Cancellation in Sessions View**:
    - Introduced generation token verification in the sessions auto-refresh background task to terminate outdated polling loops immediately when switching navigation tabs.
  - **Autocomplete Metadata Cache Pruning & Multi-DB Isolation**:
    - Automatically pruned dropped table column mappings from `SqlMetadataCache` and cleared metadata cache upon switching active database connections to prevent memory accumulation.
  - **Eager Memory Deallocation on Modal Dismissal**:
    - Explicitly cleared and released large row matrices, AST syntax trees, raw SQL dump scripts, and review plans upon closing tool modals (Data Diff, Schema Diff, Mock Data, SQL Dump/Restore, Maintenance).
