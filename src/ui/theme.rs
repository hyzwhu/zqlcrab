//! Theme definitions and visual tokens for the database client, aligned with DESIGN.md.

use gpui_kit::gpui::{Background, Fill, Hsla, Rgba, hsla, transparent_black};
use std::ops::Deref;
use std::sync::atomic::{AtomicU8, Ordering};

/// 0 = Dark, 1 = Light
static ACTIVE_THEME_MODE: AtomicU8 = AtomicU8::new(0);

/// Sets the active theme mode across the application.
pub fn set_active_theme_mode(is_light: bool) {
    ACTIVE_THEME_MODE.store(if is_light { 1 } else { 0 }, Ordering::SeqCst);
}

/// Returns whether the active theme mode is Light.
pub fn is_light_theme() -> bool {
    ACTIVE_THEME_MODE.load(Ordering::Relaxed) == 1
}

/// A reactive theme token dynamically resolving to Dark or Light colors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeToken {
    pub dark: Hsla,
    pub light: Hsla,
}

impl ThemeToken {
    pub const fn new(dark: Hsla, light: Hsla) -> Self {
        Self { dark, light }
    }

    #[inline(always)]
    pub fn get(&self) -> Hsla {
        if is_light_theme() {
            self.light
        } else {
            self.dark
        }
    }

    #[inline(always)]
    pub fn opacity(&self, alpha: f32) -> Hsla {
        self.get().opacity(alpha)
    }
}

impl Deref for ThemeToken {
    type Target = Hsla;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        if is_light_theme() {
            &self.light
        } else {
            &self.dark
        }
    }
}

impl From<ThemeToken> for Hsla {
    #[inline(always)]
    fn from(token: ThemeToken) -> Self {
        token.get()
    }
}

impl From<ThemeToken> for Fill {
    #[inline(always)]
    fn from(token: ThemeToken) -> Self {
        Fill::Color(token.get().into())
    }
}

impl From<ThemeToken> for Background {
    #[inline(always)]
    fn from(token: ThemeToken) -> Self {
        token.get().into()
    }
}

impl From<Hsla> for ThemeToken {
    #[inline(always)]
    fn from(h: Hsla) -> Self {
        Self { dark: h, light: h }
    }
}

impl From<Rgba> for ThemeToken {
    #[inline(always)]
    fn from(r: Rgba) -> Self {
        let h: Hsla = r.into();
        Self { dark: h, light: h }
    }
}

pub struct ThemeColors;

impl ThemeColors {
    pub const TRANSPARENT: ThemeToken = ThemeToken::new(
        transparent_black(),
        transparent_black(),
    );
    // Primary / Accent
    pub const PRIMARY: ThemeToken = ThemeToken::new(
        hsla(201.0 / 360.0, 0.96, 0.32, 1.0), // Dark: #0369A1
        hsla(201.0 / 360.0, 0.96, 0.32, 1.0), // Light: #0369A1
    );
    pub const PRIMARY_LIGHT: ThemeToken = ThemeToken::new(
        hsla(199.0 / 360.0, 0.89, 0.48, 1.0), // Dark: #0EA5E9
        hsla(199.0 / 360.0, 0.89, 0.48, 1.0), // Light: #0EA5E9
    );
    pub const PRIMARY_BORDER: ThemeToken = ThemeToken::new(
        hsla(199.0 / 360.0, 0.95, 0.60, 1.0), // Dark: #38BDF8
        hsla(201.0 / 360.0, 0.90, 0.42, 1.0), // Light: #0284C7 (sharp contrast)
    );
    pub const PRIMARY_BG: ThemeToken = ThemeToken::new(
        hsla(201.0 / 360.0, 0.96, 0.32, 0.25), // Dark: translucent
        hsla(201.0 / 360.0, 0.96, 0.32, 0.12), // Light: soft translucent blue
    );

    // Background surfaces
    pub const BG_APP: ThemeToken = ThemeToken::new(
        hsla(222.0 / 360.0, 0.47, 0.11, 1.0), // Dark: #0F172A (Obsidian/Slate)
        hsla(210.0 / 360.0, 0.20, 0.96, 1.0), // Light: #F1F5F9 (Clean cool grey)
    );
    pub const BG_SURFACE: ThemeToken = ThemeToken::new(
        hsla(217.0 / 360.0, 0.33, 0.17, 1.0), // Dark: #1E293B
        hsla(0.0, 0.0, 1.0, 1.0),             // Light: #FFFFFF (Pure white surface)
    );
    pub const BG_SURFACE_HOVER: ThemeToken = ThemeToken::new(
        hsla(215.0 / 360.0, 0.25, 0.27, 1.0), // Dark: #334155
        hsla(210.0 / 360.0, 0.20, 0.93, 1.0), // Light: #E2E8F0
    );
    pub const BG_SURFACE_ACTIVE: ThemeToken = ThemeToken::new(
        hsla(215.0 / 360.0, 0.19, 0.35, 1.0), // Dark: #475569
        hsla(214.0 / 360.0, 0.20, 0.88, 1.0), // Light: #CBD5E1
    );

    // Text tiers
    pub const TEXT_PRIMARY: ThemeToken = ThemeToken::new(
        hsla(210.0 / 360.0, 0.40, 0.98, 1.0), // Dark: #F8FAFC
        hsla(222.0 / 360.0, 0.47, 0.11, 1.0), // Light: #0F172A (Dark deep slate)
    );
    pub const TEXT_MUTED: ThemeToken = ThemeToken::new(
        hsla(214.0 / 360.0, 0.32, 0.84, 1.0), // Dark: #CBD5E1
        hsla(215.0 / 360.0, 0.22, 0.35, 1.0), // Light: #475569
    );
    pub const TEXT_FAINT: ThemeToken = ThemeToken::new(
        hsla(215.0 / 360.0, 0.16, 0.65, 1.0), // Dark: #94A3B8
        hsla(215.0 / 360.0, 0.16, 0.55, 1.0), // Light: #64748B
    );

    // Borders and dividers
    pub const BORDER: ThemeToken = ThemeToken::new(
        hsla(215.0 / 360.0, 0.25, 0.27, 1.0), // Dark: #334155
        hsla(214.0 / 360.0, 0.32, 0.88, 1.0), // Light: #E2E8F0
    );
    pub const BORDER_LIGHT: ThemeToken = ThemeToken::new(
        hsla(215.0 / 360.0, 0.19, 0.35, 0.7),
        hsla(210.0 / 360.0, 0.20, 0.92, 1.0), // Light: #F1F5F9
    );
    pub const BORDER_PROMINENT: ThemeToken = ThemeToken::new(
        hsla(215.0 / 360.0, 0.20, 0.45, 1.0), // Dark: #475569
        hsla(215.0 / 360.0, 0.20, 0.75, 1.0), // Light: #94A3B8
    );

    // Status indicators
    pub const SUCCESS: ThemeToken = ThemeToken::new(
        hsla(160.0 / 360.0, 0.84, 0.39, 1.0), // #10B981
        hsla(160.0 / 360.0, 0.84, 0.35, 1.0), // #059669
    );
    pub const WARNING: ThemeToken = ThemeToken::new(
        hsla(38.0 / 360.0, 0.92, 0.50, 1.0), // #F59E0B
        hsla(38.0 / 360.0, 0.92, 0.42, 1.0), // #D97706
    );
    pub const ERROR: ThemeToken = ThemeToken::new(
        hsla(0.0 / 360.0, 0.84, 0.60, 1.0), // #EF4444
        hsla(0.0 / 360.0, 0.84, 0.50, 1.0), // #DC2626
    );
}
