## 🦀 zqlcrab v0.1.4

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.4 Release Highlights:
- ✨ **Visual Mock Data Generator & Batch Seeder (可视化模拟测试数据生成器与批量填充引擎)**:
  - **Intelligent Heuristic Inference**: Automatically detects column semantic types based on field names (`id`, `name`, `email`, `phone`, `status`, `price`, `timestamp`, `uuid`, etc.) and database data types to assign smart mock generator strategies.
  - **Comprehensive Generator Strategies**: Generate realistic person names, emails, phone numbers, UUIDs v4, auto-increment sequences, random integers, floating point numbers with custom decimals, timestamps, dates, booleans, enum choices, and lorem ipsum sentences.
  - **Interactive Live Preview**: Sample preview table dynamically renders the first 8 rows of realistic test data before insertion with a one-click regenerate option.
  - **Multi-Dialect Atomic Transactions**: Batch seeds up to 5,000+ rows in configurable transaction batches (100–500 rows) with engine-specific identifier quoting (PostgreSQL/SQLite `""`, MySQL `` ` ``) and literal escaping, automatically rolling back on error.
  - **Real-Time Progress & Instant Grid Reload**: Visual progress bar tracking insertion percentage and speed, with automatic data grid synchronization upon completion.
  - **Sidebar Table Context Menu**: Accessible via right-clicking any table in the schema tree -> `Generate Mock Data...` (`生成模拟测试数据...`).
- 🚀 **Visual Data & Schema Export Wizard & Table Dump (数据导出向导与全量转储引擎)**:
  - **Multi-Format Serialization**: Seamlessly export tables and query slices into SQL Script (Full Dump / DDL / Inserts), CSV, TSV, JSON Array, NDJSON (stream), and GitHub Flavored Markdown (GFM).
  - **Comprehensive Dump Options**: Multi-dialect SQL dumps support `DROP TABLE IF EXISTS` (`CASCADE` for PG), dialect-aware transaction boundaries (`BEGIN TRANSACTION` / `START TRANSACTION`), configurable batch multi-row `INSERT` sizes, and engine identifier quoting.
  - **Granular Filter & Scope**: Choose between Structure & Data, Data Only, or Structure Only (DDL); apply arbitrary `WHERE` conditions and row limits directly in the export wizard.
  - **Live Dynamic Preview**: Instant 20-row preview panel renders real-time serialization results with truncation indicators without memory overhead.
  - **Flexible Destinations**: Export directly to local disk with timestamped filenames (`rfd::FileDialog` / native file manager reveal) or copy formatted content straight to the system clipboard.
  - **Seamless Context Navigation**: Launch the wizard directly via the table right-click context menu in the sidebar (`Export Data / Dump...`).
