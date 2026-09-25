//! Modal dialog for creating and editing SQL Snippets and reusable scripts.

use crate::db::snippets::SnippetCategory;
use crate::db::types::DatabaseFamily;
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
};
use gpui_kit::gpui::{
    App, ElementId, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _,
    px, rgba,
};
use std::rc::Rc;

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct SnippetEditModal {
    pub is_edit: bool,
    title_input: Entity<InputState>,
    desc_input: Entity<InputState>,
    sql_input: Entity<InputState>,
    selected_category: SnippetCategory,
    selected_dialect: Option<DatabaseFamily>,
    error_message: Option<String>,
    language: AppLanguage,
    on_save: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_cancel: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_select_category: Option<Rc<dyn Fn(SnippetCategory, &mut Window, &mut App) + 'static>>,
    on_select_dialect: Option<Rc<dyn Fn(Option<DatabaseFamily>, &mut Window, &mut App) + 'static>>,
}

#[allow(clippy::too_many_arguments)]
impl SnippetEditModal {
    pub fn new(
        is_edit: bool,
        title_input: &Entity<InputState>,
        desc_input: &Entity<InputState>,
        sql_input: &Entity<InputState>,
        selected_category: SnippetCategory,
        selected_dialect: Option<DatabaseFamily>,
        error_message: Option<String>,
        language: AppLanguage,
    ) -> Self {
        Self {
            is_edit,
            title_input: title_input.clone(),
            desc_input: desc_input.clone(),
            sql_input: sql_input.clone(),
            selected_category,
            selected_dialect,
            error_message,
            language,
            on_save: None,
            on_cancel: None,
            on_select_category: None,
            on_select_dialect: None,
        }
    }

