//! VS Code-style left rail Activity Bar navigation component.

use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::gpui::{
    App, ElementId, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px,
};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivityNav {
    #[default]
    Databases,
    Console,
    Settings,
}

impl ActivityNav {
    pub fn is_settings(&self) -> bool {
        matches!(self, Self::Settings)
    }
}

#[derive(IntoElement)]
pub struct ActivityBar {
    active_nav: ActivityNav,
    language: AppLanguage,
    on_select_nav: Option<Rc<dyn Fn(ActivityNav, &mut Window, &mut App) + 'static>>,
}

impl ActivityBar {
    pub fn new(active_nav: ActivityNav, language: AppLanguage) -> Self {
        Self {
            active_nav,
            language,
            on_select_nav: None,
        }
    }

    pub fn on_select_nav<F>(mut self, handler: F) -> Self
    where
        F: Fn(ActivityNav, &mut Window, &mut App) + 'static,
    {
        self.on_select_nav = Some(Rc::new(handler));
        self
    }

    fn render_nav_item(
        &self,
        id: &'static str,
        nav: ActivityNav,
        icon: IconName,
        tooltip_key: &'static str,
    ) -> impl IntoElement {
        let is_active = self.active_nav == nav;
        let tooltip_text = t(tooltip_key, self.language);
        let on_select = self.on_select_nav.clone();

        let mut btn = Button::new(ElementId::Name(id.into()))
            .ghost()
            .icon(icon)
            .tooltip(tooltip_text);

        if let Some(handler) = on_select {
            btn = btn.on_click(move |_, window, cx| {
                handler(nav, window, cx);
            });
        }

        h_flex()
            .w_full()
            .h(px(40.0))
            .items_center()
            .relative()
            .bg(if is_active {
                ThemeColors::BG_SURFACE_ACTIVE
            } else {
                ThemeColors::TRANSPARENT
            })
            .child(
                div()
                    .w(px(3.0))
                    .h(px(22.0))
                    .rounded_r(px(2.0))
                    .bg(if is_active {
                        ThemeColors::PRIMARY_BORDER
                    } else {
                        ThemeColors::TRANSPARENT
                    }),
            )
            .child(
                h_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(btn),
            )
    }
}

impl RenderOnce for ActivityBar {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        let db_item = self.render_nav_item(
            "act_nav_db",
            ActivityNav::Databases,
            IconName::Database,
            "nav.databases",
        );
        let console_item = self.render_nav_item(
            "act_nav_console",
            ActivityNav::Console,
            IconName::Code,
            "nav.console",
        );
        let settings_item = self.render_nav_item(
            "act_nav_settings",
            ActivityNav::Settings,
            IconName::Settings,
            "nav.settings",
        );

        v_flex()
            .w(px(48.0))
            .h_full()
            .bg(ThemeColors::BG_SURFACE)
            .border_r_1()
            .border_color(ThemeColors::BORDER)
            .items_center()
            .justify_between()
            .py_1p5()
            .child(
                v_flex()
                    .w_full()
                    .items_center()
                    .gap_1()
                    .child(db_item)
                    .child(console_item),
            )
            .child(
                v_flex()
                    .w_full()
                    .items_center()
                    .child(settings_item),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_nav_defaults_and_predicates() {
        let default_nav = ActivityNav::default();
        assert_eq!(default_nav, ActivityNav::Databases);
        assert!(!default_nav.is_settings());
        assert!(ActivityNav::Settings.is_settings());
        assert_ne!(ActivityNav::Databases, ActivityNav::Console);
    }
}
