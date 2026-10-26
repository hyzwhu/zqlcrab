## 🦀 zqlcrab v0.1.20

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.20 Release Highlights:
- 🗺️ **Entity-Relationship (ER) Diagram Standalone High-Resolution SVG Export (实体关系拓扑图谱矢量 SVG 导出与保存)**:
  - **Standalone Vector SVG Rendering Engine (`ErDiagramGraph::to_svg`)**:
    - Generates self-contained, high-resolution vector SVG graphics styled with a modern dark theme and custom typography.
    - Dynamically calculates canvas boundaries with auto-padding and viewBox scaling for lossless zoom and print-ready architectural documentation.
    - Renders interactive table entities with drop-shadow filters, headers, column count badges, and schema dividers.
    - Features color-coded PK (`PRIMARY KEY`) and FK (`FOREIGN KEY`) indicator badges alongside column data types.
    - Smoothly renders cubic and quadratic Bezier curves for foreign key relations with cardinality badges (`1:1`, `1:N`, `N:M`).
    - Robust XML escaping for entity, column, constraint, and label strings to prevent XML injection and rendering corruption.
  - **Seamless UI Integration & Native File Export**:
    - Integrated native **Export SVG** button in the ER diagram workspace toolbar with download icons and responsive tooltips.
    - Native file dialog integration (`rfd::FileDialog`) with default file naming (`er_diagram.svg`) and `.svg` format filtering.
    - Real-time notification and error feedback displayed on the application footer status bar upon export completion.
  - **Localization & Unit Testing**:
    - Full English and 简体中文 translation key coverage (`er_diagram.export_svg`).
    - Comprehensive unit test suite (`test_svg_export`) covering standalone SVG structure, table cards, badges, and relational edge rendering.
    - All tests passing with zero warnings or failures.
