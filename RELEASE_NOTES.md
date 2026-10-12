## 🦀 zqlcrab v0.1.10

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.10 Release Highlights:
- ⏱️ **Visual SQL Query Profiler & Resource Timeline Panel (SQL 查询执行画像与资源时序耗时分析)**:
  - **Multi-Dialect Execution Engine & Resource Metrics Extraction**:
    - **MySQL Native Profiling**: Orchestrates `SET profiling = 1`, runs query statement events, captures `SHOW PROFILE ALL FOR QUERY ...`, parsing execution status stages, CPU user/system times, and block I/O operations (`Block_ops_in`, `Block_ops_out`).
    - **PostgreSQL Buffer & Plan Analysis**: Dispatches `EXPLAIN (ANALYZE, BUFFERS, TIMING, SUMMARY, FORMAT JSON/TEXT)`, analyzing execution vs. planning times, buffer cache hits vs. reads (`Shared Hit Blocks`, `Shared Read Blocks`), and node execution latency.
    - **SQLite & Synthetic Client Pipeline**: Measures high-resolution microsecond client execution phases (Syntax Parsing & AST Validation, Cost-based Query Planning, Virtual Machine Engine Step Execution, Row Stream & Buffering).
  - **Console Bottom Pane 3-Tab Architecture**:
    - Seamless bottom navigation across 3 specialized views: `Results` (Data Grid), `Execution Plan` (EXPLAIN Visual Tree & Cost Breakdown), and `Profile` (Resource Timeline & Gantt Waterfall).
    - Top toolbar action button `Profile` (`⌘⇧P` / `Ctrl+Shift+P`) with loading animation and status feedback.
  - **Visual Waterfall Timeline & Gantt Chart Visualization**:
    - Microsecond time scale markers across the query timeline (0 ms, 25%, 50%, 75%, 100%).
    - Proportional horizontal execution bars color-coded by category:
      - Green (`ThemeColors::SUCCESS`) for Data Transmission & Fetch.
      - Blue (`ThemeColors::PRIMARY_LIGHT`) for Engine Execution.
      - Orange (`ThemeColors::WARNING`) for Optimizer & Statistics.
      - Red (`ThemeColors::ERROR`) alert for bottleneck stages consuming >35% of query latency.
    - Rich interactive tooltips displaying exact microsecond duration, percentage of total time, start offset, and engine details.
  - **Top KPI Summary Metrics Cards**:
    - *Total Duration*: Overall query latency in milliseconds and microseconds.
    - *Planning Time*: Time spent in compilation, catalog lookup, and AST optimization.
    - *Execution Time*: Time spent in engine virtual machine execution.
    - *Buffer Cache Hit Ratio*: Shared memory buffer hit ratio percentage (e.g. `100.0%`).
    - *Bottleneck Stage*: Heaviest query stage highlighted with badge.
  - **Tabular Execution Breakdown & Export**:
    - Tabular breakdown listing stage index, stage name, category badge, start offset, duration (ms), proportion ratio (%), and detailed I/O / buffer attributes.
    - One-click copy formatted execution profile to system clipboard as a structured Markdown report.
  - **Complete Bilingual Localization**:
    - Comprehensive English and Simplified Chinese (简体中文) translations across all KPI metrics, stage categories, toolbar actions, and tooltips.
