//! Database Index Advisor & Health Diagnostics Modal Dialog.
//!
//! Provides a visual dashboard for inspecting index health, redundant/duplicate indexes,
//! unindexed foreign keys, and query-driven missing index recommendations with executable DDL.

use crate::db::index_advisor::{
    IndexHealthReport, IndexHealthScoreLevel, IndexIssueSeverity, IndexIssueType,
    generate_all_remediation_sql,
};
use crate::db::types::DatabaseFamily;
use crate::settings::AppLanguage;
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    spinner::Spinner,
};
use gpui_kit::gpui::{
    App, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::*, px, rgba,
};
use std::rc::Rc;

/// Active tab in the Index Advisor modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndexAdvisorTab {
    #[default]
    HealthAudit,
    QueryAdvisor,
}

#[derive(IntoElement)]
#[allow(clippy::type_complexity)]
pub struct IndexAdvisorModal {
    active_tab: IndexAdvisorTab,
    table_name: String,
    report: Option<IndexHealthReport>,
    query_text: Option<String>,
    family: DatabaseFamily,
    is_loading: bool,
    language: AppLanguage,
    on_tab_change: Option<Rc<dyn Fn(IndexAdvisorTab, &mut Window, &mut App) + 'static>>,
    on_refresh: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_copy_sql: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_open_console: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl IndexAdvisorModal {
    pub fn new(
        table_name: impl Into<String>,
        report: Option<IndexHealthReport>,
        query_text: Option<String>,
        family: DatabaseFamily,
        language: AppLanguage,
    ) -> Self {
        Self {
            active_tab: IndexAdvisorTab::HealthAudit,
            table_name: table_name.into(),
            report,
            query_text,
            family,
            is_loading: false,
            language,
            on_tab_change: None,
            on_refresh: None,
            on_copy_sql: None,
            on_open_console: None,
            on_close: None,
        }
    }

    pub fn active_tab(mut self, tab: IndexAdvisorTab) -> Self {
        self.active_tab = tab;
        self
    }

    pub fn is_loading(mut self, loading: bool) -> Self {
        self.is_loading = loading;
        self
    }

    pub fn on_tab_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(IndexAdvisorTab, &mut Window, &mut App) + 'static,
    {
        self.on_tab_change = Some(Rc::new(handler));
        self
    }

    pub fn on_refresh<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_refresh = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_sql<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_copy_sql = Some(Rc::new(handler));
        self
    }

