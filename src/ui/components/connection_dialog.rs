//! Connection profile modal dialog for creating and configuring database connections.

use crate::db::types::DatabaseType;
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
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, px, rgba,
};
use std::rc::Rc;

#[derive(IntoElement)]
pub struct ConnectionDialog {
    db_type: DatabaseType,
    name_input: Entity<InputState>,
    host_input: Entity<InputState>,
    port_input: Entity<InputState>,
    database_input: Entity<InputState>,
    user_input: Entity<InputState>,
    pass_input: Entity<InputState>,
    is_testing: bool,
    test_result: Option<Result<String, String>>,
    on_select_type: Option<Rc<dyn Fn(DatabaseType, &mut Window, &mut App) + 'static>>,
    on_test: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_save: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_cancel: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl ConnectionDialog {
    pub fn new(
        db_type: DatabaseType,
        name_input: &Entity<InputState>,
        host_input: &Entity<InputState>,
        port_input: &Entity<InputState>,
        database_input: &Entity<InputState>,
        user_input: &Entity<InputState>,
        pass_input: &Entity<InputState>,
    ) -> Self {
        Self {
            db_type,
            name_input: name_input.clone(),
            host_input: host_input.clone(),
            port_input: port_input.clone(),
            database_input: database_input.clone(),
            user_input: user_input.clone(),
            pass_input: pass_input.clone(),
            is_testing: false,
            test_result: None,
            on_select_type: None,
            on_test: None,
            on_save: None,
            on_cancel: None,
        }
    }

    pub fn testing(mut self, is_testing: bool) -> Self {
        self.is_testing = is_testing;
        self
    }

    pub fn test_result(mut self, res: Option<Result<String, String>>) -> Self {
        self.test_result = res;
        self
    }

    pub fn on_select_type<F>(mut self, handler: F) -> Self
    where
        F: Fn(DatabaseType, &mut Window, &mut App) + 'static,
    {
        self.on_select_type = Some(Rc::new(handler));
        self
    }

    pub fn on_test<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_test = Some(Rc::new(handler));
        self
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
}

impl RenderOnce for ConnectionDialog {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let cancel_handler = self.on_cancel.clone();
        let mut close_btn = Button::new("close_dialog")
            .ghost()
            .xsmall()
            .icon(IconName::X);
        if let Some(ref on_cancel) = cancel_handler {
            let on_cancel = on_cancel.clone();
            close_btn = close_btn.on_click(move |_, window, cx| {
                on_cancel(window, cx);
            });
        }

        let mut cancel_btn = Button::new("cancel_btn")
            .ghost()
            .small()
            .label("Cancel");
        if let Some(ref on_cancel) = cancel_handler {
            let on_cancel = on_cancel.clone();
            cancel_btn = cancel_btn.on_click(move |_, window, cx| {
                on_cancel(window, cx);
            });
        }

        let mut test_btn = Button::new("test_conn_btn")
            .outline()
            .small()
            .icon(IconName::Activity)
            .label(if self.is_testing { "Testing..." } else { "Test Connection" });
        if let Some(on_test) = self.on_test {
            test_btn = test_btn.on_click(move |_, window, cx| {
                on_test(window, cx);
            });
        }

        let mut save_btn = Button::new("save_conn_btn")
            .primary()
            .small()
            .icon(IconName::Check)
            .label("Save & Connect");
        if let Some(on_save) = self.on_save {
            save_btn = save_btn.on_click(move |_, window, cx| {
                on_save(window, cx);
            });
        }

        // Database engine type selection cards
        let engines = [
            (DatabaseType::Sqlite, "SQLite", IconName::Database),
            (DatabaseType::Postgres, "PostgreSQL", IconName::Layers),
            (DatabaseType::Mysql, "MySQL", IconName::Cpu),
        ];

