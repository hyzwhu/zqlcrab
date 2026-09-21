# Zqlcrab 跨端与服务端分层架构设计 (Architecture Blueprint)

## 1. 架构演进背景

Zqlcrab 基于 Rust 与 GPUI / `gpui-kit` 构建。`gpui-kit` 支持编译并运行在 WebAssembly (WASM) 运行环境。
在传统桌面客户端运行环境中，应用程序可直接调用操作系统原生 Socket API 连接目标数据库（PostgreSQL、MySQL、SQLite 本地文件等）。但在浏览器/WebAssembly 环境中，受限于浏览器同源策略（SOP）和沙箱安全限制：
- **浏览器环境无法直接发起任意原始 TCP/Socket 连接**（例如直连 3306 或 5432 端口）；
- **本地文件系统受限**，无法直接打开本地任意绝对路径的 SQLite 数据库；
- Web 端必须采用 **Web (WASM 前端) + HTTP / WebSocket API + 后端轻量网关服务 (Server)** 的架构模式。

为了保持一套业务代码兼顾 **原生桌面直连（最极致性能）** 与 **Web 端服务化代理（轻量跨端无缝访问）**，Zqlcrab 采用清晰的分层解耦架构（Clean Architecture & Ports and Adapters）。

---

## 2. 总体分层拓扑

```
+---------------------------------------------------------------------------------+
|                               Presentation Layer                                |
|                                                                                 |
|   +---------------------------------------+   +-----------------------------+   |
|   |          Desktop Native GUI           |   |       Web / WASM GUI        |   |
|   |         (macOS / Linux / Windows)     |   |    (gpui-kit wasm32 build)  |   |
|   +---------------------------------------+   +-----------------------------+   |
|                                       \           /                             |
|                                        v         v                              |
|   +-------------------------------------------------------------------------+   |
|   |           UI Components, EditorState, DataGrid & Changeset Engine       |   |
|   +-------------------------------------------------------------------------+   |
+---------------------------------------------------------------------------------+
                                         |
                                         v
+---------------------------------------------------------------------------------+
|                          Application & Domain Core                              |
|                          (100% Platform Agnostic)                               |
|                                                                                 |
|   - SqlMetadataCache & Autocomplete Provider                                    |
|   - GridChangeset (Diff, Cell Editing, Staging & Undo/Redo)                     |
|   - SQL Generator & Review Plan (INSERT, UPDATE, DELETE, DDL AST)               |
|   - Safety Validator & SQL Formatter                                            |
|   - EXPLAIN Parser & Tree Visualizer                                            |
+---------------------------------------------------------------------------------+
                                         |
                                         v
+---------------------------------------------------------------------------------+
|                         Data Gateway & Transport Layer                          |
|                                                                                 |
|                        trait DatabaseAdapter (Async)                            |
|                                        |                                        |
|               +------------------------+------------------------+               |
|               |                                                 |               |
|               v                                                 v               |
|   +-----------------------+                         +-----------------------+   |
|   |    Native Drivers     |                         |   RemoteHttpAdapter   |   |
|   | (Postgres/MySQL/SQLite|                         | (HTTP / JSON Gateway) |   |
|   +-----------------------+                         +-----------------------+   |
|               |                                                 |               |
+---------------|-------------------------------------------------|---------------+
                | (Raw TCP / Local OS Filesystem)                 | (HTTP POST / WS)
                v                                                 v
    +-----------------------+                         +-----------------------+
    |   Target Databases    |                         |     zqlcrab-server    |
    | (PG / MySQL / SQLite) |                         | (Rust Axum / Actix)   |
    +-----------------------+                         +-----------------------+
                                                                  |
                                                                  v (Relay TCP)
                                                      +-----------------------+
                                                      |   Target Databases    |
                                                      +-----------------------+
```

---

## 3. 核心分层职责