    pub fn on_open_console<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_open_console = Some(Rc::new(handler));
        self
    }

    pub fn on_close<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for IndexAdvisorModal {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let close_handler = self.on_close.clone();
        let refresh_handler = self.on_refresh.clone();
        let copy_handler = self.on_copy_sql.clone();
        let console_handler = self.on_open_console.clone();
        let tab_handler = self.on_tab_change.clone();

        let remediation_sql = if let Some(r) = &self.report {
            generate_all_remediation_sql(r, self.family)
        } else {
            String::new()
        };

        // Header
        let mut close_btn = Button::new("advisor_close_btn")
            .ghost()
            .xsmall()
            .icon(IconName::Close);
        if let Some(on_close) = close_handler.clone() {
            close_btn = close_btn.on_click(move |_, window, cx| {
                on_close(window, cx);
            });
        }

        let mut refresh_btn = Button::new("advisor_refresh_btn")
            .ghost()
            .xsmall()
            .icon(IconName::RotateCw)
            .tooltip(t("advisor.refresh_tooltip", self.language));
        if let Some(on_ref) = refresh_handler {
            refresh_btn = refresh_btn.on_click(move |_, window, cx| {
                on_ref(window, cx);
            });
        }

        let header = h_flex()
            .w_full()
            .px_5()
            .py_3p5()
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
                        Icon::new(IconName::Zap)
                            .size(px(20.0))
                            .text_color(ThemeColors::PRIMARY_BORDER),
                    )
                    .child(
                        v_flex()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_base()
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(t("advisor.title", self.language)),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(ThemeColors::PRIMARY_BORDER)
                                            .child(self.table_name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("advisor.subtitle", self.language)),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(refresh_btn)
                    .child(close_btn),
            );

        // Tab Navigation
        let is_tab_health = self.active_tab == IndexAdvisorTab::HealthAudit;
        let is_tab_query = self.active_tab == IndexAdvisorTab::QueryAdvisor;

        let tab_health_handler = tab_handler.clone();
        let tab_query_handler = tab_handler.clone();

        let tab_bar = h_flex()
            .w_full()
            .px_5()
            .py_1p5()
            .gap_4()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_APP)
            .child(
                div()
                    .id("tab_advisor_health")
                    .px_2()
                    .py_1()
                    .cursor_pointer()
                    .border_b_2()
                    .text_xs()
                    .font_weight(if is_tab_health {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .when(is_tab_health, |this| {
                        this.border_color(ThemeColors::PRIMARY_BORDER)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                    })
                    .when(!is_tab_health, |this| {
                        this.border_color(rgba(0x00000000))
                            .text_color(ThemeColors::TEXT_MUTED)
                            .hover(|h| h.text_color(ThemeColors::TEXT_PRIMARY))
                    })
                    .child(t("advisor.tab_health", self.language))
                    .when_some(tab_health_handler, |this, h| {
                        this.on_click(move |_, window, cx| {
                            h(IndexAdvisorTab::HealthAudit, window, cx);
                        })
                    }),
            )
            .child(
                div()
                    .id("tab_advisor_query")
                    .px_2()
                    .py_1()
                    .cursor_pointer()
                    .border_b_2()
                    .text_xs()
                    .font_weight(if is_tab_query {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .when(is_tab_query, |this| {
                        this.border_color(ThemeColors::PRIMARY_BORDER)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                    })
                    .when(!is_tab_query, |this| {
                        this.border_color(rgba(0x00000000))
                            .text_color(ThemeColors::TEXT_MUTED)
                            .hover(|h| h.text_color(ThemeColors::TEXT_PRIMARY))
                    })
                    .child(t("advisor.tab_query", self.language))
                    .when_some(tab_query_handler, |this, h| {
                        this.on_click(move |_, window, cx| {
                            h(IndexAdvisorTab::QueryAdvisor, window, cx);
                        })
                    }),
            );

        // Content Area
        let content =
            if self.is_loading {
                v_flex()
                    .size_full()
                    .h(px(380.0))
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(Spinner::new())
                    .child(
                        div()
                            .text_sm()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("advisor.analyzing", self.language)),
                    )
                    .into_any_element()
            } else if let Some(report) = &self.report {
                match self.active_tab {
                    IndexAdvisorTab::HealthAudit => self
                        .render_health_tab(report, copy_handler.clone())
                        .into_any_element(),
                    IndexAdvisorTab::QueryAdvisor => self
                        .render_query_tab(report, copy_handler.clone())
                        .into_any_element(),
                }
            } else {
                v_flex()
                    .size_full()
                    .h(px(380.0))
                    .items_center()
                    .justify_center()
                    .child(div().text_sm().text_color(ThemeColors::TEXT_MUTED).child(
                        "No analysis report available. Click Refresh to inspect table indexes.",
                    ))
                    .into_any_element()
            };

        // Footer Actions
        let sql_for_copy = remediation_sql.clone();
        let mut copy_all_btn = Button::new("advisor_copy_all_btn")
            .ghost()
            .small()
            .icon(IconName::Copy)
            .child(t("advisor.btn_copy_ddl", self.language));
        if let Some(handler) = copy_handler {
            copy_all_btn = copy_all_btn.on_click(move |_, window, cx| {
                handler(sql_for_copy.clone(), window, cx);
            });
        }

        let sql_for_console = remediation_sql;
        let mut open_console_btn = Button::new("advisor_open_console_btn")
            .primary()
            .small()
            .icon(IconName::Terminal)
            .child(t("advisor.btn_open_console", self.language));
        if let Some(handler) = console_handler {
            open_console_btn = open_console_btn.on_click(move |_, window, cx| {
                handler(sql_for_console.clone(), window, cx);
            });
        }

        let family_label = match self.family {
            DatabaseFamily::Postgres => "PostgreSQL",
            DatabaseFamily::MySql => "MySQL",
            DatabaseFamily::Sqlite => "SQLite",
        };

        let footer = h_flex()
            .w_full()
            .px_5()
            .py_3()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Check)
                            .size(px(14.0))
                            .text_color(ThemeColors::SUCCESS),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(format!("Dialect: {}", family_label)),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(copy_all_btn)
                    .child(open_console_btn),
            );

        // Modal Frame with Backdrop
        let modal = v_flex()
            .occlude()
            .w(px(720.0))
            .max_h(px(640.0))
            .rounded_xl()
            .border_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .shadow_xl()
            .overflow_hidden()
            .child(header)
            .child(tab_bar)
            .child(
                div()
                    .id("index_advisor_scroll_area")
                    .w_full()
                    .max_h(px(480.0))
                    .overflow_y_scroll()
                    .child(content),
            )
            .child(footer);

        div()
            .id("index_advisor_backdrop")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .bg(rgba(0x00000088))
            .items_center()
            .justify_center()
            .flex()
            .child(modal)
    }
}

