//! Sidebar navigation displaying saved connection profiles and database schema tree.

use crate::db::types::{ConnectionConfig, DatabaseFamily, DatabaseType, TableInfo};
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem},
    resizable::{resizable_panel, v_resizable, ResizableState},
};
use gpui_kit::gpui::{
    Anchor, App, Context, ElementId, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _,
    px,
};
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(IntoElement)]
pub struct Sidebar {
    connections: Vec<ConnectionConfig>,
    active_connection_id: Option<String>,
    active_tables: Vec<TableInfo>,
    connection_filter: Entity<InputState>,
    table_filter: Entity<InputState>,
    split_state: Entity<ResizableState>,
    selected_table: Option<String>,
    on_new_connection: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_select_connection: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_select_table: Option<Rc<dyn Fn(TableInfo, &mut Window, &mut App) + 'static>>,
    on_disconnect: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_refresh: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_quick_query: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_edit_connection: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_duplicate_connection: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_delete_connection: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_create_table: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Sidebar {
    pub fn new(
        connections: Vec<ConnectionConfig>,
        active_connection_id: Option<String>,
        active_tables: Vec<TableInfo>,
        connection_filter: &Entity<InputState>,
        table_filter: &Entity<InputState>,
        split_state: &Entity<ResizableState>,
    ) -> Self {
        Self {
            connections,
            active_connection_id,
            active_tables,
            connection_filter: connection_filter.clone(),
            table_filter: table_filter.clone(),
            split_state: split_state.clone(),
            selected_table: None,
            on_new_connection: None,
            on_select_connection: None,
            on_select_table: None,
            on_disconnect: None,
            on_refresh: None,
            on_quick_query: None,
            on_edit_connection: None,
            on_duplicate_connection: None,
            on_delete_connection: None,
            on_create_table: None,
        }
    }

    pub fn selected_table(mut self, table: Option<String>) -> Self {
        self.selected_table = table;
        self
    }

    pub fn on_create_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_create_table = Some(Rc::new(handler));
        self
    }

