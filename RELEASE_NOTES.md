## 🦀 zqlcrab v0.1.13

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.13 Release Highlights:
- 🖱️ **Horizontal Scrolling for SQL Snippets & Script Filter Tabs (SQL 代码片段库标签栏支持左右滑动)**:
  - **Fluid Horizontal Scroll Container**:
    - Resolved filter pill clipping where dialect and category filter tabs overflowed the container on smaller windows/displays without the ability to scroll.
    - Assigned unique ElementId (`snippet_tabs_scroll`), `min_w_0()`, and `overflow_x_scroll()` to enable horizontal gesture navigation via trackpad swipes and mouse wheel scrolling.
  - **Flex-Shrink Protection & Visual Separation**:
    - Applied `.flex_shrink_0()` to each dialect and category pill, preventing pill deformation, text truncation, or cramped button layouts.
    - Added an aesthetic vertical divider between dialect filters and category filters for enhanced visual hierarchy.
- 🛡️ **Modal Scroll Event Isolation & Hit-Test Occlusion (弹窗上下滑动事件隔离，防止背景视图联动滑动)**:
  - **GPUI Hit-Test Occlusion (`.occlude()`)**:
    - Fixed a scroll event propagation issue where scrolling vertically inside the Storage Capacity Analysis & Maintenance Wizard modal simultaneously triggered scrolling on the underlying view (e.g., SQL Snippets list, Data Grid, or Query Editor).
    - Added `.id(...).occlude()` to modal backdrop overlays and modal cards, terminating GPUI hit-test traversal (`HitboxBehavior::BlockMouse`) and preventing events from leaking to background layers.
  - **Universal Dialog Shielding**:
    - Standardized modal occlusion across all application wizards and dialogs: Storage Maintenance Wizard, Database Dump, Restore, Data Transfer, Mock Data Generator, Schema Diff, Data Diff, Kill Session Confirm, Snippet Editor, and Export Dialog.