        let mut engine_selector = h_flex().gap_2().w_full();
        for (dtype, name, icon) in engines {
            let is_selected = self.db_type == dtype;
            let on_select = self.on_select_type.clone();

            let card = h_flex()
                .id(ElementId::Name(format!("engine_{:?}", dtype).into()))
                .flex_1()
                .p_2p5()
                .items_center()
                .justify_center()
                .gap_2()
                .rounded_md()
                .border_1()
                .cursor_pointer()
                .border_color(if is_selected {
                    ThemeColors::PRIMARY_BORDER
                } else {
                    ThemeColors::BORDER
                })
                .bg(if is_selected {
                    ThemeColors::BG_SURFACE_ACTIVE
                } else {
                    ThemeColors::BG_SURFACE
                })
                .hover(|s| s.bg(ThemeColors::BG_SURFACE_HOVER))
                .child(
                    Icon::new(icon)
                        .size(px(16.0))
                        .text_color(if is_selected {
                            ThemeColors::PRIMARY_BORDER
                        } else {
                            ThemeColors::TEXT_MUTED
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(if is_selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if is_selected {
                            ThemeColors::TEXT_PRIMARY
                        } else {
                            ThemeColors::TEXT_MUTED
                        })
                        .child(name),
                )
                .on_click(move |_, window, cx| {
                    if let Some(ref handler) = on_select {
                        handler(dtype, window, cx);
                    }
                });

            engine_selector = engine_selector.child(card);
        }

        // Form fields depending on database type
        let form_fields = match self.db_type {
            DatabaseType::Sqlite => {
                v_flex()
                    .gap_3()
                    .w_full()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child("Connection Name"),
                            )
                            .child(Input::new(&self.name_input).id("conn_name").w_full()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child("Database File Path (or ':memory:')"),
                            )
                            .child(Input::new(&self.database_input).id("db_path").w_full())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_FAINT)
                                    .child("Tip: Use ':memory:' for an in-memory SQLite database, or a local file path."),
                            ),
                    )
            }
            DatabaseType::Postgres | DatabaseType::Mysql => {
                let default_port = if self.db_type == DatabaseType::Postgres { "5432" } else { "3306" };
                v_flex()
                    .gap_3()
                    .w_full()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child("Connection Name"),
                            )
                            .child(Input::new(&self.name_input).id("conn_name").w_full()),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .w_full()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child("Host"),
                                    )
                                    .child(Input::new(&self.host_input).id("conn_host").w_full()),
                            )
                            .child(
                                v_flex()
                                    .w(px(100.0))
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(format!("Port ({default_port})")),
                                    )
                                    .child(Input::new(&self.port_input).id("conn_port").w_full()),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child("Database Name"),
                            )
                            .child(Input::new(&self.database_input).id("conn_db").w_full()),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .w_full()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child("Username"),
                                    )
                                    .child(Input::new(&self.user_input).id("conn_user").w_full()),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child("Password"),
                                    )
                                    .child(Input::new(&self.pass_input).id("conn_pass").w_full()),
                            ),
                    )
            }
        };

        // Test status notification
        let test_banner = self.test_result.map(|res| {
            match res {
                Ok(msg) => h_flex()
                    .p_2()
                    .rounded_md()
                    .items_center()
                    .gap_2()
                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                    .border_1()
                    .border_color(ThemeColors::SUCCESS)
                    .child(
                        Icon::new(IconName::CircleCheck)
                            .size(px(14.0))
                            .text_color(ThemeColors::SUCCESS),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(msg),
                    ),
                Err(err) => h_flex()
                    .p_2()
                    .rounded_md()
                    .items_center()
                    .gap_2()
                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                    .border_1()
                    .border_color(ThemeColors::ERROR)
                    .child(
                        Icon::new(IconName::TriangleAlert)
                            .size(px(14.0))
                            .text_color(ThemeColors::ERROR),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::ERROR)
                            .child(err),
                    ),
            }
        });

        // Dialog container modal
        let modal = v_flex()
            .w(px(500.0))
            .bg(ThemeColors::BG_APP)
            .rounded_xl()
            .border_1()
            .border_color(ThemeColors::BORDER)
            .shadow_lg()
            .child(
                // Modal header
                h_flex()
                    .w_full()
                    .p_4()
                    .justify_between()
                    .items_center()
                    .border_b_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2p5()
                            .child(
                                Icon::new(IconName::Server)
                                    .size(px(18.0))
                                    .text_color(ThemeColors::PRIMARY_BORDER),
                            )
                            .child(
                                v_flex()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child("New Database Connection"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child("Configure a new database profile to connect"),
                                    ),
                            ),
                    )
                    .child(close_btn),
            )
            .child(
                // Modal body
                v_flex()
                    .p_4()
                    .gap_4()
                    .child(engine_selector)
                    .child(form_fields)
                    .children(test_banner),
            )
            .child(
                // Modal footer
                h_flex()
                    .w_full()
                    .p_3()
                    .px_4()
                    .justify_between()
                    .items_center()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .child(test_btn)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(cancel_btn)
                            .child(save_btn),
                    ),
            );

        // Backdrop overlay
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x000000AA))
            .justify_center()
            .items_center()
            .flex()
            .child(modal)
    }
}
