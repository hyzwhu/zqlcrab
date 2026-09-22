//! Settings view component providing application preferences, theme selection, and language switching.

use crate::settings::{AppLanguage, AppSettings, ThemePreference};
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::gpui::{
    App, ElementId, FontWeight, Image, ImageFormat, InteractiveElement as _, IntoElement,
    ParentElement, RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, hsla, img,
    prelude::FluentBuilder as _, px,
};
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Appearance,
    Editor,
    Query,
    Language,
    About,
}

#[derive(IntoElement)]
pub struct SettingsView {
    settings: AppSettings,
    active_tab: SettingsTab,
    is_checking_update: bool,
    update_status_msg: Option<String>,
    update_result: Option<crate::update::UpdateCheckResult>,
    on_change_settings:
        Option<Rc<dyn Fn(Box<dyn FnOnce(&mut AppSettings)>, &mut Window, &mut App) + 'static>>,
    on_reset_defaults: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_check_updates: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_open_release_url: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_tab: Option<Rc<dyn Fn(SettingsTab, &mut Window, &mut App) + 'static>>,
}

impl SettingsView {
    pub fn new(settings: AppSettings, active_tab: SettingsTab) -> Self {
        Self {
            settings,
            active_tab,
            is_checking_update: false,
            update_status_msg: None,
            update_result: None,
            on_change_settings: None,
            on_reset_defaults: None,
            on_check_updates: None,
            on_open_release_url: None,
            on_select_tab: None,
        }
    }

    pub fn checking_update(mut self, checking: bool) -> Self {
        self.is_checking_update = checking;
        self
    }

    pub fn update_status_msg(mut self, msg: Option<String>) -> Self {
        self.update_status_msg = msg;
        self
    }

    pub fn update_result(mut self, result: Option<crate::update::UpdateCheckResult>) -> Self {
        self.update_result = result;
        self
    }