    pub fn on_new_connection<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_new_connection = Some(Rc::new(handler));
        self
    }

    pub fn on_select_connection<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_select_connection = Some(Rc::new(handler));
        self
    }

    pub fn on_select_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(TableInfo, &mut Window, &mut App) + 'static,
    {
        self.on_select_table = Some(Rc::new(handler));
        self
    }

    pub fn on_disconnect<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_disconnect = Some(Rc::new(handler));
        self
    }

    pub fn on_refresh<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_refresh = Some(Rc::new(handler));
        self
    }

    pub fn on_quick_query<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_quick_query = Some(Rc::new(handler));
        self
    }

    pub fn on_edit_connection<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_edit_connection = Some(Rc::new(handler));
        self
    }

    pub fn on_duplicate_connection<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_duplicate_connection = Some(Rc::new(handler));
        self
    }

    pub fn on_delete_connection<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_delete_connection = Some(Rc::new(handler));
        self
    }

    fn render_connection_menu(
        conn: &ConnectionConfig,
        is_active: bool,
        on_disconnect: &Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
        on_select: &Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
        on_edit: &Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
        on_duplicate: &Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
        on_delete: &Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
        menu: PopupMenu,
        _window: &mut Window,
        _cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let mut menu = menu;
        let conn_id = conn.id.clone();

        if is_active {
            let disc = on_disconnect.clone();
            menu = menu.item(
                PopupMenuItem::new("Close Connection")
                    .icon(IconName::Power)
                    .on_click(move |_, window, cx| {
                        if let Some(ref handler) = disc {
                            handler(window, cx);
                        }
                    }),
            );
        } else {
            let sel = on_select.clone();
            let cid = conn_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Connect")
                    .icon(IconName::Power)
                    .on_click(move |_, window, cx| {
                        if let Some(ref handler) = sel {
                            handler(cid.clone(), window, cx);
                        }
                    }),
            );
        }

        let edit = on_edit.clone();
        let cid_edit = conn_id.clone();
        menu = menu.item(
            PopupMenuItem::new("Edit")
                .icon(IconName::Pencil)
                .on_click(move |_, window, cx| {
                    if let Some(ref handler) = edit {
                        handler(cid_edit.clone(), window, cx);
                    }
                }),
        );

        let dup = on_duplicate.clone();
        let cid_dup = conn_id.clone();
        menu = menu.item(
            PopupMenuItem::new("Duplicate")
                .icon(IconName::Copy)
                .on_click(move |_, window, cx| {
                    if let Some(ref handler) = dup {
                        handler(cid_dup.clone(), window, cx);
                    }
                }),
        );

        menu = menu.separator();

        let del = on_delete.clone();
        let cid_del = conn_id.clone();
        let del_icon = Icon::new(IconName::Trash).text_color(ThemeColors::ERROR);
        menu = menu.item(
            PopupMenuItem::element(|_, _| {
                div().text_color(ThemeColors::ERROR).child("Delete")
            })
            .icon(del_icon)
            .on_click(move |_, window, cx| {
                if let Some(ref handler) = del {
                    handler(cid_del.clone(), window, cx);
                }
            }),
        );

        menu
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_new_action = self.on_new_connection.clone();
        let on_disconnect_action = self.on_disconnect.clone();
        let on_select_action = self.on_select_connection.clone();
        let on_edit_action = self.on_edit_connection.clone();
        let on_duplicate_action = self.on_duplicate_connection.clone();
        let on_delete_action = self.on_delete_connection.clone();

        let mut new_btn = Button::new("new_conn")
            .primary()
            .xsmall()
            .icon(IconName::Plus)
            .label("New")
            .tooltip("New Connection Profile");

        if let Some(ref on_new) = on_new_action {
            let on_new = on_new.clone();
            new_btn = new_btn.on_click(move |_, window, cx| {
                on_new(window, cx);
            });
        }

        let mut refresh_btn = Button::new("refresh_sidebar")
            .ghost()
            .xsmall()
            .icon(IconName::RotateCw)
            .tooltip("Refresh Tables");

        if let Some(on_refresh) = self.on_refresh {
            refresh_btn = refresh_btn.on_click(move |_, window, cx| {
                on_refresh(window, cx);
            });
        }

        let mut disconnect_btn = Button::new("disconnect_sidebar")
            .ghost()
            .xsmall()
            .icon(IconName::Unplug)
            .tooltip("Disconnect");

        if let Some(ref on_disconnect) = on_disconnect_action {
            let on_disconnect = on_disconnect.clone();
            disconnect_btn = disconnect_btn.on_click(move |_, window, cx| {
                on_disconnect(window, cx);
            });
        }

        let header = h_flex()
            .h(px(40.0))
            .w_full()
            .px_3()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Icon::new(IconName::Database)
                            .size(px(14.0))
                            .text_color(ThemeColors::PRIMARY_BORDER),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child("CONNECTIONS"),
                    )
                    .child(
                        div()
                            .px_1()
                            .rounded_sm()
                            .bg(ThemeColors::BG_SURFACE_HOVER)
                            .text_size(px(10.0))
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(self.connections.len().to_string()),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .when(self.active_connection_id.is_some(), |this| {
                        this.child(refresh_btn).child(disconnect_btn)
                    })
                    .child(new_btn),
            );

        // Connections search bar
        let conn_search_row = h_flex()
            .w_full()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(Input::new(&self.connection_filter).small().w_full());

        // Filter connections by search query
        let conn_query = self.connection_filter.read(cx).value().to_string();
        let conn_query = conn_query.trim().to_lowercase();
        let filtered_connections: Vec<&ConnectionConfig> = self
            .connections
            .iter()
            .filter(|c| {
                conn_query.is_empty()
                    || c.name.to_lowercase().contains(&conn_query)
                    || c.database.to_lowercase().contains(&conn_query)
            })
            .collect();

        // Connections list
        let mut conn_list = v_flex()
            .id("sidebar_conn_scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap_0p5()
            .p_1();

        if filtered_connections.is_empty() {
            conn_list = conn_list.child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(ThemeColors::TEXT_FAINT)
                    .child(if conn_query.is_empty() {
                        "No connections configured"
                    } else {
                        "No matching connections"
                    }),
            );
        }

        for conn in filtered_connections {
            let is_active = self.active_connection_id.as_deref() == Some(&conn.id);

            let engine_icon = match conn.db_type {
                DatabaseType::Sqlite => IconName::Database,
                DatabaseType::Mysql => IconName::Cpu,
                DatabaseType::MariaDB => IconName::Server,
                DatabaseType::TiDB => IconName::Layers,
                DatabaseType::OceanBase => IconName::HardDrive,
                DatabaseType::StarRocks | DatabaseType::Doris => IconName::Activity,
                DatabaseType::PolarDB => IconName::Server,
                DatabaseType::Postgres => IconName::Layers,
                DatabaseType::CockroachDB => IconName::Cpu,
                DatabaseType::TimescaleDB => IconName::Activity,
                DatabaseType::Redshift => IconName::Layers,
                DatabaseType::YugabyteDB => IconName::HardDrive,
                DatabaseType::OpenGauss | DatabaseType::Kingbase => IconName::Server,
                DatabaseType::Greenplum => IconName::Layers,
            };

            let conn_clone = conn.clone();
            let is_conn_active = is_active;
            let on_disc_clone = on_disconnect_action.clone();
            let on_sel_clone = on_select_action.clone();
            let on_edit_clone = on_edit_action.clone();
            let on_dup_clone = on_duplicate_action.clone();
            let on_del_clone = on_delete_action.clone();

            let more_btn = Button::new(ElementId::Name(format!("conn_more_{}", conn.id).into()))
                .ghost()
                .xsmall()
                .icon(IconName::Ellipsis)
                .tooltip("Connection options")
                .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, window, cx| {
                    Self::render_connection_menu(
                        &conn_clone,
                        is_conn_active,
                        &on_disc_clone,
                        &on_sel_clone,
                        &on_edit_clone,
                        &on_dup_clone,
                        &on_del_clone,
                        menu,
                        window,
                        cx,
                    )
                });

            let conn_id_click = conn.id.clone();
            let on_select_click = on_select_action.clone();

            let left_area = h_flex()
                .id(ElementId::Name(format!("conn_left_{}", conn.id).into()))
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .child(
                    div()
                        .w(px(6.0))
                        .h(px(6.0))
                        .rounded_full()
                        .bg(if is_active {
                            ThemeColors::SUCCESS
                        } else {
                            ThemeColors::TEXT_FAINT
                        }),
                )
                .child(
                    Icon::new(engine_icon)
                        .size(px(13.0))
                        .text_color(if is_active {
                            ThemeColors::PRIMARY_BORDER
                        } else {
                            ThemeColors::TEXT_MUTED
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(conn.name.clone()),
                )
                .when(conn.environment.is_production(), |this| {
                    this.child(
                        div()
                            .px_1()
                            .rounded_sm()
                            .bg(ThemeColors::ERROR)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child("PROD"),
                    )
                })
                .when(conn.is_read_only, |this| {
                    this.child(
                        div()
                            .px_1()
                            .rounded_sm()
                            .bg(ThemeColors::WARNING)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child("RO"),
                    )
                })
                .on_click(move |_, window, cx| {
                    if let Some(ref handler) = on_select_click {
                        handler(conn_id_click.clone(), window, cx);
                    }
                });

            let conn_clone_ctx = conn.clone();
            let on_disc_ctx = on_disconnect_action.clone();
            let on_sel_ctx = on_select_action.clone();
            let on_edit_ctx = on_edit_action.clone();
            let on_dup_ctx = on_duplicate_action.clone();
            let on_del_ctx = on_delete_action.clone();

            let conn_item = h_flex()
                .id(ElementId::Name(format!("conn_item_{}", conn.id).into()))
                .w_full()
                .px_2()
                .py_0p5()
                .rounded_md()
                .items_center()
                .justify_between()
                .bg(if is_active {
                    ThemeColors::BG_SURFACE_ACTIVE
                } else {
                    ThemeColors::BG_SURFACE
                })
                .hover(|s| s.bg(ThemeColors::BG_SURFACE_HOVER))
                .child(left_area)
                .child(more_btn)
                .context_menu(move |menu, window, cx| {
                    Self::render_connection_menu(
                        &conn_clone_ctx,
                        is_conn_active,
                        &on_disc_ctx,
                        &on_sel_ctx,
                        &on_edit_ctx,
                        &on_dup_ctx,
                        &on_del_ctx,
                        menu,
                        window,
                        cx,
                    )
                });

            conn_list = conn_list.child(conn_item);
        }

        if self.connections.is_empty() {
            let mut add_conn_bottom = Button::new("add_conn_bottom")
                .outline()
                .xsmall()
                .w_full()
                .icon(IconName::Plus)
                .label("Add Connection");
            if let Some(ref on_new) = on_new_action {
                let on_new = on_new.clone();
                add_conn_bottom = add_conn_bottom.on_click(move |_, window, cx| {
                    on_new(window, cx);
                });
            }
            conn_list = conn_list.child(add_conn_bottom);
        }

        let filter = self.table_filter.read(cx).value().to_string();
        let filter = filter.trim().to_lowercase();
        let filtered_tables: Vec<TableInfo> = if filter.is_empty() {
            self.active_tables.clone()
        } else {
            self.active_tables
                .iter()
                .filter(|t| {
                    t.name.to_lowercase().contains(&filter)
                        || t.schema
                            .as_deref()
                            .is_some_and(|s| s.to_lowercase().contains(&filter))
                })
                .cloned()
                .collect()
        };

        let active_family = self
            .connections
            .iter()
            .find(|c| Some(c.id.as_str()) == self.active_connection_id.as_deref())
            .map(|c| c.db_type.family())
            .unwrap_or(DatabaseFamily::Sqlite);
        let active_database = self
            .connections
            .iter()
            .find(|c| Some(c.id.as_str()) == self.active_connection_id.as_deref())
            .map(|c| c.database.clone())
            .unwrap_or_default();

        let on_quick = self.on_quick_query.clone();

        let mut grouped: BTreeMap<String, (Vec<TableInfo>, Vec<TableInfo>)> = BTreeMap::new();
        for tbl in filtered_tables {
            let schema = tbl
                .schema
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "default".to_string());
            let entry = grouped.entry(schema).or_default();
            if tbl.is_view() {
                entry.1.push(tbl);
            } else {
                entry.0.push(tbl);
            }
        }

        let conn_pane = v_flex()
            .size_full()
            .min_h_0()
            .child(header)
            .child(conn_search_row)
            .child(conn_list);

        let db_name = if active_database.is_empty() {
            family_label(active_family).to_string()
        } else {
            active_database.clone()
        };

        let mut create_tbl_btn = Button::new("db_create_table_btn")
            .ghost()
            .xsmall()
            .icon(IconName::Plus)
            .tooltip("Create New Table");
        if let Some(ref on_create) = self.on_create_table {
            let on_create = on_create.clone();
            create_tbl_btn = create_tbl_btn.on_click(move |_, window, cx| {
                on_create(window, cx);
            });
        }

        let database_row = h_flex()
            .w_full()
            .px_3()
            .py_1p5()
            .items_center()
            .justify_between()
            .gap_2()
            .bg(ThemeColors::BG_SURFACE)
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Icon::new(IconName::Database)
                            .size(px(13.0))
                            .text_color(ThemeColors::WARNING),
                    )
                    .child(
                        div()
                            .px_1()
                            .py_0p5()
                            .rounded_sm()
                            .bg(ThemeColors::PRIMARY_BG)
                            .text_color(ThemeColors::PRIMARY_LIGHT)
                            .text_size(px(10.0))
                            .font_weight(FontWeight::BOLD)
                            .child("DATABASE"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(db_name),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .child(create_tbl_btn)
                    .child(
                        div()
                            .px_1()
                            .py_0p5()
                            .rounded_sm()
                            .bg(ThemeColors::BG_SURFACE_HOVER)
                            .text_size(px(10.0))
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(family_label(active_family)),
                    ),
            );

        let search_row = h_flex()
            .w_full()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .child(Input::new(&self.table_filter).small().w_full());

        let tables_pane = v_flex()
            .size_full()
            .min_h_0()
            .child(database_row)
            .child(search_row)
            .child({
                let mut tbl_list = v_flex()
                    .id("sidebar_tables_scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap_0p5()
                    .p_1();

                for (schema, (tables, views)) in grouped {
                    tbl_list = tbl_list.child(
                        h_flex()
                            .w_full()
                            .px_2()
                            .py_1()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Icon::new(IconName::Folder)
                                    .size(px(13.0))
                                    .text_color(ThemeColors::PRIMARY_LIGHT),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(schema.clone()),
                            ),
                    );

                    tbl_list = tbl_list.child(group_header("TABLES", tables.len(), IconName::Table, self.on_create_table.clone()));
                    for tbl in &tables {
                        tbl_list = tbl_list.child(table_row(
                            tbl,
                            &self.selected_table,
                            active_family,
                            self.on_select_table.clone(),
                            on_quick.clone(),
                        ));
                    }

                    tbl_list = tbl_list.child(group_header("VIEWS", views.len(), IconName::Eye, None));
                    if views.is_empty() {
                        tbl_list = tbl_list.child(empty_group_hint());
                    }
                    for tbl in &views {
                        tbl_list = tbl_list.child(table_row(
                            tbl,
                            &self.selected_table,
                            active_family,
                            self.on_select_table.clone(),
                            on_quick.clone(),
                        ));
                    }
                }

                tbl_list
            });

        let default_conn_height = px(
            (36.0 + 34.0 + (self.connections.len().min(6) as f32) * 31.0 + 6.0)
                .clamp(140.0, 256.0),
        );

        let content = if self.active_connection_id.is_some() {
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(
                    v_resizable("sidebar-split")
                        .with_state(&self.split_state)
                        .child(
                            resizable_panel()
                                .size(default_conn_height)
                                .size_range(px(100.0)..px(600.0))
                                .flex_none()
                                .child(conn_pane),
                        )
                        .child(
                            resizable_panel()
                                .child(tables_pane),
                        ),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(conn_pane)
                .into_any_element()
        };

        v_flex()
            .w(px(260.0))
            .h_full()
            .bg(ThemeColors::BG_APP)
            .border_r_1()
            .border_color(ThemeColors::BORDER)
            .child(content)
    }
}