    pub fn on_save<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_save = Some(Rc::new(handler));
        self
    }

    pub fn on_cancel<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_cancel = Some(Rc::new(handler));
        self
    }

    pub fn on_select_category<F>(mut self, handler: F) -> Self
    where
        F: Fn(SnippetCategory, &mut Window, &mut App) + 'static,
    {
        self.on_select_category = Some(Rc::new(handler));
        self
    }

    pub fn on_select_dialect<F>(mut self, handler: F) -> Self
    where
        F: Fn(Option<DatabaseFamily>, &mut Window, &mut App) + 'static,
    {
        self.on_select_dialect = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SnippetEditModal {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let cancel_handler = self.on_cancel.clone();
        let save_handler = self.on_save.clone();

        // Dialect options pills
        let mut dialect_pills = h_flex().items_center().gap_1();
        let dialects = [
            (None, "Universal"),
            (Some(DatabaseFamily::Postgres), "PostgreSQL"),
            (Some(DatabaseFamily::MySql), "MySQL"),
            (Some(DatabaseFamily::Sqlite), "SQLite"),
        ];

        for (opt_dia, label) in dialects {
            let is_sel = self.selected_dialect == opt_dia;
            let handler = self.on_select_dialect.clone();
            let pill = div()
                .id(ElementId::Name(format!("snip_edit_dia_{}", label).into()))
                .px_2()
                .py_1()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .bg(if is_sel {
                    ThemeColors::PRIMARY_BORDER
                } else {
                    ThemeColors::BG_SURFACE_ACTIVE
                })
                .text_color(if is_sel {
                    ThemeColors::TEXT_PRIMARY
                } else {
                    ThemeColors::TEXT_MUTED
                })
                .child(label)
                .when_some(handler, |this, h| {
                    this.on_mouse_down(gpui_kit::gpui::MouseButton::Left, move |_, window, cx| {
                        h(opt_dia, window, cx);
                    })
                });
            dialect_pills = dialect_pills.child(pill);
        }

        // Category options pills
        let mut category_pills = h_flex().items_center().gap_1();
        let categories = [
            (SnippetCategory::Custom, "Custom", "⭐"),
            (SnippetCategory::Template, "Template", "📋"),
            (SnippetCategory::Performance, "Performance", "⚡"),
            (SnippetCategory::Maintenance, "Maintenance", "🛠️"),
            (SnippetCategory::Schema, "Schema", "📐"),
        ];

        for (cat, label, icon) in categories {
            let is_sel = self.selected_category == cat;
            let handler = self.on_select_category.clone();
            let pill = div()
                .id(ElementId::Name(format!("snip_edit_cat_{}", label).into()))
                .px_2()
                .py_1()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .bg(if is_sel {
                    ThemeColors::PRIMARY_BORDER
                } else {
                    ThemeColors::BG_SURFACE_ACTIVE
                })
                .text_color(if is_sel {
                    ThemeColors::TEXT_PRIMARY
                } else {
                    ThemeColors::TEXT_MUTED
                })
                .child(format!("{} {}", icon, label))
                .when_some(handler, |this, h| {
                    this.on_mouse_down(gpui_kit::gpui::MouseButton::Left, move |_, window, cx| {
                        h(cat, window, cx);
                    })
                });
            category_pills = category_pills.child(pill);
        }

        // Dialog frame
        let modal_title = if self.is_edit {
            t("snippets.edit_modal_title", self.language)
        } else {
            t("snippets.new_modal_title", self.language)
        };

        let dialog = v_flex()
            .w(px(580.0))
            .max_h(px(640.0))
            .rounded_lg()
            .border_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .shadow_lg()
            .overflow_hidden()
            // Header
            .child(
                h_flex()
                    .w_full()
                    .px_5()
                    .py_3p5()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(IconName::Code)
                                    .size(px(18.0))
                                    .text_color(ThemeColors::PRIMARY_BORDER),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_base()
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(modal_title),
                            ),
                    )
                    .child(
                        Button::new("snip_modal_close")
                            .small()
                            .ghost()
                            .icon(IconName::Close)
                            .when_some(cancel_handler.clone(), |btn, h| {
                                btn.on_click(move |_, window, cx| h(window, cx))
                            }),
                    ),
            )
            // Body Form
            .child(
                div()
                    .id("snippet_form_scroll")
                    .w_full()
                    .p_5()
                    .gap_3p5()
                    .overflow_y_scroll()
                    // Title field
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("snippets.field_title", self.language)),
                            )
                            .child(Input::new(&self.title_input).cleanable(true)),
                    )
                    // Category & Dialect pickers
                    .child(
                        h_flex()
                            .w_full()
                            .gap_4()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(t("snippets.field_category", self.language)),
                                    )
                                    .child(category_pills),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(t("snippets.field_dialect", self.language)),
                                    )
                                    .child(dialect_pills),
                            ),
                    )
                    // Description field
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("snippets.field_desc", self.language)),
                            )
                            .child(Input::new(&self.desc_input).cleanable(true)),
                    )
                    // SQL query text field
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("snippets.field_sql", self.language)),
                            )
                            .child(Input::new(&self.sql_input)),
                    )
                    // Error notice if any
                    .when_some(self.error_message, |this, err| {
                        this.child(
                            div()
                                .w_full()
                                .p_2p5()
                                .rounded_md()
                                .bg(rgba(0xef444415))
                                .border_1()
                                .border_color(rgba(0xef444440))
                                .text_xs()
                                .text_color(rgba(0xf87171ff))
                                .child(err),
                        )
                    }),
            )
            // Footer Actions
            .child(
                h_flex()
                    .w_full()
                    .px_5()
                    .py_3()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_APP)
                    .child(
                        Button::new("snip_modal_cancel")
                            .ghost()
                            .label(t("common.cancel", self.language))
                            .when_some(cancel_handler, |btn, h| {
                                btn.on_click(move |_, window, cx| h(window, cx))
                            }),
                    )
                    .child(
                        Button::new("snip_modal_save")
                            .primary()
                            .label(t("common.save", self.language))
                            .when_some(save_handler, |btn, h| {
                                btn.on_click(move |_, window, cx| h(window, cx))
                            }),
                    ),
            );

        // Modal backdrop overlay
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x00000088))
            .items_center()
            .justify_center()
            .child(dialog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snippet_edit_modal_modes() {
        assert_eq!(SnippetCategory::default(), SnippetCategory::Custom);
    }
}
