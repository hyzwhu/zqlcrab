//! SQL Snippet and query template management view with multi-dialect filtering and live execution actions.

use crate::db::snippets::{SnippetCategory, SqlSnippet};
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
pub struct SnippetView {
    snippets: Vec<SqlSnippet>,
    search_input: Entity<InputState>,
    selected_dialect: Option<DatabaseFamily>,
    selected_category: Option<SnippetCategory>,
    language: AppLanguage,
    on_load_snippet: Option<Rc<dyn Fn(SqlSnippet, &mut Window, &mut App) + 'static>>,
    on_run_snippet: Option<Rc<dyn Fn(SqlSnippet, &mut Window, &mut App) + 'static>>,
    on_copy_sql: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_new_snippet: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_edit_snippet: Option<Rc<dyn Fn(SqlSnippet, &mut Window, &mut App) + 'static>>,
    on_delete_snippet: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_dialect: Option<Rc<dyn Fn(Option<DatabaseFamily>, &mut Window, &mut App) + 'static>>,
    on_select_category:
        Option<Rc<dyn Fn(Option<SnippetCategory>, &mut Window, &mut App) + 'static>>,
}

impl SnippetView {
    pub fn new(
        snippets: Vec<SqlSnippet>,
        search_input: &Entity<InputState>,
        selected_dialect: Option<DatabaseFamily>,
        selected_category: Option<SnippetCategory>,
        language: AppLanguage,
    ) -> Self {
        Self {
            snippets,
            search_input: search_input.clone(),
            selected_dialect,
            selected_category,
            language,
            on_load_snippet: None,
            on_run_snippet: None,
            on_copy_sql: None,
            on_new_snippet: None,
            on_edit_snippet: None,
            on_delete_snippet: None,
            on_select_dialect: None,
            on_select_category: None,
        }
    }

    pub fn on_load_snippet<F>(mut self, handler: F) -> Self
    where
        F: Fn(SqlSnippet, &mut Window, &mut App) + 'static,
    {
        self.on_load_snippet = Some(Rc::new(handler));
        self
    }

    pub fn on_run_snippet<F>(mut self, handler: F) -> Self
    where
        F: Fn(SqlSnippet, &mut Window, &mut App) + 'static,
    {
        self.on_run_snippet = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_sql<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_copy_sql = Some(Rc::new(handler));
        self
    }

