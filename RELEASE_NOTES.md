## 🦀 zqlcrab v0.1.14

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.14 Release Highlights:
- 📊 **Data Grid Toolbar Optimization & Export Dropdown Menu (数据表格底部工具栏防挤压与导出动作下拉菜单化)**:
  - **Unified Export Dropdown Menu**:
    - Replaced individual, horizontal export action buttons (CSV, JSON, Markdown, SQL) with a consolidated, space-efficient `Export` dropdown menu button.
    - Each export format option is clearly labeled with its dedicated file icon (`CSV (*.csv)`, `JSON (*.json)`, `Markdown (*.md)`, `SQL INSERT (*.sql)`).
  - **Responsive Flex Layout Protection**:
    - Wrapped action groups and pagination elements with `.flex_shrink_0()` to eliminate layout overlap or button truncation on narrower windows and split views.
    - Preserved immediate access to Import and Table Data Diff actions while keeping page navigation indicators crisp and aligned.
