## 🦀 zqlcrab v0.1.6

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.6 Release Highlights:
- ⚡ **Active Sessions & Real-Time Process Monitor (活跃会话与实时进程监控面板)**:
  - **Multi-Dialect Process Inspection**: Native, dialect-aware querying of database backend activities for PostgreSQL (`pg_stat_activity` with backend PID, client IP, transaction age, wait event, query duration, query snippet), MySQL (`information_schema.PROCESSLIST` with connection ID, user, db, host, command, latency, state, current statement), and SQLite (in-process session inventory via `PRAGMA database_list`).
  - **Color-Coded Latency Gradient Classification**: Real-time duration metrics with visual severity badges for quick bottleneck identification — `Normal` (< 3s), `Notice` (3s–15s), `Warning` (15s–60s), and `Critical` (≥ 60s).
  - **Dedicated Activity Bar Workspace (`ActivityNav::Sessions`)**: One-click navigation with live session counters (Total Connections, Active Queries, and Slow Queries) alongside instant full-text filtering across PID, user, database, client host, state, wait events, and query text.
  - **Auto-Refresh Engine**: Configurable live polling intervals (`Off`, `5s`, `10s`, `30s`) keeping administrators and developers continuously informed of system load and locked transactions.
  - **Safe Process Termination & Cancellation with Confirmation Modal**:
    - **Cancel Query (`KILL QUERY` / `pg_cancel_backend`)**: Instantly abort long-running statements while preserving active client connections.
    - **Terminate Connection (`KILL` / `pg_terminate_backend`)**: Forcefully terminate rogue sessions with rollback safeguards.
    - **Visual Safety Guardrails**: Critical modal confirmation dialog (`KillConfirmModal`) displaying exact SQL commands, target metadata, and execution warnings to prevent accidental production disruptions.
    - **Current Backend & SQLite Protection**: Identifies caller connections to prevent accidental suicide termination, with safe handling for embedded SQLite engines.
  - **Integrated Query Actions**: One-click copy of active session SQL statements and direct opening of slow queries in the SQL Query Editor for instant EXPLAIN plan analysis and troubleshooting.
  - **Bilingual Localization (i18n)**: Full English and Simplified Chinese (简体中文) support across all session views, metrics badges, kill confirmation dialogs, and auto-refresh indicators.
