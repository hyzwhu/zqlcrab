//! Main desktop application workspace coordinating navigation, query console, and data inspection.

use crate::db::handle::ActiveConnection;
use crate::db::manager::ConnectionManager;
use crate::db::types::{
    ColumnInfo, ConnectionConfig, DatabaseType, IndexInfo, QueryResult, TableInfo,
};
use crate::ui::components::{
    AppStatusBar, ConnectionDialog, DataGrid, QueryConsole, SchemaViewer, Sidebar,
};
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
    input::{InputState, TextareaState},
};
use gpui_kit::gpui::{
    App, AsyncApp, Context, Entity, FontWeight, IntoElement, ParentElement, Render,
    Styled, Window, div, prelude::*, px, transparent_black,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTab {
    QueryConsole,
    DataGrid,
    Schema,
}

pub struct CrabStudioApp {
    manager: ConnectionManager,
    saved_connections: Vec<ConnectionConfig>,
    active_connection: Option<ActiveConnection>,
    active_tables: Vec<TableInfo>,
    selected_table: Option<String>,
    table_data: Option<QueryResult>,
    schema_columns: Vec<ColumnInfo>,
    schema_indexes: Vec<IndexInfo>,
    schema_ddl: Option<String>,
    active_tab: WorkspaceTab,
    query_editor: Entity<TextareaState>,
    console_result: Option<QueryResult>,
    console_error: Option<String>,
    is_executing_query: bool,
    status_message: Option<String>,
    // Dialog state
    dialog_open: bool,
    dialog_db_type: DatabaseType,
    dialog_name_input: Entity<InputState>,
    dialog_host_input: Entity<InputState>,
    dialog_port_input: Entity<InputState>,
    dialog_database_input: Entity<InputState>,
    dialog_user_input: Entity<InputState>,
    dialog_pass_input: Entity<InputState>,
    dialog_is_testing: bool,
    dialog_test_result: Option<Result<String, String>>,
}

impl CrabStudioApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let manager = ConnectionManager::new();

        // If no saved connections exist, add a friendly default SQLite memory database
        if manager.list_configs().is_empty() {
            let sample_cfg = ConnectionConfig::sqlite("Sample SQLite (In-Memory)", ":memory:");
            let _ = manager.save_config(sample_cfg);
        }

        let saved = manager.list_configs();

        let query_editor = cx.new(|cx| {
            TextareaState::new(window, cx).default_value(
                "-- CrabStudio SQL Workspace\n-- Type your SQL queries here and press ⌘↵ or Run\nSELECT 1 AS id, 'Welcome to CrabStudio' AS message;\n",
            )
        });

        let dialog_name_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("My Database")
        });
        let dialog_host_input = cx.new(|cx| {
            InputState::new(window, cx).default_value("127.0.0.1")
        });
        let dialog_port_input = cx.new(|cx| {
            InputState::new(window, cx).default_value("5432")
        });
        let dialog_database_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(":memory:")
        });
        let dialog_user_input = cx.new(|cx| {
            InputState::new(window, cx).default_value("postgres")
        });
        let dialog_pass_input = cx.new(|cx| {
            InputState::new(window, cx).masked(true)
        });

        Self {
            manager,
            saved_connections: saved,
            active_connection: None,
            active_tables: Vec::new(),
            selected_table: None,
            table_data: None,
            schema_columns: Vec::new(),
            schema_indexes: Vec::new(),
            schema_ddl: None,
            active_tab: WorkspaceTab::QueryConsole,
            query_editor,
            console_result: None,
            console_error: None,
            is_executing_query: false,
            status_message: Some("Ready".to_string()),
            dialog_open: false,
            dialog_db_type: DatabaseType::Sqlite,
            dialog_name_input,
            dialog_host_input,
            dialog_port_input,
            dialog_database_input,
            dialog_user_input,
            dialog_pass_input,
            dialog_is_testing: false,
            dialog_test_result: None,
        }
    }

    /// Select and connect to a saved profile
    pub fn select_connection(&mut self, conn_id: &str, cx: &mut Context<Self>) {
        let Some(config) = self.manager.get_config(conn_id) else {
            return;
        };

        self.status_message = Some(format!("Connecting to {}...", config.name));
        cx.notify();

        let config_clone = config.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let conn_res = ActiveConnection::connect_config(config_clone).await;
            match conn_res {
                Ok(conn) => {
                    // If SQLite :memory:, seed demo tables for high-value immediate feedback
                    if conn.config.db_type == DatabaseType::Sqlite && conn.config.database == ":memory:" {
                        let _ = conn.execute_query(
                            "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT NOT NULL, email TEXT UNIQUE, created_at DATETIME DEFAULT CURRENT_TIMESTAMP);\n\
                             INSERT INTO users (username, email) VALUES ('alice', 'alice@crabstudio.io'), ('bob', 'bob@crabstudio.io'), ('charlie', 'charlie@crabstudio.io');\n\
                             CREATE TABLE IF NOT EXISTS products (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, price REAL NOT NULL, stock INTEGER DEFAULT 0);\n\
                             INSERT INTO products (name, price, stock) VALUES ('Ferris Plushie', 29.99, 100), ('Rust Mechanical Keyboard', 149.50, 42), ('GPU Monitor', 399.00, 15);"
                        ).await;
                    }

                    let tables = conn.list_tables(None, None).await.unwrap_or_default();
                    let first_table = tables.first().map(|t| t.name.clone());

                    let mut initial_data = None;
                    let mut cols = Vec::new();
                    let mut idxs = Vec::new();
                    let mut ddl = None;

                    if let Some(ref tbl) = first_table {
                        initial_data = conn.execute_query(&format!("SELECT * FROM \"{tbl}\" LIMIT 100")).await.ok();
                        cols = conn.list_columns(None, None, tbl).await.unwrap_or_default();
                        idxs = conn.list_indexes(None, None, tbl).await.unwrap_or_default();
                        ddl = conn.get_table_ddl(None, None, tbl).await.ok().flatten();
                    }

                    this.update(cx, |app, cx| {
                        app.active_connection = Some(conn);
                        app.active_tables = tables;
                        app.selected_table = first_table;
                        app.table_data = initial_data;
                        app.schema_columns = cols;
                        app.schema_indexes = idxs;
                        app.schema_ddl = ddl;
                        app.status_message = Some(format!("Connected to {}", config.name));
                        cx.notify();
                    }).ok();
                }
                Err(err) => {
                    this.update(cx, |app, cx| {
                        app.status_message = Some(format!("Connection error: {err}"));
                        cx.notify();
                    }).ok();
                }
            }
        }).detach();
    }

    /// Select a table from the sidebar
    pub fn select_table(&mut self, table_name: &str, cx: &mut Context<Self>) {
        self.selected_table = Some(table_name.to_string());
        self.status_message = Some(format!("Loading table {table_name}..."));
        cx.notify();

        let tbl = table_name.to_string();
        let Some(conn) = self.active_connection.clone() else {
            return;
        };

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let data_res = conn.execute_query(&format!("SELECT * FROM \"{tbl}\" LIMIT 100")).await;
            let cols = conn.list_columns(None, None, &tbl).await.unwrap_or_default();
            let idxs = conn.list_indexes(None, None, &tbl).await.unwrap_or_default();
            let ddl = conn.get_table_ddl(None, None, &tbl).await.ok().flatten();

            this.update(cx, |app, cx| {
                app.table_data = data_res.ok();
                app.schema_columns = cols;
                app.schema_indexes = idxs;
                app.schema_ddl = ddl;
                if app.active_tab == WorkspaceTab::QueryConsole {
                    app.active_tab = WorkspaceTab::DataGrid;
                }
                app.status_message = Some(format!("Loaded table {tbl}"));
                cx.notify();
            }).ok();
        }).detach();
    }

    /// Disconnect from the active database
    pub fn disconnect(&mut self, cx: &mut Context<Self>) {
        if let Some(conn) = self.active_connection.take() {
            let name = conn.config.name.clone();
            cx.spawn(async move |_, _| {
                let _ = conn.disconnect().await;
            }).detach();
            self.active_tables.clear();
            self.selected_table = None;
            self.table_data = None;
            self.schema_columns.clear();
            self.schema_indexes.clear();
            self.schema_ddl = None;
            self.status_message = Some(format!("Disconnected from {name}"));
            cx.notify();
        }
    }

    /// Refresh tables in the active database
    pub fn refresh_tables(&mut self, cx: &mut Context<Self>) {
        let Some(conn) = self.active_connection.clone() else {
            return;
        };
        self.status_message = Some("Refreshing database schema...".to_string());
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let tables = conn.list_tables(None, None).await.unwrap_or_default();
            this.update(cx, |app, cx| {
                app.active_tables = tables;
                app.status_message = Some("Schema refreshed".to_string());
                cx.notify();
            }).ok();
        }).detach();
    }

    /// Execute the query written in the query editor
    pub fn run_query(&mut self, cx: &mut Context<Self>) {
        let sql = self.query_editor.read(cx).value().to_string();
        if sql.trim().is_empty() {
            return;
        }

        let Some(conn) = self.active_connection.clone() else {
            self.console_error = Some("No active database connection. Please select or create a connection first.".to_string());
            cx.notify();
            return;
        };

        self.is_executing_query = true;
        self.console_error = None;
        self.status_message = Some("Executing query...".to_string());
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let res = conn.execute_query(&sql).await;
            this.update(cx, |app, cx| {
                app.is_executing_query = false;
                match res {
                    Ok(qr) => {
                        let rows = qr.rows.len();
                        let duration = qr.execution_time_ms.unwrap_or(0);
                        app.console_result = Some(qr);
                        app.console_error = None;
                        app.status_message = Some(format!("Query completed: {rows} rows returned in {duration}ms"));
                    }
                    Err(err) => {
                        app.console_error = Some(err.to_string());
                        app.status_message = Some("Query failed".to_string());
                    }
                }
                cx.notify();
            }).ok();
        }).detach();
    }

    /// Clear console editor & results
    pub fn clear_console(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query_editor.update(cx, |editor, cx| {
            editor.set_value("", window, cx);
        });
        self.console_result = None;
        self.console_error = None;
        cx.notify();
    }

    /// Open new connection dialog
    pub fn open_connection_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog_open = true;
        self.dialog_db_type = DatabaseType::Sqlite;
        self.dialog_test_result = None;
        self.dialog_is_testing = false;

        self.dialog_name_input.update(cx, |input, cx| {
            input.set_value("New Connection", window, cx);
        });
        self.dialog_database_input.update(cx, |input, cx| {
            input.set_value(":memory:", window, cx);
        });
        cx.notify();
    }

    /// Close connection dialog
    pub fn close_connection_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog_open = false;
        cx.notify();
    }

    /// Switch database type in connection dialog
    pub fn set_dialog_db_type(&mut self, dtype: DatabaseType, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog_db_type = dtype;
        self.dialog_test_result = None;

        match dtype {
            DatabaseType::Sqlite => {
                self.dialog_name_input.update(cx, |inp, cx| inp.set_value("SQLite DB", window, cx));
                self.dialog_database_input.update(cx, |inp, cx| inp.set_value(":memory:", window, cx));
            }
            DatabaseType::Postgres => {
                self.dialog_name_input.update(cx, |inp, cx| inp.set_value("PostgreSQL Local", window, cx));
                self.dialog_host_input.update(cx, |inp, cx| inp.set_value("127.0.0.1", window, cx));
                self.dialog_port_input.update(cx, |inp, cx| inp.set_value("5432", window, cx));
                self.dialog_database_input.update(cx, |inp, cx| inp.set_value("postgres", window, cx));
                self.dialog_user_input.update(cx, |inp, cx| inp.set_value("postgres", window, cx));
            }
            DatabaseType::Mysql => {
                self.dialog_name_input.update(cx, |inp, cx| inp.set_value("MySQL Local", window, cx));
                self.dialog_host_input.update(cx, |inp, cx| inp.set_value("127.0.0.1", window, cx));
                self.dialog_port_input.update(cx, |inp, cx| inp.set_value("3306", window, cx));
                self.dialog_database_input.update(cx, |inp, cx| inp.set_value("test", window, cx));
                self.dialog_user_input.update(cx, |inp, cx| inp.set_value("root", window, cx));
            }
        }
        cx.notify();
    }

    /// Collect form values from dialog inputs
    fn build_config_from_dialog(&self, cx: &Context<Self>) -> ConnectionConfig {
        let name = self.dialog_name_input.read(cx).value().to_string();
        let name = if name.trim().is_empty() { "Untitled Connection".to_string() } else { name };

        match self.dialog_db_type {
            DatabaseType::Sqlite => {
                let db_path = self.dialog_database_input.read(cx).value().to_string();
                let path = if db_path.trim().is_empty() { ":memory:".to_string() } else { db_path };
                ConnectionConfig::sqlite(name, path)
            }
            DatabaseType::Postgres => {
                let host = self.dialog_host_input.read(cx).value().to_string();
                let port_str = self.dialog_port_input.read(cx).value().to_string();
                let port = port_str.trim().parse::<u16>().unwrap_or(5432);
                let db = self.dialog_database_input.read(cx).value().to_string();
                let user = self.dialog_user_input.read(cx).value().to_string();
                let pass_str = self.dialog_pass_input.read(cx).value().to_string();
                let pass = if pass_str.is_empty() { None } else { Some(pass_str) };
                ConnectionConfig::postgres(name, host, port, db, user, pass)
            }
            DatabaseType::Mysql => {
                let host = self.dialog_host_input.read(cx).value().to_string();
                let port_str = self.dialog_port_input.read(cx).value().to_string();
                let port = port_str.trim().parse::<u16>().unwrap_or(3306);
                let db = self.dialog_database_input.read(cx).value().to_string();
                let user = self.dialog_user_input.read(cx).value().to_string();
                let pass_str = self.dialog_pass_input.read(cx).value().to_string();
                let pass = if pass_str.is_empty() { None } else { Some(pass_str) };
                ConnectionConfig::mysql(name, host, port, db, user, pass)
            }
        }
    }

    /// Test dialog connection asynchronously
    pub fn test_dialog_connection(&mut self, cx: &mut Context<Self>) {
        let config = self.build_config_from_dialog(cx);
        self.dialog_is_testing = true;
        self.dialog_test_result = None;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let res = ActiveConnection::test_config(&config).await;
            this.update(cx, |app, cx| {
                app.dialog_is_testing = false;
                match res {
                    Ok(stat) => {
                        let ping = stat.ping_ms.unwrap_or(0);
                        let ver = stat.server_version.unwrap_or_else(|| "OK".to_string());
                        app.dialog_test_result = Some(Ok(format!("Connected successfully! Version: {ver}, Ping: {ping}ms")));
                    }
                    Err(err) => {
                        app.dialog_test_result = Some(Err(format!("Connection failed: {err}")));
                    }
                }
                cx.notify();
            }).ok();
        }).detach();
    }

    /// Save and connect from dialog
    pub fn save_and_connect_dialog(&mut self, cx: &mut Context<Self>) {
        let config = self.build_config_from_dialog(cx);
        let _ = self.manager.save_config(config.clone());
        self.saved_connections = self.manager.list_configs();
        self.dialog_open = false;

        self.select_connection(&config.id, cx);
    }
}