fn family_label(family: DatabaseFamily) -> &'static str {
    match family {
        DatabaseFamily::Sqlite => "sqlite",
        DatabaseFamily::Postgres => "postgres",
        DatabaseFamily::MySql => "mysql",
    }
}

fn group_header(
    label: &'static str,
    count: usize,
    icon: IconName,
    on_create_table: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    let show_add = label == "TABLES" && on_create_table.is_some();
    let mut add_btn = Button::new("tables_add_btn")
        .ghost()
        .xsmall()
        .icon(IconName::Plus)
        .tooltip("Create New Table");
    if let Some(on_create) = on_create_table {
        add_btn = add_btn.on_click(move |_, window, cx| {
            on_create(window, cx);
        });
    }

    h_flex()
        .w_full()
        .px_2()
        .py_1()
        .items_center()
        .justify_between()
        .child(
            h_flex()
                .items_center()
                .gap_1p5()
                .child(
                    Icon::new(icon)
                        .size(px(12.0))
                        .text_color(ThemeColors::SUCCESS),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child(label),
                ),
        )
        .child(
            h_flex()
                .items_center()
                .gap_1()
                .when(show_add, |this| this.child(add_btn))
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_FAINT)
                        .child(count.to_string()),
                ),
        )
}

fn empty_group_hint() -> impl IntoElement {
    div()
        .px_6()
        .py_1()
        .text_xs()
        .text_color(ThemeColors::TEXT_FAINT)
        .child("No objects found")
}

