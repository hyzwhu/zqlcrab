## 🦀 zqlcrab v0.1.21

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.21 Release Highlights:
- 🛠️ **Visual Table Designer & Multi-Dialect ALTER Migration Engine (可视化表结构设计器与多方言迁移生成引擎)**:
  - **Intuitive GUI Schema Designer**: Visual table editing interface supporting column additions, type modifications, nullability constraints, defaults, primary keys, and auto-increment properties.
  - **Multi-Dialect ALTER DDL Generation**: Automatic synthesis of differential `ALTER TABLE` migration scripts across MySQL, PostgreSQL, SQLite, and MariaDB.
  - **Non-Destructive Schema Synchronizer**: Preview before execution with safety guards against destructive operations.
- 🔌 **MySQL Driver TDSQL & Cloud Proxy Compatibility Fix (MySQL 驱动 TDSQL 与分布式云网关兼容性修复)**:
  - **Fixed `Unknown system variable 'socket'` Failure**: Disabled `prefer_socket` in `mysql_async` connection builder.
  - Resolves connection handshake crashes when connecting to Tencent Cloud TDSQL (port 15071), TiDB, Doris, StarRocks, and other MySQL-compatible distributed gateways that do not expose the internal single-node `@@socket` variable.
  - Enforces clean TCP wire-protocol communication across all MySQL-compatible database engines.
- 🧪 **Stability & Test Suite**:
  - All 193 unit and integration tests passing cleanly with zero regressions.