impl Render for CrabStudioApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_conn_id = self.active_connection.as_ref().map(|c| c.config.id.clone());
        let is_connected = self.active_connection.is_some();
        let conn_name = self.active_connection.as_ref().map(|c| c.config.name.clone());
        let db_type_str = self.active_connection.as_ref().map(|c| c.config.db_type.to_string());
        let active_status = self.active_connection.as_ref().and_then(|c| c.status.clone());

        // Header TitleBar
        let title_bar = TitleBar::new().child(
            h_flex()
                .size_full()
                .justify_between()
                .items_center()
                .px_3()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::Database)
                                .size(px(16.0))
                                .text_color(ThemeColors::PRIMARY_BORDER),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child("CrabStudio"),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                .text_xs()
                                .text_color(ThemeColors::TEXT_FAINT)
                                .child("v0.1.0"),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .when_some(self.status_message.as_ref(), |this, msg| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(msg.clone()),
                            )
                        }),
                ),
        );

        // Sidebar callbacks
        let sidebar = {
            let app_handle = cx.entity().clone();
            Sidebar::new(
                self.saved_connections.clone(),
                active_conn_id,
                self.active_tables.clone(),
            )
            .selected_table(self.selected_table.clone())
            .on_new_connection({
                let handle = app_handle.clone();
                move |window, cx| {
                    handle.update(cx, |this, cx| {
                        this.open_connection_dialog(window, cx);
                    });
                }
            })
            .on_select_connection({
                let handle = app_handle.clone();
                move |conn_id, _, cx| {
                    handle.update(cx, |this, cx| {
                        this.select_connection(&conn_id, cx);
                    });
                }
            })
            .on_select_table({
                let handle = app_handle.clone();
                move |tbl_name, _, cx| {
                    handle.update(cx, |this, cx| {
                        this.select_table(&tbl_name, cx);
                    });
                }
            })
            .on_disconnect({
                let handle = app_handle.clone();
                move |_, cx| {
                    handle.update(cx, |this, cx| {
                        this.disconnect(cx);
                    });
                }
            })
            .on_refresh({
                let handle = app_handle.clone();
                move |_, cx| {
                    handle.update(cx, |this, cx| {
                        this.refresh_tables(cx);
                    });
                }
            })
        };

        // Main Content Area Tabs
        let tabs_bar = {
            let app_handle = cx.entity().clone();
            let selected_tbl_label = self.selected_table.as_deref().unwrap_or("Table");

            h_flex()
                .w_full()
                .px_2()
                .gap_1()
                .border_b_1()
                .border_color(ThemeColors::BORDER)
                .bg(ThemeColors::BG_SURFACE)
                .child({
                    let is_active = self.active_tab == WorkspaceTab::QueryConsole;
                    let handle = app_handle.clone();
                    Button::new("tab_console")
                        .small()
                        .ghost()
                        .icon(IconName::Terminal)
                        .label("SQL Console")
                        .border_b_2()
                        .border_color(if is_active {
                            ThemeColors::PRIMARY_BORDER
                        } else {
                            transparent_black()
                        })
                        .on_click(move |_, _, cx| {
                            handle.update(cx, |this, cx| {
                                this.active_tab = WorkspaceTab::QueryConsole;
                                cx.notify();
                            });
                        })
                })
                .child({
                    let is_active = self.active_tab == WorkspaceTab::DataGrid;
                    let handle = app_handle.clone();
                    let label = format!("Data ({selected_tbl_label})");
                    Button::new("tab_data")
                        .small()
                        .ghost()
                        .icon(IconName::Table)
                        .label(label)
                        .border_b_2()
                        .border_color(if is_active {
                            ThemeColors::PRIMARY_BORDER
                        } else {
                            transparent_black()
                        })
                        .on_click(move |_, _, cx| {
                            handle.update(cx, |this, cx| {
                                this.active_tab = WorkspaceTab::DataGrid;
                                cx.notify();
                            });
                        })
                })
                .child({
                    let is_active = self.active_tab == WorkspaceTab::Schema;
                    let handle = app_handle.clone();
                    let label = format!("Schema ({selected_tbl_label})");
                    Button::new("tab_schema")
                        .small()
                        .ghost()
                        .icon(IconName::TableProperties)
                        .label(label)
                        .border_b_2()
                        .border_color(if is_active {
                            ThemeColors::PRIMARY_BORDER
                        } else {
                            transparent_black()
                        })
                        .on_click(move |_, _, cx| {
                            handle.update(cx, |this, cx| {
                                this.active_tab = WorkspaceTab::Schema;
                                cx.notify();
                            });
                        })
                })
        };

        // Main Tab Content
        let main_content = match self.active_tab {
            WorkspaceTab::QueryConsole => {
                let app_handle = cx.entity().clone();
                let on_run = {
                    let handle = app_handle.clone();
                    move |_: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.run_query(cx);
                        });
                    }
                };
                let on_clear = {
                    let handle = app_handle.clone();
                    move |window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.clear_console(window, cx);
                        });
                    }
                };

                QueryConsole::new(&self.query_editor)
                    .result(self.console_result.clone())
                    .error(self.console_error.clone())
                    .executing(self.is_executing_query)
                    .on_run(on_run)
                    .on_clear(on_clear)
                    .into_any_element()
            }
            WorkspaceTab::DataGrid => DataGrid::new(self.table_data.clone())
                .table_name(self.selected_table.clone())
                .into_any_element(),
            WorkspaceTab::Schema => SchemaViewer::new(
                self.selected_table.clone(),
                self.schema_columns.clone(),
                self.schema_indexes.clone(),
                self.schema_ddl.clone(),
            )
            .into_any_element(),
        };

        // Footer status bar
        let query_row_count = match self.active_tab {
            WorkspaceTab::QueryConsole => self.console_result.as_ref().map(|r| r.rows.len()),
            WorkspaceTab::DataGrid => self.table_data.as_ref().map(|r| r.rows.len()),
            WorkspaceTab::Schema => Some(self.schema_columns.len()),
        };
        let query_duration = self.console_result.as_ref().and_then(|r| r.execution_time_ms);

        let mut status_bar = AppStatusBar::new()
            .connected(is_connected)
            .status(active_status)
            .query_stats(query_row_count, query_duration);
        if let Some(name) = conn_name {
            status_bar = status_bar.profile(name);
        }
        if let Some(engine) = db_type_str {
            status_bar = status_bar.engine(engine);
        }

        // Connection Dialog modal overlay if open
        let dialog_overlay = if self.dialog_open {
            let app_handle = cx.entity().clone();
            let on_select_type = {
                let handle = app_handle.clone();
                move |dtype: DatabaseType, window: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.set_dialog_db_type(dtype, window, cx);
                    });
                }
            };
            let on_test = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.test_dialog_connection(cx);
                    });
                }
            };
            let on_save = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.save_and_connect_dialog(cx);
                    });
                }
            };
            let on_cancel = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.close_connection_dialog(cx);
                    });
                }
            };

            Some(
                ConnectionDialog::new(
                    self.dialog_db_type,
                    &self.dialog_name_input,
                    &self.dialog_host_input,
                    &self.dialog_port_input,
                    &self.dialog_database_input,
                    &self.dialog_user_input,
                    &self.dialog_pass_input,
                )
                .testing(self.dialog_is_testing)
                .test_result(self.dialog_test_result.clone())
                .on_select_type(on_select_type)
                .on_test(on_test)
                .on_save(on_save)
                .on_cancel(on_cancel),
            )
        } else {
            None
        };

        // Root layout
        v_flex()
            .size_full()
            .bg(ThemeColors::BG_APP)
            .child(title_bar)
            .child(
                h_flex()
                    .flex_1()
                    .w_full()
                    .child(sidebar)
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .child(tabs_bar)
                            .child(div().flex_1().child(main_content)),
                    ),
            )
            .child(status_bar)
            .children(dialog_overlay)
    }
}
