//! Connection profile modal dialog for creating and configuring database connections.

use crate::db::types::{DatabaseFamily, DatabaseType};
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
};
use gpui_kit::gpui::{
    Anchor, App, Entity, FontWeight, IntoElement, ParentElement,
    RenderOnce, Styled, Window, div, px, rgba,
};
use std::rc::Rc;

/// Helper to resolve standard icons for all supported database engines.
pub fn database_icon(db_type: DatabaseType) -> IconName {
    match db_type {
        DatabaseType::Sqlite => IconName::Database,

        // MySQL Protocol Ecosystem
        DatabaseType::Mysql => IconName::Fish,
        DatabaseType::MariaDB => IconName::Fish,
        DatabaseType::TiDB => IconName::Atom,
        DatabaseType::OceanBase => IconName::Anchor,
        DatabaseType::StarRocks => IconName::Sparkles,
        DatabaseType::Doris => IconName::Boxes,
        DatabaseType::PolarDB => IconName::Compass,
        DatabaseType::TDSQL => IconName::Shield,
        DatabaseType::SelectDB => IconName::Warehouse,
        DatabaseType::Databend => IconName::Infinity,
        DatabaseType::GoldenDB => IconName::Gem,
        DatabaseType::SingleStore => IconName::Zap,
        DatabaseType::ManticoreSearch => IconName::Search,
        DatabaseType::CloudSQLMySQL => IconName::Cloud,

        // PostgreSQL Protocol Ecosystem
        DatabaseType::Postgres => IconName::Layers,
        DatabaseType::CockroachDB => IconName::Orbit,
        DatabaseType::TimescaleDB => IconName::Clock,
        DatabaseType::Redshift => IconName::Flame,
        DatabaseType::YugabyteDB => IconName::Globe,
        DatabaseType::OpenGauss => IconName::Feather,
        DatabaseType::Kingbase => IconName::Crown,
        DatabaseType::GaussDB => IconName::Cpu,
        DatabaseType::Greenplum => IconName::TreePine,
        DatabaseType::QuestDB => IconName::Gauge,
        DatabaseType::Vastbase => IconName::HardDrive,
        DatabaseType::YashanDB => IconName::Blocks,
        DatabaseType::HighGo => IconName::Feather,
        DatabaseType::UXDB => IconName::Lock,
        DatabaseType::GBase8c => IconName::Server,
        DatabaseType::EnterpriseDB => IconName::Building,
        DatabaseType::CrateDB => IconName::Boxes,
        DatabaseType::Materialize => IconName::Workflow,
        DatabaseType::AlloyDB => IconName::DatabaseZap,
        DatabaseType::CloudSQLPG => IconName::Cloud,
        DatabaseType::FujitsuPG => IconName::Building,
    }
}

