## 🦀 zqlcrab v0.1.11

Fast, lightweight, GPU-accelerated database desktop client built with Rust & GPUI.

### 📋 Changelog

#### v0.1.11 Release Highlights:
- 🎨 **Sidebar Table List & Underscore Typography Optimization (侧边栏对象树排版与下划线渲染优化)**:
  - **Fixed Underscore Visibility & Vertical Bounds Clipping**:
    - Resolved font baseline descender clipping where underscores (`_`) and letters with descenders (`g`, `p`, `q`, `y`) in snake_case table and connection names (e.g., `lato_analyst`) were truncated by tight layout bounds and `overflow_hidden`.
    - Explicitly configured line height (`line_height: 20px`) across table and connection items, guaranteeing pristine rendering of underscores and special characters.
  - **Comfortable Standard Row Height & Breathing Room**:
    - Upgraded table row height from the cramped ~20px to the standard desktop IDE height `28px` (`min_h: 28px`), providing balanced vertical breathing room and ergonomic spacing.
    - Standardized schema folder and group header rows to `26px` with subtle list gaps (`gap: 1.5px`) to eliminate dense visual crowding.
  - **Refined Sidebar Aesthetics & Horizontal Space**:
    - Expanded default sidebar width from 260px to 280px (with stretch range up to 320px) to drastically reduce ellipsis truncation for longer table identifiers.
    - Optimized tree indentation to `18px`, compacting quick action button clusters while elevating row hover and active selection styling with smooth `rounded_md` corners.
