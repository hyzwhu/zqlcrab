---
name: CrabStudio
colors:
  primary: "#0369A1"
  primary-hover: "#075985"
  primary-active: "#0C4A6E"
  on-primary: "#FFFFFF"
  secondary: "#475569"
  tertiary: "#6D28D9"
  neutral: "#0F172A"
  surface: "#1E293B"
  surface-hover: "#334155"
  surface-active: "#475569"
  on-surface: "#F8FAFC"
  on-surface-muted: "#CBD5E1"
  border: "#334155"
  border-focus: "#38BDF8"
  success: "#047857"
  on-success: "#FFFFFF"
  warning: "#B45309"
  on-warning: "#FFFFFF"
  error: "#B91C1C"
  on-error: "#FFFFFF"
typography:
  headline-lg:
    fontFamily: Inter, -apple-system, BlinkMacSystemFont, sans-serif
    fontSize: 1.25rem
    fontWeight: 600
    lineHeight: 1.75rem
  headline-md:
    fontFamily: Inter, -apple-system, BlinkMacSystemFont, sans-serif
    fontSize: 1rem
    fontWeight: 600
    lineHeight: 1.5rem
  body-md:
    fontFamily: Inter, -apple-system, BlinkMacSystemFont, sans-serif
    fontSize: 0.875rem
    fontWeight: 400
    lineHeight: 1.25rem
  body-sm:
    fontFamily: Inter, -apple-system, BlinkMacSystemFont, sans-serif
    fontSize: 0.75rem
    fontWeight: 400
    lineHeight: 1rem
  code:
    fontFamily: JetBrains Mono, Menlo, Monaco, monospace
    fontSize: 0.8125rem
    fontWeight: 400
    lineHeight: 1.25rem
rounded:
  none: 0px
  sm: 4px
  md: 6px
  lg: 8px
  full: 9999px
spacing:
  xxs: 2px
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  xxl: 32px
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.md}"
    padding: 8px
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.md}"
  button-primary-active:
    backgroundColor: "{colors.primary-active}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.md}"
  panel:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.lg}"
  panel-neutral:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.lg}"
  panel-hover:
    backgroundColor: "{colors.surface-hover}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.md}"
  table-cell:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.none}"
  table-cell-muted:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.on-surface-muted}"
    rounded: "{rounded.none}"
  status-badge:
    backgroundColor: "{colors.surface-active}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.full}"
  status-badge-success:
    backgroundColor: "{colors.success}"
    textColor: "{colors.on-success}"
    rounded: "{rounded.full}"
  status-badge-warning:
    backgroundColor: "{colors.warning}"
    textColor: "{colors.on-warning}"
    rounded: "{rounded.full}"
  status-badge-error:
    backgroundColor: "{colors.error}"
    textColor: "{colors.on-error}"
    rounded: "{rounded.full}"
  badge-secondary:
    backgroundColor: "{colors.secondary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.sm}"
  badge-tertiary:
    backgroundColor: "{colors.tertiary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.sm}"
  border-indicator:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.border-focus}"
    rounded: "{rounded.sm}"
  border-container:
    backgroundColor: "{colors.border}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.none}"
---

## Overview
CrabStudio is a refined, high-performance database client engineered with GPUI. Designed with dark-first technical clarity, it pairs an obsidian surface hierarchy with an electric ocean interaction driver. The interface prioritizes dense tabular readability, unambiguous connection state, and rapid keyboard-driven workflows across macOS, Windows, and Linux.

## Colors
- **Primary (`#0369A1`):** Electric Ocean serves as the focal interaction driver for query execution, active connection beacons, and primary actions.
- **Surface (`#1E293B`) & Neutral (`#0F172A`):** Deep slate-obsidian backdrops establishing spatial depth without harsh contrasts.
- **On-Surface (`#F8FAFC`) & Muted (`#CBD5E1`):** High legibility text tiers exceeding WCAG AA standards for long query and inspection sessions.
- **Semantic Accents:** Success (`#047857`) for connected status and query completions; Warning (`#B45309`) for slow transactions; Error (`#B91C1C`) for query execution faults.

## Typography
- **UI Headlines (`Inter`, 1rem - 1.25rem, semibold):** Crisp titles for database schemas, active connection headers, and modal dialogues.
- **Data & Body (`Inter`, 0.875rem):** Balanced density for properties, tree labels, and configuration inputs.
- **SQL & Query Results (`JetBrains Mono`, 0.8125rem):** Fixed-width clarity for SQL syntax and tabular cell contents.

## Layout
The window layout adheres to desktop ergonomics:
- **TitleBar:** Houses the application branding, active connection selector, and quick search.
- **Sidebar (Resizable):** Left-aligned collapsible tree displaying connections, schemas, tables, and views.
- **Workspace:** Tabbed container supporting SQL query consoles, visual data grids, and schema DDL inspectors.
- **StatusBar:** Bottom bar showing connection ping, active transaction state, row count metrics, and execution duration.

## Elevation & Depth
Elevation is rendered through border lines (`#334155`) and hairline separator tokens rather than fuzzy dropshadows, ensuring native performance and crisp pixel boundaries on high-DPI displays.

## Shapes
- **Controls & Buttons:** 6px radius (`rounded.md`) giving a tailored, modern finish.
- **Inputs & Modals:** 8px radius (`rounded.lg`) establishing structural encapsulation.
- **Table Grid Cells:** 0px radius (`rounded.none`) ensuring clean rectangular grid alignments.

## Components
- **Primary Buttons:** High contrast ocean blue action triggers for "Run Query" and "Save Connection".
- **Tree Items:** Compact vertical spacing with distinct hover (`surface-hover`) and selected (`surface-active`) states.
- **Data Grid:** Alternating row highlights, fixed header heights, and monospace value rendering.

## Do's and Don'ts
- **Do** preserve semantic tokens (`cx.theme()`) rather than hardcoding hex colors in view builders.
- **Do** show explicit loading and connection indicators for asynchronous operations.
- **Don't** clutter the screen with superfluous borders; use spatial padding and subtle tone differences.
- **Don't** hide query execution errors; display actionable diagnostics inline.