#[derive(IntoElement)]
pub struct ConnectionDialog {
    db_type: DatabaseType,
    name_input: Entity<InputState>,
    host_input: Entity<InputState>,
    port_input: Entity<InputState>,
    database_input: Entity<InputState>,
    user_input: Entity<InputState>,
    pass_input: Entity<InputState>,
    is_read_only: bool,
    is_testing: bool,
    is_editing: bool,
    test_result: Option<Result<String, String>>,
    on_select_type: Option<Rc<dyn Fn(DatabaseType, &mut Window, &mut App) + 'static>>,
    on_toggle_read_only: Option<Rc<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
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
            is_read_only: false,
            is_testing: false,
            is_editing: false,
            test_result: None,
            on_select_type: None,
            on_toggle_read_only: None,
            on_test: None,
            on_save: None,
            on_cancel: None,
        }
    }

    pub fn editing(mut self, is_editing: bool) -> Self {
        self.is_editing = is_editing;
        self
    }

    pub fn read_only(mut self, is_read_only: bool) -> Self {
        self.is_read_only = is_read_only;
        self
    }

    pub fn on_toggle_read_only<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_read_only = Some(Rc::new(handler));
        self
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
            .label(if self.is_editing { "Save Changes" } else { "Save & Connect" });
        if let Some(on_save) = self.on_save {
            save_btn = save_btn.on_click(move |_, window, cx| {
                on_save(window, cx);
            });
        }

        // Dropdown database engine selector supporting all 36 engines
        let current_db_type = self.db_type;
        let on_select_type = self.on_select_type.clone();

        let embedded_dbs = [DatabaseType::Sqlite];

        let mysql_ecosystem_dbs = [
            DatabaseType::Mysql,
            DatabaseType::MariaDB,
            DatabaseType::TiDB,
            DatabaseType::OceanBase,
            DatabaseType::StarRocks,
            DatabaseType::Doris,
            DatabaseType::PolarDB,
            DatabaseType::TDSQL,
            DatabaseType::SelectDB,
            DatabaseType::Databend,
            DatabaseType::GoldenDB,
            DatabaseType::SingleStore,
            DatabaseType::ManticoreSearch,
            DatabaseType::CloudSQLMySQL,
        ];

        let postgres_ecosystem_dbs = [
            DatabaseType::Postgres,
            DatabaseType::CockroachDB,
            DatabaseType::TimescaleDB,
            DatabaseType::Redshift,
            DatabaseType::YugabyteDB,
            DatabaseType::OpenGauss,
            DatabaseType::Kingbase,
            DatabaseType::GaussDB,
            DatabaseType::Greenplum,
            DatabaseType::QuestDB,
            DatabaseType::Vastbase,
            DatabaseType::YashanDB,
            DatabaseType::HighGo,
            DatabaseType::UXDB,
            DatabaseType::GBase8c,
            DatabaseType::EnterpriseDB,
            DatabaseType::CrateDB,
            DatabaseType::Materialize,
            DatabaseType::AlloyDB,
            DatabaseType::CloudSQLPG,
            DatabaseType::FujitsuPG,
        ];

        let family_badge_label = match current_db_type.family() {
            DatabaseFamily::Sqlite => "SQLite Engine",
            DatabaseFamily::MySql => "MySQL Protocol",
            DatabaseFamily::Postgres => "PostgreSQL Protocol",
        };

        let protocol_desc = match current_db_type.family() {
            DatabaseFamily::Sqlite => "Embedded local database · Fast file or in-memory SQLite engine",
            DatabaseFamily::MySql => "Native MySQL wire protocol · Compatible with MariaDB, TiDB, Doris, StarRocks & more",
            DatabaseFamily::Postgres => "Native PostgreSQL wire protocol · Compatible with CockroachDB, Timescale, openGauss & more",
        };

        let on_sel_for_menu = on_select_type.clone();
        let db_dropdown_btn = Button::new("db_type_select_btn")
            .outline()
            .w_full()
            .dropdown_caret(true)
            .icon(database_icon(current_db_type))
            .label(format!("{}  ·  {}", current_db_type.display_name(), current_db_type.category().display_name()))
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _window, _cx| {
                menu = menu
                    .scrollable(true)
                    .max_h(px(380.0))
                    .min_w(px(520.0));

                menu = menu.label("Embedded / Local");
                for dtype in embedded_dbs {
                    let is_curr = dtype == current_db_type;
                    let on_sel = on_sel_for_menu.clone();
                    menu = menu.item(
                        PopupMenuItem::new(dtype.display_name())
                            .icon(database_icon(dtype))
                            .checked(is_curr)
                            .on_click(move |_, window, cx| {
                                if let Some(ref handler) = on_sel {
                                    handler(dtype, window, cx);
                                }
                            }),
                    );
                }

                menu = menu.separator();
                menu = menu.label("MySQL Wire Protocol Ecosystem");
                for dtype in mysql_ecosystem_dbs {
                    let is_curr = dtype == current_db_type;
                    let on_sel = on_sel_for_menu.clone();
                    menu = menu.item(
                        PopupMenuItem::new(dtype.display_name())
                            .icon(database_icon(dtype))
                            .checked(is_curr)
                            .on_click(move |_, window, cx| {
                                if let Some(ref handler) = on_sel {
                                    handler(dtype, window, cx);
                                }
                            }),
                    );
                }

                menu = menu.separator();
                menu = menu.label("PostgreSQL Wire Protocol Ecosystem");
                for dtype in postgres_ecosystem_dbs {
                    let is_curr = dtype == current_db_type;
                    let on_sel = on_sel_for_menu.clone();
                    menu = menu.item(
                        PopupMenuItem::new(dtype.display_name())
                            .icon(database_icon(dtype))
                            .checked(is_curr)
                            .on_click(move |_, window, cx| {
                                if let Some(ref handler) = on_sel {
                                    handler(dtype, window, cx);
                                }
                            }),
                    );
                }

                menu
            });

        let engine_selector = v_flex()
            .gap_1p5()
            .w_full()
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child("Database Engine / Ecosystem"),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(ThemeColors::BG_SURFACE_HOVER)
                                    .text_size(px(10.0))
                                    .text_color(ThemeColors::TEXT_FAINT)
                                    .child("36 Supported"),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(ThemeColors::PRIMARY_BG)
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::PRIMARY_LIGHT)
                                    .child(family_badge_label),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(ThemeColors::BG_SURFACE_HOVER)
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(current_db_type.category().display_name()),
                            ),
                    ),
            )
            .child(db_dropdown_btn)
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .px_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(protocol_desc),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(format!("Default Port: {}", if current_db_type.is_file_based() { "N/A".to_string() } else { current_db_type.default_port().to_string() })),
                    ),
            );

        // Form fields depending on database type
        let form_fields = if self.db_type.is_file_based() {
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
        } else {
            let default_port = self.db_type.default_port().to_string();
            let default_user = self.db_type.default_user();
            let default_db = self.db_type.default_database();

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
                                .w(px(110.0))
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
                                .child(format!("Database Name ({default_db})")),
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
                                        .child(format!("Username ({default_user})")),
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

        let on_toggle_ro = self.on_toggle_read_only.clone();
        let cur_ro = self.is_read_only;

        let read_only_row = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .p_2p5()
            .rounded_lg()
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(if cur_ro {
                ThemeColors::WARNING
            } else {
                ThemeColors::BORDER
            })
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Shield)
                            .size(px(16.0))
                            .text_color(if cur_ro {
                                ThemeColors::WARNING
                            } else {
                                ThemeColors::TEXT_MUTED
                            }),
                    )
                    .child(
                        v_flex()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if cur_ro {
                                        ThemeColors::WARNING
                                    } else {
                                        ThemeColors::TEXT_PRIMARY
                                    })
                                    .child("Read-Only Protection"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_FAINT)
                                    .child("Block destructive queries (DROP, DELETE, UPDATE, TRUNCATE)"),
                            ),
                    ),
            )
            .child({
                let mut btn = Button::new("toggle_ro_btn").xsmall();
                if cur_ro {
                    btn = btn.primary().label("Enabled");
                } else {
                    btn = btn.ghost().label("Disabled");
                }
                if let Some(handler) = on_toggle_ro {
                    btn = btn.on_click(move |_, window, cx| {
                        handler(!cur_ro, window, cx);
                    });
                }
                btn
            });

        // Dialog container modal
        let modal = v_flex()
            .w(px(560.0))
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
                                            .child(if self.is_editing {
                                                "Edit Connection Profile"
                                            } else {
                                                "New Database Connection"
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child(if self.is_editing {
                                                "Modify connection settings and credentials"
                                            } else {
                                                "Configure a new database profile to connect"
                                            }),
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
                    .child(read_only_row)
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
