## 🦀 zqlcrab v0.1.18

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.18 Release Highlights:
- 📜 **Advanced SQL Query History Audit, Search & Export (高级 SQL 查询历史审计、多维检索与导出分析)**:
  - **Reactive Keyword Search & Multi-Dimensional Filtering**:
    - Integrated reactive search input box (`Entity<InputState>`) with instant event subscriptions matching SQL text, connection profile names, and database error messages.
    - Added segmented status filter pills: `All` (全部), `Success` (仅成功), `Errors` (仅异常), `Slow` (慢查询 >500ms), and `Starred` (⭐ 仅收藏) with real-time query counts.
    - Added dialect filter pills: `All Dialects`, `PostgreSQL`, `MySQL`, and `SQLite`.
  - **Audit KPI Metrics Dashboard**:
    - Introduced execution performance KPI strip displaying:
      - **Total Executed Queries** count.
      - **Execution Success Rate (%)** with color-coded health indicators.
      - **Slow Queries Count (>500ms)** in warning amber badge.
      - **Total Cumulative Duration (ms)** consumed across queries.
  - **Multi-Format History Audit Export**:
    - Export filtered history directly to RFC4180 CSV, formatted JSON, or standard runnable `.sql` audit batch scripts with metadata header comments.
    - Integrated with native platform save dialog (`rfd::FileDialog`).
  - **Favorite Star Management & Single Record Deletion**:
    - Added one-click star/favorite toggling (`is_favorite`) persisted automatically to `query_history.json`.
    - Added individual history entry deletion without needing to wipe the entire audit trail.
  - **1-Click SQL Snippet Conversion**:
    - Integrated `Save as Snippet` button directly into history item cards, immediately pre-filling and launching `SnippetEditModal`.
  - **Full Bilingual Localization & Testing**:
    - Added complete English and 简体中文 translation keys for history titles, KPI metrics, filters, export tooltips, and badges.
    - All 183 unit tests passing.
