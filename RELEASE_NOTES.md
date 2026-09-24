## 🦀 zqlcrab v0.1.4

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.4 Release Highlights:
- 🚀 **Visual Data & Schema Export Wizard & Table Dump (数据导出向导与全量转储引擎)**:
  - **Multi-Format Serialization**: Seamlessly export tables and query slices into SQL Script (Full Dump / DDL / Inserts), CSV, TSV, JSON Array, NDJSON (stream), and GitHub Flavored Markdown (GFM).
  - **Comprehensive Dump Options**: Multi-dialect SQL dumps support `DROP TABLE IF EXISTS` (`CASCADE` for PG), dialect-aware transaction boundaries (`BEGIN TRANSACTION` / `START TRANSACTION`), configurable batch multi-row `INSERT` sizes, and engine identifier quoting.
  - **Granular Filter & Scope**: Choose between Structure & Data, Data Only, or Structure Only (DDL); apply arbitrary `WHERE` conditions and row limits directly in the export wizard.
  - **Live Dynamic Preview**: Instant 20-row preview panel renders real-time serialization results with truncation indicators without memory overhead.
  - **Flexible Destinations**: Export directly to local disk with timestamped filenames (`rfd::FileDialog` / native file manager reveal) or copy formatted content straight to the system clipboard.
  - **Seamless Context Navigation**: Launch the wizard directly via the table right-click context menu in the sidebar (`Export Data / Dump...`).
