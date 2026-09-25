## 🦀 zqlcrab v0.1.5

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.5 Release Highlights:
- ✨ **SQL Snippets & Reusable Script Library (SQL 代码片段与常用脚本库)**:
  - **Curated Multi-Dialect Diagnostics Catalog**: Pre-packaged, production-tested diagnostic and performance monitoring queries for PostgreSQL (Active Queries, Lock Contention, Table Bloat & Sizes, Cache Hit Ratio, Unused Indexes), MySQL (Processlist & Thread Status, InnoDB Running Transactions, Table Data & Index Sizes, Engine Summary), and SQLite (PRAGMA Integrity Check, Table & Index Inventory, Vacuum & Optimize, Foreign Key Check).
  - **Reusable Query Templates**: Universal boilerplate templates for offset-based pagination, conditional batch updates with change auditing, and safe data synchronization.
  - **Custom Snippet Management & Persistence**: Create, edit, and delete customized SQL snippets with title, category, dialect tagging, description, and SQL body. Persisted locally to `~/.config/zqlcrab/snippets.json` for persistence across app restarts.
  - **Full-Featured Workspace View (`ActivityNav::Snippets`)**: Dedicated left activity bar tab featuring real-time full-text search across snippet titles, descriptions, and SQL syntax, combined with one-click dialect and category filter pills.
  - **One-Click Query Console Integration**: Directly load snippets into the active SQL editor, execute immediately in a query console tab, or copy SQL directly to the system clipboard.
  - **Console "Save as Snippet" Workflow**: Instantly save the currently active query from the SQL editor into your personal snippet library via the console toolbar button (`Save as Snippet`).
  - **Bilingual Localization (i18n)**: Comprehensive English and Simplified Chinese (简体中文) localization across all snippet labels, tooltips, dialogs, and diagnostic descriptions.
