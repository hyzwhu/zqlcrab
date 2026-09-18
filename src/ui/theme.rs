//! Theme definitions and visual tokens for the database client, aligned with DESIGN.md.

use gpui_kit::gpui::{Hsla, hsla};

pub struct ThemeColors;

impl ThemeColors {
    // Primary / Accent
    pub const PRIMARY: Hsla = hsla(201.0 / 360.0, 0.96, 0.32, 1.0); // #0369A1
    pub const PRIMARY_LIGHT: Hsla = hsla(199.0 / 360.0, 0.89, 0.48, 1.0); // #0EA5E9
    pub const PRIMARY_BORDER: Hsla = hsla(199.0 / 360.0, 0.95, 0.60, 1.0); // #38BDF8
    pub const PRIMARY_BG: Hsla = hsla(201.0 / 360.0, 0.96, 0.32, 0.25); // translucent selection bg

    // Background surfaces (Obsidian / Slate)
    pub const BG_APP: Hsla = hsla(222.0 / 360.0, 0.47, 0.11, 1.0); // #0F172A
    pub const BG_SURFACE: Hsla = hsla(217.0 / 360.0, 0.33, 0.17, 1.0); // #1E293B
    pub const BG_SURFACE_HOVER: Hsla = hsla(215.0 / 360.0, 0.25, 0.27, 1.0); // #334155
    pub const BG_SURFACE_ACTIVE: Hsla = hsla(215.0 / 360.0, 0.19, 0.35, 1.0); // #475569

    // Text tiers
    pub const TEXT_PRIMARY: Hsla = hsla(210.0 / 360.0, 0.40, 0.98, 1.0); // #F8FAFC
    pub const TEXT_MUTED: Hsla = hsla(214.0 / 360.0, 0.32, 0.84, 1.0); // #CBD5E1
    pub const TEXT_FAINT: Hsla = hsla(215.0 / 360.0, 0.16, 0.65, 1.0); // #94A3B8

    // Borders and dividers
    pub const BORDER: Hsla = hsla(215.0 / 360.0, 0.25, 0.27, 1.0); // #334155
    pub const BORDER_LIGHT: Hsla = hsla(215.0 / 360.0, 0.19, 0.35, 0.7);
    pub const BORDER_PROMINENT: Hsla = hsla(215.0 / 360.0, 0.20, 0.45, 1.0); // sharp prominent divider

    // Status indicators
    pub const SUCCESS: Hsla = hsla(160.0 / 360.0, 0.84, 0.39, 1.0); // #10B981
    pub const WARNING: Hsla = hsla(38.0 / 360.0, 0.92, 0.50, 1.0); // #F59E0B
    pub const ERROR: Hsla = hsla(0.0 / 360.0, 0.84, 0.60, 1.0); // #EF4444
}