impl IndexAdvisorModal {
    fn render_health_tab(
        &self,
        report: &IndexHealthReport,
        copy_handler: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    ) -> impl IntoElement {
        let (score_color, score_bg) = match report.score_level {
            IndexHealthScoreLevel::Excellent => (ThemeColors::SUCCESS, rgba(0x22c55e18)),
            IndexHealthScoreLevel::Good => (ThemeColors::PRIMARY_BORDER, rgba(0x3b82f618)),
            IndexHealthScoreLevel::Fair => (ThemeColors::WARNING, rgba(0xf59e0b18)),
            IndexHealthScoreLevel::Poor => (ThemeColors::ERROR, rgba(0xef444418)),
        };

        let redundant_count = report
            .issues
            .iter()
            .filter(|i| {
                matches!(
                    i.issue_type,
                    IndexIssueType::PrefixCovered { .. } | IndexIssueType::ExactDuplicate { .. }
                )
            })
            .count();

        let unindexed_fk_count = report
            .issues
            .iter()
            .filter(|i| matches!(i.issue_type, IndexIssueType::UnindexedForeignKey { .. }))
            .count();

        // 1. KPI Strip
        let kpi_cards = h_flex()
            .w_full()
            .gap_3()
            // Score Card
            .child(
                h_flex()
                    .flex_1()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(score_color)
                    .bg(score_bg)
                    .items_center()
                    .justify_between()
                    .child(
                        v_flex()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(t("advisor.health_score", self.language)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(score_color)
                                    .child(report.score_level.display_name()),
                            ),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(score_color)
                            .child(format!("{}/100", report.health_score)),
                    ),
            )
            // Total Indexes
            .child(
                v_flex()
                    .flex_1()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_APP)
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("advisor.kpi_total_indexes", self.language)),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(report.total_indexes.to_string()),
                    ),
            )
            // Redundant Issues
            .child(
                v_flex()
                    .flex_1()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(if redundant_count > 0 {
                        ThemeColors::WARNING
                    } else {
                        ThemeColors::BORDER
                    })
                    .bg(ThemeColors::BG_APP)
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("advisor.kpi_redundant_count", self.language)),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if redundant_count > 0 {
                                ThemeColors::WARNING
                            } else {
                                ThemeColors::TEXT_PRIMARY
                            })
                            .child(redundant_count.to_string()),
                    ),
            )
            // Unindexed FKs
            .child(
                v_flex()
                    .flex_1()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(if unindexed_fk_count > 0 {
                        ThemeColors::ERROR
                    } else {
                        ThemeColors::BORDER
                    })
                    .bg(ThemeColors::BG_APP)
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(t("advisor.kpi_unindexed_fks", self.language)),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if unindexed_fk_count > 0 {
                                ThemeColors::ERROR
                            } else {
                                ThemeColors::TEXT_PRIMARY
                            })
                            .child(unindexed_fk_count.to_string()),
                    ),
            );

        // 2. Diagnostic Issues List
        let mut issues_container = v_flex().w_full().gap_2p5();

        if report.issues.is_empty() {
            issues_container = issues_container.child(
                h_flex()
                    .w_full()
                    .p_4()
                    .items_center()
                    .gap_3()
                    .rounded_lg()
                    .bg(rgba(0x22c55e10))
                    .border_1()
                    .border_color(ThemeColors::SUCCESS)
                    .child(
                        Icon::new(IconName::Check)
                            .size(px(18.0))
                            .text_color(ThemeColors::SUCCESS),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ThemeColors::SUCCESS)
                            .child(t("advisor.empty_healthy", self.language)),
                    ),
            );
        } else {
            for (idx, issue) in report.issues.iter().enumerate() {
                let badge_color = match issue.severity {
                    IndexIssueSeverity::Critical => ThemeColors::ERROR,
                    IndexIssueSeverity::Warning => ThemeColors::WARNING,
                    IndexIssueSeverity::Info => ThemeColors::PRIMARY_BORDER,
                };

                let sql_for_copy = issue.remediation_sql.clone();
                let on_copy = copy_handler.clone();

                let mut copy_fix_btn =
                    Button::new(ElementId::NamedInteger("fix_copy".into(), idx as u64))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Copy)
                        .child("Copy DDL");
                if let (Some(sql), Some(handler)) = (sql_for_copy, on_copy) {
                    copy_fix_btn = copy_fix_btn.on_click(move |_, window, cx| {
                        handler(sql.clone(), window, cx);
                    });
                }

                let card = v_flex()
                    .w_full()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(badge_color)
                    .bg(ThemeColors::BG_SURFACE)
                    .gap_2()
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
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .bg(badge_color)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(issue.severity.badge_label()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(issue.title.clone()),
                                    ),
                            )
                            .child(copy_fix_btn),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(issue.description.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(badge_color)
                            .child(format!("Impact: {}", issue.impact)),
                    )
                    .when_some(issue.remediation_sql.as_ref(), |this, sql| {
                        this.child(
                            div()
                                .w_full()
                                .p_2()
                                .rounded_md()
                                .bg(ThemeColors::BG_APP)
                                .font_family("JetBrains Mono, monospace")
                                .text_xs()
                                .text_color(ThemeColors::PRIMARY_BORDER)
                                .child(sql.clone()),
                        )
                    });

                issues_container = issues_container.child(card);
            }
        }

        // 3. Existing Indexes Listing
        let mut indexes_table = v_flex()
            .w_full()
            .rounded_lg()
            .border_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .overflow_hidden();

        let table_header = h_flex()
            .w_full()
            .px_3()
            .py_2()
            .bg(ThemeColors::BG_APP)
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(
                div()
                    .w(px(200.0))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child("Index Name"),
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child("Columns"),
            )
            .child(
                div()
                    .w(px(80.0))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child("Type"),
            );

        indexes_table = indexes_table.child(table_header);

        for idx in &report.existing_indexes {
            let type_str = if idx.is_primary {
                "PRIMARY"
            } else if idx.is_unique {
                "UNIQUE"
            } else {
                "INDEX"
            };

            let row = h_flex()
                .w_full()
                .px_3()
                .py_1p5()
                .border_b_1()
                .border_color(ThemeColors::BORDER)
                .items_center()
                .child(
                    div()
                        .w(px(200.0))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(idx.name.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .font_family("JetBrains Mono, monospace")
                        .text_color(ThemeColors::PRIMARY_BORDER)
                        .child(format!("({})", idx.columns.join(", "))),
                )
                .child(
                    div()
                        .w(px(80.0))
                        .text_xs()
                        .text_color(if idx.is_primary || idx.is_unique {
                            ThemeColors::SUCCESS
                        } else {
                            ThemeColors::TEXT_MUTED
                        })
                        .child(type_str),
                );

            indexes_table = indexes_table.child(row);
        }

        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(kpi_cards)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(t("advisor.diagnostic_issues", self.language)),
            )
            .child(issues_container)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(t("advisor.existing_indexes", self.language)),
            )
            .child(indexes_table)
    }

    fn render_query_tab(
        &self,
        report: &IndexHealthReport,
        copy_handler: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    ) -> impl IntoElement {
        let sql_preview = self
            .query_text
            .as_deref()
            .unwrap_or("-- [No SQL Query Selected]");

        let mut recs_container = v_flex().w_full().gap_3();

        if report.missing_recommendations.is_empty() {
            recs_container = recs_container.child(
                v_flex()
                    .w_full()
                    .p_5()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .rounded_lg()
                    .bg(ThemeColors::BG_SURFACE)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .child(
                        Icon::new(IconName::Check)
                            .size(px(24.0))
                            .text_color(ThemeColors::SUCCESS),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child("No Missing Indexes Recommended"),
                    )
                    .child(div().text_xs().text_color(ThemeColors::TEXT_MUTED).child(
                        "The analyzed query conditions are already satisfied by existing indexes.",
                    )),
            );
        } else {
            for (idx, rec) in report.missing_recommendations.iter().enumerate() {
                let sql_for_copy = rec.create_ddl.clone();
                let on_copy = copy_handler.clone();

                let mut copy_btn =
                    Button::new(ElementId::NamedInteger("rec_copy".into(), idx as u64))
                        .primary()
                        .xsmall()
                        .icon(IconName::Copy)
                        .child("Copy CREATE INDEX");
                if let Some(handler) = on_copy {
                    copy_btn = copy_btn.on_click(move |_, window, cx| {
                        handler(sql_for_copy.clone(), window, cx);
                    });
                }

                let card = v_flex()
                    .w_full()
                    .p_3p5()
                    .rounded_lg()
                    .border_1()
                    .border_color(ThemeColors::PRIMARY_BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .gap_2()
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
                                        Icon::new(IconName::Zap)
                                            .size(px(16.0))
                                            .text_color(ThemeColors::WARNING),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child(format!(
                                                "Suggested Index: {}",
                                                rec.suggested_index_name
                                            )),
                                    ),
                            )
                            .child(copy_btn),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child(rec.reason.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ThemeColors::SUCCESS)
                            .child(format!("Estimated Benefit: {}", rec.estimated_improvement)),
                    )
                    .child(
                        div()
                            .w_full()
                            .p_2()
                            .rounded_md()
                            .bg(ThemeColors::BG_APP)
                            .font_family("JetBrains Mono, monospace")
                            .text_xs()
                            .text_color(ThemeColors::PRIMARY_BORDER)
                            .child(rec.create_ddl.clone()),
                    );

                recs_container = recs_container.child(card);
            }
        }

        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(
                v_flex()
                    .w_full()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child("Target SQL Query for Optimization:"),
                    )
                    .child(
                        div()
                            .w_full()
                            .p_3()
                            .rounded_md()
                            .bg(ThemeColors::BG_APP)
                            .border_1()
                            .border_color(ThemeColors::BORDER)
                            .font_family("JetBrains Mono, monospace")
                            .text_xs()
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(sql_preview.to_string()),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(t("advisor.recommend_title", self.language)),
            )
            .child(recs_container)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_advisor_tab_variants() {
        assert_eq!(IndexAdvisorTab::default(), IndexAdvisorTab::HealthAudit);
    }

    #[test]
    fn test_advisor_modal_creation() {
        let modal = IndexAdvisorModal::new(
            "users",
            None,
            Some("SELECT * FROM users WHERE email = 'test'".into()),
            DatabaseFamily::Postgres,
            AppLanguage::En,
        )
        .active_tab(IndexAdvisorTab::QueryAdvisor)
        .is_loading(false);

        assert_eq!(modal.table_name, "users");
        assert_eq!(modal.active_tab, IndexAdvisorTab::QueryAdvisor);
        assert!(!modal.is_loading);
    }
}