fn table_row(
    tbl: &TableInfo,
    selected_table: &Option<String>,
    family: DatabaseFamily,
    on_select: Option<Rc<dyn Fn(TableInfo, &mut Window, &mut App) + 'static>>,
    on_quick: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    let is_selected = selected_table.as_deref() == Some(&tbl.name);
    let info = tbl.clone();
    let info_click = info.clone();
    let qualified = tbl.qualified_name(family);
    let qualified_count = qualified.clone();
    let on_quick_select = on_quick.clone();
    let on_quick_count = on_quick.clone();
    let icon_name = if tbl.is_view() {
        IconName::Eye
    } else {
        IconName::Table
    };
    let row_id = format!(
        "tbl_row_{}_{}",
        tbl.schema.as_deref().unwrap_or("default"),
        tbl.name
    );

    h_flex()
        .id(ElementId::Name(row_id.into()))
        .w_full()
        .py_1()
        .px_2()
        .pl_6()
        .rounded_sm()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .bg(if is_selected {
            ThemeColors::PRIMARY
        } else {
            ThemeColors::BG_APP
        })
        .hover(|s| {
            if !is_selected {
                s.bg(ThemeColors::BG_SURFACE_HOVER)
            } else {
                s
            }
        })
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(icon_name)
                        .size(px(13.0))
                        .text_color(if is_selected {
                            ThemeColors::TEXT_PRIMARY
                        } else {
                            ThemeColors::SUCCESS
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child(tbl.name.clone()),
                ),
        )
        .child(
            h_flex()
                .items_center()
                .gap_1()
                .child(
                    Button::new(ElementId::Name(format!("quick_sel_{}", info.name).into()))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Play)
                        .tooltip("SELECT * LIMIT 100")
                        .when_some(on_quick_select, |btn, handler| {
                            let on_sel_quick = on_select.clone();
                            let info_quick = info.clone();
                            btn.on_click(move |_, window, cx| {
                                if let Some(ref sel_h) = on_sel_quick {
                                    sel_h(info_quick.clone(), window, cx);
                                }
                                handler(
                                    format!("SELECT * FROM {qualified} LIMIT 100;"),
                                    window,
                                    cx,
                                );
                            })
                        }),
                )
                .child(
                    Button::new(ElementId::Name(format!("quick_cnt_{}", info.name).into()))
                        .ghost()
                        .xsmall()
                        .tooltip("COUNT(*)")
                        .child("#")
                        .when_some(on_quick_count, |btn, handler| {
                            btn.on_click(move |_, window, cx| {
                                handler(
                                    format!("SELECT COUNT(*) AS total_count FROM {qualified_count};"),
                                    window,
                                    cx,
                                );
                            })
                        }),
                ),
        )
        .on_click(move |_, window, cx| {
            if let Some(ref handler) = on_select {
                handler(info_click.clone(), window, cx);
            }
        })
}