    pub fn on_new_snippet<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_new_snippet = Some(Rc::new(handler));
        self
    }

    pub fn on_edit_snippet<F>(mut self, handler: F) -> Self
    where
        F: Fn(SqlSnippet, &mut Window, &mut App) + 'static,
    {
        self.on_edit_snippet = Some(Rc::new(handler));
        self
    }

    pub fn on_delete_snippet<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_delete_snippet = Some(Rc::new(handler));
        self
    }

    pub fn on_select_dialect<F>(mut self, handler: F) -> Self
    where
        F: Fn(Option<DatabaseFamily>, &mut Window, &mut App) + 'static,
    {
        self.on_select_dialect = Some(Rc::new(handler));
        self
    }

    pub fn on_select_category<F>(mut self, handler: F) -> Self
    where
        F: Fn(Option<SnippetCategory>, &mut Window, &mut App) + 'static,
    {
        self.on_select_category = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SnippetView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let kw = self.search_input.read(cx).value().trim().to_lowercase();
        let target_dialect = self.selected_dialect;
        let target_cat = self.selected_category;

        let filtered_snippets: Vec<&SqlSnippet> = self
            .snippets
            .iter()
            .filter(|s| {
                if let Some(cat) = target_cat
                    && s.category != cat
                {
                    return false;
                }
                if let Some(dialect) = target_dialect
                    && let Some(s_dialect) = s.dialect
                    && s_dialect != dialect
                {
                    return false;
                }
                if !kw.is_empty() {
                    let in_title = s.title.to_lowercase().contains(&kw);
                    let in_desc = s.description.to_lowercase().contains(&kw);
                    let in_sql = s.sql.to_lowercase().contains(&kw);
                    if !in_title && !in_desc && !in_sql {
                        return false;
                    }
                }
                true
            })
            .collect();

        // 1. Header Toolbar
        let new_btn_handler = self.on_new_snippet.clone();
        let new_btn = Button::new("snip_new_btn")
            .primary()
            .small()
            .icon(IconName::Plus)
            .label(t("snippets.new_snippet", self.language))
            .when_some(new_btn_handler, |btn, handler| {
                btn.on_click(move |_, window, cx| handler(window, cx))
            });

        let header = h_flex()
            .w_full()
            .px_4()
            .py_3()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Code)
                            .size(px(16.0))
                            .text_color(ThemeColors::PRIMARY_BORDER),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(t("snippets.title", self.language)),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!(
                                "{} {}",
                                filtered_snippets.len(),
                                t("snippets.count_suffix", self.language)
                            )),
                    ),
            )
            .child(new_btn);

        // 2. Filter Bar
        let search_bar = Input::new(&self.search_input)
            .cleanable(true)
            .prefix(Icon::new(IconName::Search).size(px(14.0)));

        // Dialect Filter Pills
        let mut dialect_pills = h_flex().items_center().gap_1();
        let dialect_options = [
            (None, "All Dialects"),
            (Some(DatabaseFamily::Postgres), "PostgreSQL"),
            (Some(DatabaseFamily::MySql), "MySQL"),
            (Some(DatabaseFamily::Sqlite), "SQLite"),
        ];

        for (opt_dialect, label) in dialect_options {
            let is_selected = self.selected_dialect == opt_dialect;
            let handler = self.on_select_dialect.clone();
            let pill = div()
                .id(ElementId::Name(format!("snip_dia_{}", label).into()))
                .px_2()
                .py_0p5()
                .rounded_full()
                .text_xs()
                .cursor_pointer()
                .bg(if is_selected {
                    ThemeColors::PRIMARY_BORDER
                } else {
                    ThemeColors::BG_SURFACE_ACTIVE
                })
                .text_color(if is_selected {
                    ThemeColors::TEXT_PRIMARY
                } else {
                    ThemeColors::TEXT_MUTED
                })
                .child(label)
                .when_some(handler, |this, h| {
                    this.on_mouse_down(gpui_kit::gpui::MouseButton::Left, move |_, window, cx| {
                        h(opt_dialect, window, cx);
                    })
                });
            dialect_pills = dialect_pills.child(pill);
        }

        // Category Filter Pills
        let mut category_pills = h_flex().items_center().gap_1();
        let category_options = [
            (None, "All Categories", "✨"),
            (Some(SnippetCategory::Performance), "Performance", "⚡"),
            (Some(SnippetCategory::Maintenance), "Maintenance", "🛠️"),
            (Some(SnippetCategory::Schema), "Schema", "📐"),
            (Some(SnippetCategory::Template), "Templates", "📋"),
            (Some(SnippetCategory::Custom), "Custom", "⭐"),
        ];

        for (opt_cat, label, icon) in category_options {
            let is_selected = self.selected_category == opt_cat;
            let handler = self.on_select_category.clone();
            let pill = div()
                .id(ElementId::Name(format!("snip_cat_{}", label).into()))
                .px_2()
                .py_0p5()
                .rounded_full()
                .text_xs()
                .cursor_pointer()
                .bg(if is_selected {
                    ThemeColors::PRIMARY_BORDER
                } else {
                    ThemeColors::BG_SURFACE_ACTIVE
                })
                .text_color(if is_selected {
                    ThemeColors::TEXT_PRIMARY
                } else {
                    ThemeColors::TEXT_MUTED
                })
                .child(format!("{} {}", icon, label))
                .when_some(handler, |this, h| {
                    this.on_mouse_down(gpui_kit::gpui::MouseButton::Left, move |_, window, cx| {
                        h(opt_cat, window, cx);
                    })
                });
            category_pills = category_pills.child(pill);
        }

        let filter_panel = v_flex()
            .w_full()
            .px_4()
            .py_2p5()
            .gap_2()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_APP)
            .child(search_bar)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(dialect_pills)
                    .child(category_pills),
            );

        // 3. Snippet Card List
        let content = if filtered_snippets.is_empty() {
            div()
                .id("snippet_empty_state")
                .flex_1()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    Icon::new(IconName::Code)
                        .size(px(36.0))
                        .text_color(ThemeColors::TEXT_FAINT),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(t("snippets.no_matching", self.language)),
                )
        } else {
            let mut list = v_flex().gap_3().p_4();

            for (idx, item) in filtered_snippets.iter().enumerate() {
                let snippet = (*item).clone();
                let sql_for_copy = snippet.sql.clone();
                let snip_for_run = snippet.clone();
                let snip_for_load = snippet.clone();
                let snip_for_edit = snippet.clone();
                let id_for_del = snippet.id.clone();

                let on_load = self.on_load_snippet.clone();
                let on_run = self.on_run_snippet.clone();
                let on_copy = self.on_copy_sql.clone();
                let on_edit = self.on_edit_snippet.clone();
                let on_delete = self.on_delete_snippet.clone();

                // Dialect badge
                let dialect_badge = match item.dialect {
                    Some(DatabaseFamily::Postgres) => div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgba(0x0284c715))
                        .text_color(rgba(0x38bdf8ff))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .child("PostgreSQL"),
                    Some(DatabaseFamily::MySql) => div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgba(0xf59e0b15))
                        .text_color(rgba(0xfbbf24ff))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .child("MySQL"),
                    Some(DatabaseFamily::Sqlite) => div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgba(0x10b98115))
                        .text_color(rgba(0x34d399ff))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .child("SQLite"),
                    None => div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(ThemeColors::BG_SURFACE_ACTIVE)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Universal"),
                };

                let category_badge = div()
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                    .text_color(ThemeColors::TEXT_MUTED)
                    .text_xs()
                    .child(format!(
                        "{} {}",
                        item.category.icon_badge(),
                        item.category.display_label()
                    ));

                let type_indicator = if item.is_built_in {
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgba(0x6366f115))
                        .text_color(rgba(0x818cf8ff))
                        .text_xs()
                        .child("Built-in")
                } else {
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgba(0xec489915))
                        .text_color(rgba(0xf472b6ff))
                        .text_xs()
                        .child("Custom")
                };

                // SQL code preview (limited to first 4 lines with line numbers)
                let lines: Vec<&str> = item.sql.lines().take(5).collect();
                let has_more_lines = item.sql.lines().count() > 5;
                let preview_text = lines.join("\n")
                    + if has_more_lines {
                        "\n... (more lines)"
                    } else {
                        ""
                    };

                // Action buttons
                let mut action_bar = h_flex().items_center().gap_1p5();

                let load_btn = Button::new(format!("snip_load_{}", idx))
                    .small()
                    .ghost()
                    .icon(IconName::Code)
                    .label(t("snippets.load_editor", self.language))
                    .tooltip(t("snippets.load_editor_tooltip", self.language))
                    .when_some(on_load, move |btn, handler| {
                        let s = snip_for_load.clone();
                        btn.on_click(move |_, window, cx| handler(s.clone(), window, cx))
                    });

                let run_btn = Button::new(format!("snip_run_{}", idx))
                    .small()
                    .primary()
                    .icon(IconName::Play)
                    .label(t("snippets.run", self.language))
                    .tooltip(t("snippets.run_tooltip", self.language))
                    .when_some(on_run, move |btn, handler| {
                        let s = snip_for_run.clone();
                        btn.on_click(move |_, window, cx| handler(s.clone(), window, cx))
                    });

                let copy_btn = Button::new(format!("snip_copy_{}", idx))
                    .small()
                    .ghost()
                    .icon(IconName::Copy)
                    .tooltip(t("snippets.copy_sql", self.language))
                    .when_some(on_copy, move |btn, handler| {
                        let sql = sql_for_copy.clone();
                        btn.on_click(move |_, window, cx| handler(sql.clone(), window, cx))
                    });

                action_bar = action_bar.child(load_btn).child(run_btn).child(copy_btn);

                if !item.is_built_in {
                    let edit_btn = Button::new(format!("snip_edit_{}", idx))
                        .small()
                        .ghost()
                        .icon(IconName::Settings)
                        .tooltip(t("snippets.edit", self.language))
                        .when_some(on_edit, move |btn, handler| {
                            let s = snip_for_edit.clone();
                            btn.on_click(move |_, window, cx| handler(s.clone(), window, cx))
                        });

                    let delete_btn = Button::new(format!("snip_del_{}", idx))
                        .small()
                        .ghost()
                        .icon(IconName::Trash)
                        .tooltip(t("snippets.delete", self.language))
                        .when_some(on_delete, move |btn, handler| {
                            let id = id_for_del.clone();
                            btn.on_click(move |_, window, cx| handler(id.clone(), window, cx))
                        });

                    action_bar = action_bar.child(edit_btn).child(delete_btn);
                }

                let card = v_flex()
                    .w_full()
                    .p_3()
                    .gap_2()
                    .rounded_md()
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .hover(|style| {
                        style
                            .border_color(ThemeColors::PRIMARY_BORDER)
                            .bg(ThemeColors::BG_SURFACE_HOVER)
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_sm()
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(item.title.clone()),
                                    )
                                    .child(dialect_badge)
                                    .child(category_badge)
                                    .child(type_indicator),
                            )
                            .child(action_bar),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(item.description.clone()),
                    )
                    .child(
                        div()
                            .w_full()
                            .p_2()
                            .rounded_sm()
                            .bg(ThemeColors::BG_APP)
                            .border_1()
                            .border_color(ThemeColors::BORDER)
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(preview_text),
                    );

                list = list.child(card);
            }

            div()
                .id("snippet_cards_scroll")
                .flex_1()
                .overflow_y_scroll()
                .child(list)
        };

        v_flex()
            .size_full()
            .bg(ThemeColors::BG_APP)
            .child(header)
            .child(filter_panel)
            .child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snippet_category_display_and_icons() {
        assert_eq!(
            SnippetCategory::Performance.display_label(),
            "Performance & Locks"
        );
        assert_eq!(SnippetCategory::Performance.icon_badge(), "⚡");
        assert_eq!(
            SnippetCategory::Maintenance.display_label(),
            "Maintenance & Health"
        );
        assert_eq!(SnippetCategory::Maintenance.icon_badge(), "🛠️");
        assert_eq!(SnippetCategory::Schema.display_label(), "Schema & Indexes");
        assert_eq!(SnippetCategory::Schema.icon_badge(), "📐");
        assert_eq!(SnippetCategory::Template.display_label(), "Query Template");
        assert_eq!(SnippetCategory::Template.icon_badge(), "📋");
        assert_eq!(SnippetCategory::Custom.display_label(), "Custom Snippet");
        assert_eq!(SnippetCategory::Custom.icon_badge(), "⭐");
    }
}
