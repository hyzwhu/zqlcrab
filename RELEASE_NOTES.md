## 🦀 zqlcrab v0.1.3

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.3 Release Highlights:
- 📊 **Accurate Affected-Row Counts (影响行数按实际变更显示)**:
  - Saving a grid change reloads the table with `SELECT` instead of running the editor's `DELETE` or `UPDATE` a second time.
  - `BEGIN`, `COMMIT`, and `ROLLBACK` are left out of the affected-row total, so a delete script reports the rows the `DELETE` changed.
  - A `SELECT` that returns no rows keeps its columns instead of showing "0 row(s) affected".
