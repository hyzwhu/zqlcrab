## 🦀 zqlcrab v0.1.15

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.15 Release Highlights:
- ⚡ **Query Console Toolbar Optimization & Snippets Dropdown (SQL控制台工具栏响应式防挤压与下拉菜单整合)**:
  - **Unified Snippets & Actions Dropdown Menu**:
    - Consolidated multiple separate buttons (`Snippets`, `Save as Snippet`, `Clear`) into a compact, space-efficient `Snippets` dropdown menu button.
    - Quick access to browsing the SQL snippets library (`📁 Browse Snippet Library`), saving the current query (`🔖 Save as Snippet`), and clearing the query editor (`🗑️ Clear SQL Editor`).
  - **Responsive Flex Layout Protection**:
    - Applied `.flex_shrink_0()` to core action buttons (`Run`, `Format`, `Explain`, `Profile`, and `Snippets`) to eliminate button truncation, distorted margins, or label overlap on narrower screens or split-view panels.
    - Added an `overflow_x_scroll()` gesture-friendly container for the left action group, ensuring smooth horizontal scrolling even in ultra-compact views.
    - Protected the right-side active database connection badge with flex-shrink protection and text ellipsis.