    pub fn on_open_release_url<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_open_release_url = Some(Rc::new(handler));
        self
    }

    pub fn on_change_settings<F>(mut self, handler: F) -> Self
    where
        F: Fn(Box<dyn FnOnce(&mut AppSettings)>, &mut Window, &mut App) + 'static,
    {
        self.on_change_settings = Some(Rc::new(handler));
        self
    }

    pub fn on_reset_defaults<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_reset_defaults = Some(Rc::new(handler));
        self
    }

    pub fn on_check_updates<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_check_updates = Some(Rc::new(handler));
        self
    }

    pub fn on_select_tab<F>(mut self, handler: F) -> Self
    where
        F: Fn(SettingsTab, &mut Window, &mut App) + 'static,
    {
        self.on_select_tab = Some(Rc::new(handler));
        self
    }

    fn render_header(&self) -> impl IntoElement {
        let lang = self.settings.language;
        let on_reset = self.on_reset_defaults.clone();

        let mut reset_btn = Button::new("btn_reset_defaults")
            .ghost()
            .small()
            .icon(IconName::RotateCcw)
            .label(t("settings.reset", lang));

        if let Some(handler) = on_reset {
            reset_btn = reset_btn.on_click(move |_, window, cx| {
                handler(window, cx);
            });
        }

        h_flex()
            .h(px(52.0))
            .w_full()
            .px_6()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t("settings.title", lang)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child("|"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("settings.subtitle", lang)),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(ThemeColors::BG_APP)
                            .child(
                                Icon::new(IconName::Check)
                                    .size(px(12.0))
                                    .text_color(ThemeColors::SUCCESS),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_FAINT)
                                    .child(t("settings.saved", lang)),
                            ),
                    )
                    .child(reset_btn),
            )
    }

    fn render_tab_bar(&self) -> impl IntoElement {
        let lang = self.settings.language;
        let tabs = [
            (SettingsTab::Appearance, "tab.appearance", IconName::Palette),
            (SettingsTab::Editor, "tab.editor", IconName::Code),
            (SettingsTab::Query, "tab.query", IconName::Database),
            (SettingsTab::Language, "tab.language", IconName::Languages),
            (SettingsTab::About, "tab.about", IconName::Info),
        ];

        let mut tab_row = h_flex()
            .h(px(40.0))
            .w_full()
            .px_6()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE);

        for (tab, label_key, icon) in tabs {
            let is_active = self.active_tab == tab;
            let on_select = self.on_select_tab.clone();
            let label = t(label_key, lang);
            let tab_id = format!("settings_tab_{:?}", tab);

            let mut tab_btn = Button::new(ElementId::Name(tab_id.into()))
                .ghost()
                .small()
                .icon(icon)
                .label(label)
                .border_b_2()
                .border_color(if is_active {
                    ThemeColors::PRIMARY_BORDER
                } else {
                    ThemeColors::TRANSPARENT
                });

            if is_active {
                tab_btn = tab_btn.primary();
            }

            if let Some(handler) = on_select {
                tab_btn = tab_btn.on_click(move |_, window, cx| {
                    handler(tab, window, cx);
                });
            }

            tab_row = tab_row.child(tab_btn);
        }

        tab_row
    }

    fn render_theme_card(
        &self,
        theme: ThemePreference,
        title_key: &'static str,
        desc_key: &'static str,
    ) -> impl IntoElement {
        let lang = self.settings.language;
        let is_selected = self.settings.appearance.theme == theme;
        let on_change = self.on_change_settings.clone();

        let card_id = format!("theme_card_{:?}", theme);

        // Graphic preview mockup for each theme
        let preview_window = match theme {
            ThemePreference::System => {
                // Split preview: left half dark, right half light
                h_flex()
                    .w_full()
                    .h(px(76.0))
                    .rounded_t_md()
                    .overflow_hidden()
                    .border_b_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        // Dark side
                        h_flex()
                            .w_1_2()
                            .h_full()
                            .bg(hsla(222.0 / 360.0, 0.47, 0.11, 1.0))
                            .child(div().w(px(16.0)).h_full().bg(hsla(
                                217.0 / 360.0,
                                0.33,
                                0.17,
                                1.0,
                            )))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .p_2()
                                    .gap_1()
                                    .child(
                                        div()
                                            .w_3_4()
                                            .h(px(4.0))
                                            .bg(hsla(199.0 / 360.0, 0.95, 0.60, 1.0))
                                            .rounded_xs(),
                                    )
                                    .child(
                                        div()
                                            .w_1_2()
                                            .h(px(4.0))
                                            .bg(hsla(215.0 / 360.0, 0.16, 0.65, 0.5))
                                            .rounded_xs(),
                                    )
                                    .child(
                                        div()
                                            .w_5_6()
                                            .h(px(4.0))
                                            .bg(hsla(215.0 / 360.0, 0.16, 0.65, 0.5))
                                            .rounded_xs(),
                                    ),
                            ),
                    )
                    .child(
                        // Light side
                        h_flex()
                            .w_1_2()
                            .h_full()
                            .bg(hsla(0.0, 0.0, 1.0, 1.0))
                            .child(div().w(px(16.0)).h_full().bg(hsla(
                                210.0 / 360.0,
                                0.20,
                                0.94,
                                1.0,
                            )))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .p_2()
                                    .gap_1()
                                    .child(
                                        div()
                                            .w_3_4()
                                            .h(px(4.0))
                                            .bg(hsla(201.0 / 360.0, 0.96, 0.32, 1.0))
                                            .rounded_xs(),
                                    )
                                    .child(
                                        div()
                                            .w_1_2()
                                            .h(px(4.0))
                                            .bg(hsla(215.0 / 360.0, 0.16, 0.40, 0.4))
                                            .rounded_xs(),
                                    )
                                    .child(
                                        div()
                                            .w_5_6()
                                            .h(px(4.0))
                                            .bg(hsla(215.0 / 360.0, 0.16, 0.40, 0.4))
                                            .rounded_xs(),
                                    ),
                            ),
                    )
            }
            ThemePreference::Dark => {
                // Obsidian Slate dark preview
                h_flex()
                    .w_full()
                    .h(px(76.0))
                    .rounded_t_md()
                    .overflow_hidden()
                    .border_b_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(hsla(222.0 / 360.0, 0.47, 0.11, 1.0))
                    .child(
                        div()
                            .w(px(28.0))
                            .h_full()
                            .bg(hsla(217.0 / 360.0, 0.33, 0.17, 1.0))
                            .p_1p5()
                            .child(
                                div()
                                    .w_full()
                                    .h(px(3.0))
                                    .bg(hsla(199.0 / 360.0, 0.95, 0.60, 0.6))
                                    .rounded_xs(),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .p_2()
                            .gap_1p5()
                            .child(
                                div()
                                    .w_2_3()
                                    .h(px(4.0))
                                    .bg(hsla(199.0 / 360.0, 0.95, 0.60, 1.0))
                                    .rounded_xs(),
                            )
                            .child(
                                div()
                                    .w_4_5()
                                    .h(px(4.0))
                                    .bg(hsla(160.0 / 360.0, 0.84, 0.39, 0.8))
                                    .rounded_xs(),
                            )
                            .child(
                                div()
                                    .w_1_2()
                                    .h(px(4.0))
                                    .bg(hsla(215.0 / 360.0, 0.16, 0.65, 0.5))
                                    .rounded_xs(),
                            ),
                    )
            }
            ThemePreference::Light => {
                // Clean High Contrast light preview
                h_flex()
                    .w_full()
                    .h(px(76.0))
                    .rounded_t_md()
                    .overflow_hidden()
                    .border_b_1()
                    .border_color(hsla(215.0 / 360.0, 0.20, 0.85, 1.0))
                    .bg(hsla(0.0, 0.0, 1.0, 1.0))
                    .child(
                        div()
                            .w(px(28.0))
                            .h_full()
                            .bg(hsla(210.0 / 360.0, 0.20, 0.94, 1.0))
                            .p_1p5()
                            .child(
                                div()
                                    .w_full()
                                    .h(px(3.0))
                                    .bg(hsla(201.0 / 360.0, 0.96, 0.32, 0.8))
                                    .rounded_xs(),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .p_2()
                            .gap_1p5()
                            .child(
                                div()
                                    .w_2_3()
                                    .h(px(4.0))
                                    .bg(hsla(201.0 / 360.0, 0.96, 0.32, 1.0))
                                    .rounded_xs(),
                            )
                            .child(
                                div()
                                    .w_4_5()
                                    .h(px(4.0))
                                    .bg(hsla(160.0 / 360.0, 0.84, 0.35, 0.9))
                                    .rounded_xs(),
                            )
                            .child(
                                div()
                                    .w_1_2()
                                    .h(px(4.0))
                                    .bg(hsla(215.0 / 360.0, 0.16, 0.40, 0.4))
                                    .rounded_xs(),
                            ),
                    )
            }
        };

        v_flex()
            .id(ElementId::Name(card_id.into()))
            .w(px(210.0))
            .rounded_lg()
            .border_2()
            .border_color(if is_selected {
                ThemeColors::PRIMARY_BORDER
            } else {
                ThemeColors::BORDER
            })
            .bg(ThemeColors::BG_SURFACE)
            .cursor_pointer()
            .relative()
            .hover(|s| {
                if !is_selected {
                    s.border_color(ThemeColors::BORDER_PROMINENT)
                } else {
                    s
                }
            })
            .child(preview_window)
            .child(
                h_flex()
                    .w_full()
                    .p_3()
                    .items_center()
                    .justify_between()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t(title_key, lang)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_FAINT)
                                    .child(t(desc_key, lang)),
                            ),
                    )
                    .when(is_selected, |this| {
                        this.child(
                            div()
                                .w(px(20.0))
                                .h(px(20.0))
                                .rounded_full()
                                .bg(ThemeColors::PRIMARY_BG)
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(IconName::Check)
                                        .size(px(13.0))
                                        .text_color(ThemeColors::PRIMARY_BORDER),
                                ),
                        )
                    }),
            )
            .on_click(move |_, window, cx| {
                if let Some(ref handler) = on_change {
                    handler(
                        Box::new(move |s: &mut AppSettings| {
                            s.appearance.theme = theme;
                        }),
                        window,
                        cx,
                    );
                }
            })
    }

    fn render_appearance_tab(&self) -> impl IntoElement {
        let lang = self.settings.language;
        let on_check = self.on_check_updates.clone();
        let on_change = self.on_change_settings.clone();
        let is_checking = self.is_checking_update;

        let mut check_btn = Button::new("btn_check_updates")
            .small()
            .icon(if is_checking {
                IconName::RotateCw
            } else {
                IconName::RefreshCw
            })
            .label(t("appearance.check_now", lang))
            .disabled(is_checking);

        if is_checking {
            check_btn = check_btn.outline();
        } else {
            check_btn = check_btn.primary();
        }

        if let Some(handler) = on_check {
            check_btn = check_btn.on_click(move |_, window, cx| {
                handler(window, cx);
            });
        }

        let (status_node, action_buttons, new_ver_badge) = if is_checking {
            (
                div()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child(t("appearance.checking", lang)),
                check_btn.into_any_element(),
                None,
            )
        } else if let Some(ref res) = self.update_result {
            match res {
                crate::update::UpdateCheckResult::NewVersionAvailable {
                    current_version,
                    latest_version,
                    release_url,
                    ..
                } => {
                    let on_open = self.on_open_release_url.clone();
                    let url_clone = release_url.clone();
                    let view_btn = Button::new("btn_view_release")
                        .primary()
                        .small()
                        .icon(IconName::ExternalLink)
                        .label(format!(
                            "{} v{}",
                            t("appearance.view_release", lang),
                            latest_version
                        ))
                        .on_click(move |_, window, cx| {
                            if let Some(ref handler) = on_open {
                                handler(url_clone.clone(), window, cx);
                            } else {
                                cx.open_url(&url_clone);
                            }
                        });

                    let badge = div()
                        .flex_shrink_0()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(ThemeColors::SUCCESS)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(format!("NEW v{latest_version}"));

                    let msg = format!(
                        "🎉 {} v{} (Current: v{})",
                        t("appearance.new_version", lang),
                        latest_version,
                        current_version
                    );

                    (
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::SUCCESS)
                            .child(msg),
                        h_flex()
                            .gap_2()
                            .child(view_btn)
                            .child(check_btn)
                            .into_any_element(),
                        Some(badge),
                    )
                }
                crate::update::UpdateCheckResult::UpToDate {
                    current_version,
                    checked_time,
                    ..
                } => {
                    let msg = format!(
                        "✓ {} (v{}) · {} {}",
                        t("appearance.status_latest", lang),
                        current_version,
                        t("console.time", lang),
                        checked_time
                    );
                    (
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(msg),
                        check_btn.into_any_element(),
                        None,
                    )
                }
                crate::update::UpdateCheckResult::Failed { error } => (
                    div()
                        .text_xs()
                        .text_color(ThemeColors::ERROR)
                        .child(format!("⚠️ {error}")),
                    check_btn.into_any_element(),
                    None,
                ),
            }
        } else if let Some(ref msg) = self.update_status_msg {
            (
                div()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child(msg.clone()),
                check_btn.into_any_element(),
                None,
            )
        } else {
            (
                div()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child(t("appearance.updates_initial", lang)),
                check_btn.into_any_element(),
                None,
            )
        };

        v_flex()
            .w_full()
            .gap_6()
            // Theme Section
            .child(
                v_flex()
                    .w_full()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t("appearance.theme", lang)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("appearance.theme_desc", lang)),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .items_stretch()
                            .gap_4()
                            .child(self.render_theme_card(
                                ThemePreference::System,
                                "appearance.system",
                                "appearance.system_desc",
                            ))
                            .child(self.render_theme_card(
                                ThemePreference::Dark,
                                "appearance.dark",
                                "appearance.dark_desc",
                            ))
                            .child(self.render_theme_card(
                                ThemePreference::Light,
                                "appearance.light",
                                "appearance.light_desc",
                            )),
                    ),
            )
            // Activity Bar & Status Bar Layout Toggle
            .child(self.render_toggle_row(
                "appearance.show_activity_bar",
                "appearance.show_activity_bar_desc",
                self.settings.appearance.show_activity_bar,
                {
                    let on_ch = on_change.clone();
                    let curr = self.settings.appearance.show_activity_bar;
                    move |window, cx| {
                        if let Some(ref handler) = on_ch {
                            handler(
                                Box::new(move |s| {
                                    s.appearance.show_activity_bar = !curr;
                                }),
                                window,
                                cx,
                            );
                        }
                    }
                },
            ))
            .child(self.render_toggle_row(
                "appearance.show_status_bar",
                "appearance.show_status_bar_desc",
                self.settings.appearance.show_status_bar,
                {
                    let on_ch = on_change.clone();
                    let curr = self.settings.appearance.show_status_bar;
                    move |window, cx| {
                        if let Some(ref handler) = on_ch {
                            handler(
                                Box::new(move |s| {
                                    s.appearance.show_status_bar = !curr;
                                }),
                                window,
                                cx,
                            );
                        }
                    }
                },
            ))
            // Check for Updates Section
            .child(
                v_flex()
                    .w_full()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t("appearance.updates", lang)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("appearance.updates_desc", lang)),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .p_4()
                            .rounded_lg()
                            .bg(ThemeColors::BG_SURFACE)
                            .border_1()
                            .border_color(ThemeColors::BORDER)
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_3()
                                    .child({
                                        let logo_img = Arc::new(Image {
                                            format: ImageFormat::Png,
                                            bytes: crate::ui::app::LOGO_PNG_BYTES.to_vec(),
                                            id: 0x7a716c63726162,
                                        });
                                        div()
                                            .size(px(40.0))
                                            .rounded_lg()
                                            .overflow_hidden()
                                            .flex_shrink_0()
                                            .child(img(logo_img).size(px(40.0)).rounded_lg())
                                    })
                                    .child(
                                        v_flex()
                                            .gap_0p5()
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .flex_shrink_0()
                                                            .text_sm()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                                            .child("zqlcrab"),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_shrink_0()
                                                            .px_1p5()
                                                            .py_0p5()
                                                            .rounded_sm()
                                                            .bg(ThemeColors::BG_APP)
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(ThemeColors::PRIMARY_BORDER)
                                                            .child(format!(
                                                                "v{}",
                                                                env!("CARGO_PKG_VERSION")
                                                            )),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_shrink_0()
                                                            .px_2()
                                                            .py_0p5()
                                                            .rounded_sm()
                                                            .bg(ThemeColors::BG_APP)
                                                            .text_xs()
                                                            .text_color(ThemeColors::TEXT_MUTED)
                                                            .child(format!(
                                                                "{}-{}",
                                                                std::env::consts::OS,
                                                                std::env::consts::ARCH
                                                            )),
                                                    )
                                                    .children(new_ver_badge),
                                            )
                                            .child(status_node),
                                    ),
                            )
                            .child(action_buttons),
                    ),
            )
    }

    fn render_editor_tab(&self) -> impl IntoElement {
        let ed = &self.settings.editor;
        let on_change = self.on_change_settings.clone();

        let fonts = [
            "JetBrains Mono",
            "Fira Code",
            "Menlo",
            "SF Mono",
            "Monospace",
        ];
        let font_sizes: [f32; 5] = [11.0, 12.0, 13.0, 14.0, 16.0];
        let tab_sizes = [2, 4, 8];

        v_flex()
            .w_full()
            .gap_4()
            // Font family row
            .child(
                self.render_preference_row("editor.font_family", "editor.font_family_desc", {
                    let mut font_row = h_flex().items_center().gap_1();
                    for f in fonts {
                        let is_curr = ed.font_family == f;
                        let on_ch = on_change.clone();
                        let f_str = f.to_string();
                        let mut btn = Button::new(ElementId::Name(format!("font_{}", f).into()))
                            .small()
                            .label(f);
                        if is_curr {
                            btn = btn.primary();
                        } else {
                            btn = btn.outline();
                        }
                        btn = btn.on_click(move |_, window, cx| {
                            if let Some(ref handler) = on_ch {
                                let f_val = f_str.clone();
                                handler(
                                    Box::new(move |s| {
                                        s.editor.font_family = f_val;
                                    }),
                                    window,
                                    cx,
                                );
                            }
                        });
                        font_row = font_row.child(btn);
                    }
                    font_row
                }),
            )
            // Font size row
            .child(
                self.render_preference_row("editor.font_size", "editor.font_size_desc", {
                    let mut size_row = h_flex().items_center().gap_1();
                    for sz in font_sizes {
                        let is_curr = (ed.font_size - sz).abs() < f32::EPSILON;
                        let on_ch = on_change.clone();
                        let label = format!("{sz:.0}px");
                        let mut btn = Button::new(ElementId::Name(format!("size_{}", sz).into()))
                            .small()
                            .label(label);
                        if is_curr {
                            btn = btn.primary();
                        } else {
                            btn = btn.outline();
                        }
                        btn = btn.on_click(move |_, window, cx| {
                            if let Some(ref handler) = on_ch {
                                handler(
                                    Box::new(move |s| {
                                        s.editor.font_size = sz;
                                    }),
                                    window,
                                    cx,
                                );
                            }
                        });
                        size_row = size_row.child(btn);
                    }
                    size_row
                }),
            )
            // Tab size row
            .child(
                self.render_preference_row("editor.tab_size", "editor.tab_size_desc", {
                    let mut tab_row = h_flex().items_center().gap_1();
                    for ts in tab_sizes {
                        let is_curr = ed.tab_size == ts;
                        let on_ch = on_change.clone();
                        let label = format!("{ts} spaces");
                        let mut btn =
                            Button::new(ElementId::Name(format!("tabsize_{}", ts).into()))
                                .small()
                                .label(label);
                        if is_curr {
                            btn = btn.primary();
                        } else {
                            btn = btn.outline();
                        }
                        btn = btn.on_click(move |_, window, cx| {
                            if let Some(ref handler) = on_ch {
                                handler(
                                    Box::new(move |s| {
                                        s.editor.tab_size = ts;
                                    }),
                                    window,
                                    cx,
                                );
                            }
                        });
                        tab_row = tab_row.child(btn);
                    }
                    tab_row
                }),
            )
            // Line numbers toggle
            .child(self.render_toggle_row(
                "editor.line_numbers",
                "editor.line_numbers_desc",
                ed.line_numbers,
                {
                    let on_ch = on_change.clone();
                    let curr = ed.line_numbers;
                    move |window, cx| {
                        if let Some(ref handler) = on_ch {
                            handler(
                                Box::new(move |s| {
                                    s.editor.line_numbers = !curr;
                                }),
                                window,
                                cx,
                            );
                        }
                    }
                },
            ))
            // Word wrap toggle
            .child(self.render_toggle_row(
                "editor.word_wrap",
                "editor.word_wrap_desc",
                ed.word_wrap,
                {
                    let on_ch = on_change.clone();
                    let curr = ed.word_wrap;
                    move |window, cx| {
                        if let Some(ref handler) = on_ch {
                            handler(
                                Box::new(move |s| {
                                    s.editor.word_wrap = !curr;
                                }),
                                window,
                                cx,
                            );
                        }
                    }
                },
            ))
            // Format on run toggle
            .child(self.render_toggle_row(
                "editor.format_on_run",
                "editor.format_on_run_desc",
                ed.format_on_run,
                {
                    let on_ch = on_change.clone();
                    let curr = ed.format_on_run;
                    move |window, cx| {
                        if let Some(ref handler) = on_ch {
                            handler(
                                Box::new(move |s| {
                                    s.editor.format_on_run = !curr;
                                }),
                                window,
                                cx,
                            );
                        }
                    }
                },
            ))
            // Bracket matching toggle
            .child(self.render_toggle_row(
                "editor.bracket_matching",
                "editor.bracket_matching_desc",
                ed.bracket_matching,
                {
                    let on_ch = on_change.clone();
                    let curr = ed.bracket_matching;
                    move |window, cx| {
                        if let Some(ref handler) = on_ch {
                            handler(
                                Box::new(move |s| {
                                    s.editor.bracket_matching = !curr;
                                }),
                                window,
                                cx,
                            );
                        }
                    }
                },
            ))
    }

    fn render_query_tab(&self) -> impl IntoElement {
        let q = &self.settings.query;
        let on_change = self.on_change_settings.clone();

        let limits = [100, 500, 1000, 5000, 10000];
        let timeouts = [10, 30, 60, 120];
        let hist_limits = [100, 250, 500, 1000];

        v_flex()
            .w_full()
            .gap_4()
            // Default row limit
            .child(
                self.render_preference_row("query.default_limit", "query.default_limit_desc", {
                    let mut row = h_flex().items_center().gap_1();
                    for l in limits {
                        let is_curr = q.default_limit == l;
                        let on_ch = on_change.clone();
                        let label = format!("{l}");
                        let mut btn = Button::new(ElementId::Name(format!("limit_{}", l).into()))
                            .small()
                            .label(label);
                        if is_curr {
                            btn = btn.primary();
                        } else {
                            btn = btn.outline();
                        }
                        btn = btn.on_click(move |_, window, cx| {
                            if let Some(ref handler) = on_ch {
                                handler(
                                    Box::new(move |s| {
                                        s.query.default_limit = l;
                                    }),
                                    window,
                                    cx,
                                );
                            }
                        });
                        row = row.child(btn);
                    }
                    row
                }),
            )
            // Query timeout
            .child(
                self.render_preference_row("query.timeout", "query.timeout_desc", {
                    let mut row = h_flex().items_center().gap_1();
                    for t_sec in timeouts {
                        let is_curr = q.query_timeout_secs == t_sec;
                        let on_ch = on_change.clone();
                        let label = format!("{t_sec}s");
                        let mut btn = Button::new(ElementId::Name(format!("to_{}", t_sec).into()))
                            .small()
                            .label(label);
                        if is_curr {
                            btn = btn.primary();
                        } else {
                            btn = btn.outline();
                        }
                        btn = btn.on_click(move |_, window, cx| {
                            if let Some(ref handler) = on_ch {
                                handler(
                                    Box::new(move |s| {
                                        s.query.query_timeout_secs = t_sec;
                                    }),
                                    window,
                                    cx,
                                );
                            }
                        });
                        row = row.child(btn);
                    }
                    row
                }),
            )
            // Safe mode toggle
            .child(self.render_toggle_row(
                "query.safe_mode",
                "query.safe_mode_desc",
                q.safe_mode,
                {
                    let on_ch = on_change.clone();
                    let curr = q.safe_mode;
                    move |window, cx| {
                        if let Some(ref handler) = on_ch {
                            handler(
                                Box::new(move |s| {
                                    s.query.safe_mode = !curr;
                                }),
                                window,
                                cx,
                            );
                        }
                    }
                },
            ))
            // Auto explain toggle
            .child(self.render_toggle_row(
                "query.auto_explain",
                "query.auto_explain_desc",
                q.auto_explain_slow,
                {
                    let on_ch = on_change.clone();
                    let curr = q.auto_explain_slow;
                    move |window, cx| {
                        if let Some(ref handler) = on_ch {
                            handler(
                                Box::new(move |s| {
                                    s.query.auto_explain_slow = !curr;
                                }),
                                window,
                                cx,
                            );
                        }
                    }
                },
            ))
            // History retention limit
            .child(
                self.render_preference_row("query.history_limit", "query.history_limit_desc", {
                    let mut row = h_flex().items_center().gap_1();
                    for hl in hist_limits {
                        let is_curr = q.history_limit == hl;
                        let on_ch = on_change.clone();
                        let label = format!("{hl}");
                        let mut btn =
                            Button::new(ElementId::Name(format!("histlim_{}", hl).into()))
                                .small()
                                .label(label);
                        if is_curr {
                            btn = btn.primary();
                        } else {
                            btn = btn.outline();
                        }
                        btn = btn.on_click(move |_, window, cx| {
                            if let Some(ref handler) = on_ch {
                                handler(
                                    Box::new(move |s| {
                                        s.query.history_limit = hl;
                                    }),
                                    window,
                                    cx,
                                );
                            }
                        });
                        row = row.child(btn);
                    }
                    row
                }),
            )
    }

    fn render_language_tab(&self) -> impl IntoElement {
        let lang = self.settings.language;
        let on_change = self.on_change_settings.clone();

        let langs = [
            (AppLanguage::En, "language.en", "language.en_desc", "🇺🇸"),
            (AppLanguage::ZhCn, "language.zh", "language.zh_desc", "🇨🇳"),
            (
                AppLanguage::Auto,
                "language.auto",
                "language.auto_desc",
                "🌐",
            ),
        ];

        let mut cards_row = h_flex().w_full().items_stretch().gap_4();

        for (l_choice, title_key, desc_key, flag) in langs {
            let is_selected = self.settings.language == l_choice;
            let on_ch = on_change.clone();
            let card_id = format!("lang_card_{:?}", l_choice);

            let card = v_flex()
                .id(ElementId::Name(card_id.into()))
                .w(px(220.0))
                .p_4()
                .rounded_lg()
                .border_2()
                .border_color(if is_selected {
                    ThemeColors::PRIMARY_BORDER
                } else {
                    ThemeColors::BORDER
                })
                .bg(ThemeColors::BG_SURFACE)
                .cursor_pointer()
                .gap_3()
                .hover(|s| {
                    if !is_selected {
                        s.border_color(ThemeColors::BORDER_PROMINENT)
                    } else {
                        s
                    }
                })
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .child(div().text_size(px(24.0)).child(flag))
                        .when(is_selected, |this| {
                            this.child(
                                div()
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .rounded_full()
                                    .bg(ThemeColors::PRIMARY_BG)
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(IconName::Check)
                                            .size(px(13.0))
                                            .text_color(ThemeColors::PRIMARY_BORDER),
                                    ),
                            )
                        }),
                )
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(t(title_key, lang)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(t(desc_key, lang)),
                        ),
                )
                .on_click(move |_, window, cx| {
                    if let Some(ref handler) = on_ch {
                        handler(
                            Box::new(move |s| {
                                s.language = l_choice;
                            }),
                            window,
                            cx,
                        );
                    }
                });

            cards_row = cards_row.child(card);
        }

        v_flex()
            .w_full()
            .gap_4()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t("language.title", lang)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("language.desc", lang)),
                    ),
            )
            .child(cards_row)
    }

    fn render_informative_tab(
        &self,
        title: &'static str,
        desc: &'static str,
        icon: IconName,
        details: Vec<(&'static str, &'static str)>,
    ) -> impl IntoElement {
        let lang = self.settings.language;

        v_flex()
            .w_full()
            .gap_4()
            .child(
                h_flex()
                    .w_full()
                    .p_4()
                    .rounded_lg()
                    .bg(ThemeColors::BG_SURFACE)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(36.0))
                            .h(px(36.0))
                            .rounded_md()
                            .bg(ThemeColors::PRIMARY_BG)
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(icon)
                                    .size(px(18.0))
                                    .text_color(ThemeColors::PRIMARY_BORDER),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(t(title, lang)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t(desc, lang)),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap_2()
                    .children(details.into_iter().map(|(k, v)| {
                        h_flex()
                            .w_full()
                            .p_3()
                            .rounded_md()
                            .bg(ThemeColors::BG_SURFACE)
                            .border_1()
                            .border_color(ThemeColors::BORDER)
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(k),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(v),
                            )
                    })),
            )
    }

    fn render_preference_row(
        &self,
        title_key: &'static str,
        desc_key: &'static str,
        control: impl IntoElement,
    ) -> impl IntoElement {
        let lang = self.settings.language;

        h_flex()
            .w_full()
            .p_3()
            .rounded_lg()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .items_center()
            .justify_between()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t(title_key, lang)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t(desc_key, lang)),
                    ),
            )
            .child(control)
    }

    fn render_toggle_row<F>(
        &self,
        title_key: &'static str,
        desc_key: &'static str,
        enabled: bool,
        on_toggle: F,
    ) -> impl IntoElement
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        let mut toggle_btn =
            Button::new(ElementId::Name(format!("toggle_{}", title_key).into())).small();

        if enabled {
            toggle_btn = toggle_btn.primary().label("Enabled").icon(IconName::Check);
        } else {
            toggle_btn = toggle_btn.outline().label("Disabled");
        }

        toggle_btn = toggle_btn.on_click(move |_, window, cx| {
            on_toggle(window, cx);
        });

        self.render_preference_row(title_key, desc_key, toggle_btn)
    }
}

