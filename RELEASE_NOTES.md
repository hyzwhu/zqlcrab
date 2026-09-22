## 🦀 zqlcrab v0.1.2

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.2 Release Highlights:
- 🛡️ **Modal Overlay Event Occlusion & Click Penetration Elimination (遮罩层全局事件隔离防穿透)**:
  - Added GPUI `.occlude()` to all modal backdrops throughout the application, ensuring upper-layer dialog interactions strictly block mouse events from penetrating down to the underlying DataGrid cells, toolbar action buttons, editor buffers, and workspace navigation tabs.
  - Comprehensive coverage across all dialog flows: New Connection Modal (`ConnectionDialog`), Connection Error Diagnostic Modal (`ConnectionErrorDialog`), Table Visual Designer (`CreateTableModal`), Destructive Actions Confirmation (`ConfirmDialog`), Atomic SQL Changeset Review (`SqlReviewModal`), and Full Cell Value Inspector modal (`DataGrid`).
- 💻 **Mac-Native Monochrome Silhouette Status Bar Tray (系统托盘剪影与高清比例重构)**:
  - Converted the macOS status bar tray icon into a template monochrome silhouette (`[image setTemplate: YES]`), automatically adapting to macOS light and dark menu bars with native system styling.
  - Tight bounding-box cropped icon assets (`tray-icon.png` and `tray-icon@2x.png`) with proportional dimensions (`22.0pt × 16.0pt`) for crisp rendering on Retina displays without edge clipping.
- 📐 **Pixel-Perfect Colon-Aligned Tray Metrics (托盘指标按冒号精准对齐)**:
  - Rebuilt the status bar menu metric rows (`Active:`, `DB:`, `Ping:`, `Memory:`) using native AppKit dual-column `NSView` items.
  - Right-aligned metric labels and left-aligned metric values perfectly aligned at the colon `:`, eliminating proportional font spacing misalignments.
  - Real-time memory footprint tracking via macOS Darwin kernel `TASK_VM_INFO` physical footprint API matching Activity Monitor memory metrics.
- 🔄 **Remote Gateway Protocol & Architectural Refinement**:
  - Bumped server protocol and mock client specifications to v0.1.2.