### 3.1 Presentation Layer (表现层)
- **GPUI / gpui-kit UI 基础设施**：负责窗口、画布渲染、SVG/图标、布局排版、快捷键分发。
- **Query Console**：SQL 语法高亮、自动补全、选区识别、执行调度。
  - **选区执行**：通过 `EditorState::selected_text()` 提取当前鼠标高亮选中的 SQL 片段，优先单步执行选区。
  - **点号即时补全**：输入 `.` 触发即时解析左侧表名标识符（支持反引号、双引号去重），即使字段前缀为空，也立即提示该表所有字段。
- **DataGrid & Inspector**：虚拟滚动表格、单元格就地编辑、NULL 设置、多行插入与草稿暂存。

### 3.2 Application & Core Domain (核心领域模型)
不依赖任何操作系统原生 Socket 或系统调用，纯 Rust 内存计算，可在桌面与 WASM 双向无缝编译：
- `GridChangeset`：追踪用户在表格上的增删改变更，生成幂等原子事务批处理脚本。
- `SqlReviewPlan`：执行前 SQL 审查计划生成，安全校验拦截无 `WHERE` 条件的破坏性操作。
- `SqlMetadataCache`：缓存已知 Schema、表、列及类型，为智能补全引擎提供实时检索。

### 3.3 Gateway & Adapter Trait (数据网关层)
统一抽象 `DatabaseAdapter` 接口：
```rust
#[async_trait]
pub trait DatabaseAdapter: Send + Sync {
    async fn connect(&mut self) -> DbResult<()>;
    async fn disconnect(&mut self) -> DbResult<()>;
    fn is_connected(&self) -> bool;
    async fn test_connection(&self) -> DbResult<ConnectionStatus>;
    async fn execute_query(&self, sql: &str) -> DbResult<QueryResult>;
    async fn execute_batch(&self, sql: &str) -> DbResult<()>;
    async fn list_databases(&self) -> DbResult<Vec<DatabaseSchema>>;
    async fn list_tables(&self, database: Option<&str>, schema: Option<&str>) -> DbResult<Vec<TableInfo>>;
    async fn list_columns(&self, database: Option<&str>, schema: Option<&str>, table: &str) -> DbResult<Vec<ColumnInfo>>;
    async fn list_indexes(&self, database: Option<&str>, schema: Option<&str>, table: &str) -> DbResult<Vec<IndexInfo>>;
    async fn get_table_ddl(&self, database: Option<&str>, schema: Option<&str>, table: &str) -> DbResult<Option<String>>;
}
```

### 3.4 Transport Implementations (传输协议实现)
1. **`NativeAdapter` (桌面直连模式)**：
   - `PostgresAdapter`：基于 `tokio-postgres` 与 TLS。
   - `MysqlAdapter`：基于 `mysql_async` 连接池。
   - `SqliteAdapter`：基于 `rusqlite` 操作本地嵌入式文件或 `:memory:` 数据库。
2. **`RemoteHttpAdapter` (Web / WASM 服务化代理模式)**：
   - 将数据库指令封装为标准化 REST/JSON 请求：
     - `POST /api/v1/query`：发送 `GatewayQueryRequest { connection_id, sql, timeout_secs }`，返回 `QueryResult`；
     - `POST /api/v1/batch`：执行审查后的事务批处理；
     - `GET /api/v1/metadata/tables`、`GET /api/v1/metadata/columns`：拉取元数据；
   - 支持通过 JWT / Bearer Token 保证多租户连接安全性。

---

## 4. 演进路线规划

1. **v0.1.1（当前版本）**：
   - 完成点号（`.`）即时列补全优化；
   - 完成鼠标选区 SQL 优先执行与 EXPLAIN 分析；
   - 确立清晰的分层架构并实现 `RemoteHttpAdapter` 协议接口。
2. **v0.2.0（未来版本）**：
   - 提取独立轻量级服务端二进制包 `zqlcrab-server`（基于 Axum / Tokio）；
   - 在构建目标中增加 `wasm32-unknown-unknown` 前端产物部署方案。