impl RenderOnce for SettingsView {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        let header = self.render_header();
        let tab_bar = self.render_tab_bar();

        let tab_content = match self.active_tab {
            SettingsTab::Appearance => self.render_appearance_tab().into_any_element(),
            SettingsTab::Editor => self.render_editor_tab().into_any_element(),
            SettingsTab::Query => self.render_query_tab().into_any_element(),
            SettingsTab::Language => self.render_language_tab().into_any_element(),
            SettingsTab::About => self
                .render_informative_tab(
                    "settings.title",
                    "about.desc",
                    IconName::Info,
                    vec![
                        ("Application", "CrabStudio by zqlcrab contributors"),
                        ("Version", concat!("v", env!("CARGO_PKG_VERSION"))),
                        ("GUI Toolkit", "GPUI Kit 0.6.1 + GPUI Engine"),
                        ("License", "Apache-2.0"),
                        ("GitHub", "https://github.com/hyzwhu/zqlcrab"),
                    ],
                )
                .into_any_element(),
        };

        v_flex()
            .size_full()
            .bg(ThemeColors::BG_APP)
            .child(header)
            .child(tab_bar)
            .child(
                v_flex()
                    .id("settings_content_scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .p_6()
                    .child(tab_content),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_tab_default() {
        assert_eq!(SettingsTab::default(), SettingsTab::Appearance);
    }

    #[test]
    fn test_settings_view_builder() {
        let settings = AppSettings::default();
        let res = crate::update::UpdateCheckResult::UpToDate {
            current_version: "0.1.3".to_string(),
            latest_version: "0.1.3".to_string(),
            checked_time: "12:00:00".to_string(),
        };
        let view = SettingsView::new(settings, SettingsTab::Editor)
            .checking_update(true)
            .update_status_msg(Some("Checking...".into()))
            .update_result(Some(res.clone()))
            .on_open_release_url(|_, _, _| {});
        assert_eq!(view.active_tab, SettingsTab::Editor);
        assert!(view.is_checking_update);
        assert_eq!(view.update_status_msg.as_deref(), Some("Checking..."));
        assert_eq!(view.update_result, Some(res));
    }
}
