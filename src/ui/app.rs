//! Main desktop application workspace coordinating navigation, query console, and data inspection.

use crate::db::export::{export_result, ExportFormat, ExportOptions};
use crate::db::handle::ActiveConnection;
use crate::db::history::{QueryHistoryItem, QueryHistoryManager, QueryHistoryStatus};
use crate::db::manager::ConnectionManager;
use crate::db::types::{
    ColumnInfo, ConnectionConfig, DatabaseFamily, DatabaseType, IndexInfo, QueryResult,
    SortDirection, TableInfo,
};
use crate::ui::components::{
    AppStatusBar, ConnectionDialog, DataGrid, QueryConsole, QueryHistoryView, SchemaViewer, Sidebar,
};
use crate::ui::theme::ThemeColors;
use chrono::Utc;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
    input::{InputState, TextareaState},
};
use gpui_kit::gpui::{
    App, AsyncApp, ClipboardItem, Context, Entity, FontWeight, IntoElement, ParentElement, Render,
    Styled, Window, div, prelude::*, px, transparent_black,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTab {
    QueryConsole,
    DataGrid,
    Schema,
    History,
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

    // Query console state
    query_editor: Entity<TextareaState>,
    console_result: Option<QueryResult>,
    console_error: Option<String>,
    is_executing_query: bool,
    status_message: Option<String>,

    // History & DataGrid state
    history_manager: QueryHistoryManager,
    history_filter: String,
    grid_sort_col: Option<usize>,
    grid_sort_dir: Option<SortDirection>,
    grid_page: usize,
    grid_page_size: usize,
    grid_filter: String,
    sidebar_table_filter: String,

    // Dialog state
    dialog_open: bool,
    dialog_db_type: DatabaseType,
    dialog_name_input: Entity<InputState>,
    dialog_host_input: Entity<InputState>,
    dialog_port_input: Entity<InputState>,
    dialog_database_input: Entity<InputState>,
    dialog_user_input: Entity<InputState>,
    dialog_pass_input: Entity<InputState>,
    dialog_is_read_only: bool,
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

        let history_manager = QueryHistoryManager::new();

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
            history_manager,
            history_filter: String::new(),
            grid_sort_col: None,
            grid_sort_dir: None,
            grid_page: 0,
            grid_page_size: 50,
            grid_filter: String::new(),
            sidebar_table_filter: String::new(),
            dialog_open: false,
            dialog_db_type: DatabaseType::Sqlite,
            dialog_name_input,
            dialog_host_input,
            dialog_port_input,
            dialog_database_input,
            dialog_user_input,
            dialog_pass_input,
            dialog_is_read_only: false,
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
                    let tables_res = conn.list_tables(None, None).await.unwrap_or_default();
                    this.update(cx, |app, cx| {
                        let name = conn.config.name.clone();
                        app.active_connection = Some(conn);
                        app.active_tables = tables_res;
                        app.selected_table = None;
                        app.table_data = None;
                        app.schema_columns.clear();
                        app.schema_indexes.clear();
                        app.schema_ddl = None;
                        app.status_message = Some(format!("Connected to {name}"));
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
        self.grid_page = 0;
        self.grid_sort_col = None;
        self.grid_sort_dir = None;
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

    /// Refresh schema / tables for active connection
    pub fn refresh_schema(&mut self, cx: &mut Context<Self>) {
        let Some(conn) = self.active_connection.clone() else {
            return;
        };

        self.status_message = Some("Refreshing schema...".to_string());
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

        let conn_id = Some(conn.config.id.clone());
        let conn_name = Some(conn.config.name.clone());
        let db_type = conn.config.db_type;

        self.is_executing_query = true;
        self.console_error = None;
        self.status_message = Some("Executing query...".to_string());
        cx.notify();

        let sql_for_exec = sql.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let start = std::time::Instant::now();
            let res = conn.execute_query(&sql_for_exec).await;
            let duration = start.elapsed().as_millis() as u64;

            this.update(cx, |app, cx| {
                app.is_executing_query = false;

                let hist_item = QueryHistoryItem {
                    id: Uuid::new_v4().to_string(),
                    query_text: sql_for_exec.clone(),
                    timestamp: Utc::now(),
                    execution_duration_ms: duration,
                    status: if res.is_ok() {
                        QueryHistoryStatus::Success
                    } else {
                        QueryHistoryStatus::Error
                    },
                    rows_affected: res.as_ref().ok().and_then(|r| r.rows_affected),
                    rows_returned: res.as_ref().ok().map(|r| r.rows.len()),
                    error_message: res.as_ref().err().map(|e| e.to_string()),
                    connection_id: conn_id.clone(),
                    connection_name: conn_name.clone(),
                    database_type: Some(db_type),
                };
                app.history_manager.record(hist_item);
                let _ = app.history_manager.save();

                match res {
                    Ok(qr) => {
                        let rows = qr.rows.len();
                        let dur = qr.execution_time_ms.unwrap_or(duration);
                        app.console_result = Some(qr.clone());
                        app.table_data = Some(qr);
                        app.console_error = None;
                        app.status_message = Some(format!("Query completed: {rows} rows returned in {dur}ms"));
                    }
                    Err(err) => {
                        app.console_error = Some(err.to_string());
                        app.status_message = Some("Query execution failed".to_string());
                    }
                }
                cx.notify();
            }).ok();
        }).detach();
    }

    /// Execute a custom query from quick actions, history replay, or schema inspector
    pub fn execute_custom_sql(&mut self, sql: &str, window: &mut Window, cx: &mut Context<Self>) {
        let sql_str = sql.to_string();
        self.query_editor.update(cx, |editor, cx| {
            editor.set_value(&sql_str, window, cx);
        });
        self.active_tab = WorkspaceTab::QueryConsole;
        self.run_query(cx);
    }

    /// Load a SQL string into the editor without executing
    pub fn load_sql_into_editor(&mut self, sql: &str, window: &mut Window, cx: &mut Context<Self>) {
        let sql_str = sql.to_string();
        self.query_editor.update(cx, |editor, cx| {
            editor.set_value(&sql_str, window, cx);
        });
        self.active_tab = WorkspaceTab::QueryConsole;
        self.status_message = Some("Loaded query into editor".to_string());
        cx.notify();
    }

    /// Export DataGrid results to system clipboard
    pub fn export_grid_data(&mut self, format: ExportFormat, cx: &mut Context<Self>) {
        let res = self.table_data.as_ref().or(self.console_result.as_ref());
        let Some(data) = res else {
            self.status_message = Some("No data available to export".to_string());
            cx.notify();
            return;
        };

        let opt = ExportOptions {
            format,
            include_headers: true,
            pretty_json: true,
            table_name: self.selected_table.clone(),
            batch_size: 100,
        };

        let output = export_result(data, &opt);
        let len = output.len();
        cx.write_to_clipboard(ClipboardItem::new_string(output));
        self.status_message = Some(format!("Exported {format:?} copied to clipboard ({len} bytes)"));
        cx.notify();
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
        self.dialog_is_read_only = false;

        self.dialog_name_input.update(cx, |inp, cx| {
            inp.set_value("New Connection", window, cx);
        });
        self.dialog_database_input.update(cx, |inp, cx| {
            inp.set_value(":memory:", window, cx);
        });
        cx.notify();
    }

    /// Close connection dialog
    pub fn close_connection_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog_open = false;
        self.dialog_test_result = None;
        cx.notify();
    }

    /// Set dialog database type
    pub fn set_dialog_db_type(&mut self, db_type: DatabaseType, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog_db_type = db_type;
        self.dialog_test_result = None;

        let default_port = db_type.default_port().to_string();
        let default_db = db_type.default_database();
        let default_user = db_type.default_user();

        if db_type.is_file_based() {
            self.dialog_database_input.update(cx, |inp, cx| {
                inp.set_value(default_db, window, cx);
            });
        } else {
            self.dialog_port_input.update(cx, |inp, cx| {
                inp.set_value(&default_port, window, cx);
            });
            self.dialog_database_input.update(cx, |inp, cx| {
                inp.set_value(default_db, window, cx);
            });
            self.dialog_user_input.update(cx, |inp, cx| {
                inp.set_value(default_user, window, cx);
            });
        }
        cx.notify();
    }

    /// Collect form values from dialog inputs
    fn build_config_from_dialog(&self, cx: &Context<Self>) -> ConnectionConfig {
        let name = self.dialog_name_input.read(cx).value().to_string();
        let name = if name.trim().is_empty() {
            format!("New {}", self.dialog_db_type.display_name())
        } else {
            name
        };

        let mut cfg = match self.dialog_db_type.family() {
            DatabaseFamily::Sqlite => {
                let db_path = self.dialog_database_input.read(cx).value().to_string();
                let path = if db_path.trim().is_empty() { ":memory:".to_string() } else { db_path };
                let mut c = ConnectionConfig::sqlite(name, path);
                c.db_type = self.dialog_db_type;
                c
            }
            DatabaseFamily::Postgres => {
                let host = self.dialog_host_input.read(cx).value().to_string();
                let port_str = self.dialog_port_input.read(cx).value().to_string();
                let port = port_str.trim().parse::<u16>().unwrap_or_else(|_| self.dialog_db_type.default_port());
                let db = self.dialog_database_input.read(cx).value().to_string();
                let user = self.dialog_user_input.read(cx).value().to_string();
                let pass_str = self.dialog_pass_input.read(cx).value().to_string();
                let pass = if pass_str.is_empty() { None } else { Some(pass_str) };
                let mut c = ConnectionConfig::postgres(name, host, port, db, user, pass);
                c.db_type = self.dialog_db_type;
                c
            }
            DatabaseFamily::MySql => {
                let host = self.dialog_host_input.read(cx).value().to_string();
                let port_str = self.dialog_port_input.read(cx).value().to_string();
                let port = port_str.trim().parse::<u16>().unwrap_or_else(|_| self.dialog_db_type.default_port());
                let db = self.dialog_database_input.read(cx).value().to_string();
                let user = self.dialog_user_input.read(cx).value().to_string();
                let pass_str = self.dialog_pass_input.read(cx).value().to_string();
                let pass = if pass_str.is_empty() { None } else { Some(pass_str) };
                let mut c = ConnectionConfig::mysql(name, host, port, db, user, pass);
                c.db_type = self.dialog_db_type;
                c
            }
        };

        cfg.is_read_only = self.dialog_is_read_only;
        cfg
    }

    /// Test the connection configured in dialog
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
                    Ok(status) => {
                        let version = status.server_version.unwrap_or_else(|| "Unknown".to_string());
                        let ping = status.ping_ms.unwrap_or(0);
                        app.dialog_test_result = Some(Ok(format!("Connected! {version} ({ping} ms)")));
                    }
                    Err(err) => {
                        app.dialog_test_result = Some(Err(format!("Connection failed: {err}")));
                    }
                }
                cx.notify();
            }).ok();
        }).detach();
    }

    /// Save connection configured in dialog and connect
    pub fn save_dialog_connection(&mut self, cx: &mut Context<Self>) {
        let config = self.build_config_from_dialog(cx);
        let id = config.id.clone();
        if let Err(err) = self.manager.save_config(config) {
            self.dialog_test_result = Some(Err(format!("Failed to save profile: {err}")));
            cx.notify();
            return;
        }

        self.saved_connections = self.manager.list_configs();
        self.dialog_open = false;
        self.select_connection(&id, cx);
    }
}

impl Render for CrabStudioApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_conn_id = self.active_connection.as_ref().map(|c| c.config.id.clone());
        let is_connected = self.active_connection.is_some();
        let is_read_only = self.active_connection.as_ref().map(|c| c.config.is_read_only).unwrap_or(false);
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
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child("CrabStudio"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_FAINT)
                                .child("Desktop Database Client"),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .when(is_read_only, |this| {
                            this.child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(ThemeColors::WARNING)
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child("READ-ONLY MODE"),
                            )
                        })
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

        let app_handle = cx.entity().clone();

        // Left Sidebar
        let sidebar = Sidebar::new(
            self.saved_connections.clone(),
            active_conn_id,
            self.active_tables.clone(),
        )
        .table_filter(self.sidebar_table_filter.clone())
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
                    this.refresh_schema(cx);
                });
            }
        })
        .on_quick_query({
            let handle = app_handle.clone();
            move |sql, window, cx| {
                handle.update(cx, |this, cx| {
                    this.execute_custom_sql(&sql, window, cx);
                });
            }
        });

        // Tabs navigation bar
        let selected_tbl_label = self.selected_table.as_deref().unwrap_or("None");

        let tabs_bar = h_flex()
            .h(px(38.0))
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
                    .gap_1()
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
                    .child({
                        let is_active = self.active_tab == WorkspaceTab::History;
                        let handle = app_handle.clone();
                        let count = self.history_manager.items().len();
                        let label = format!("History ({count})");
                        Button::new("tab_history")
                            .small()
                            .ghost()
                            .icon(IconName::RotateCw)
                            .label(label)
                            .border_b_2()
                            .border_color(if is_active {
                                ThemeColors::PRIMARY_BORDER
                            } else {
                                transparent_black()
                            })
                            .on_click(move |_, _, cx| {
                                handle.update(cx, |this, cx| {
                                    this.active_tab = WorkspaceTab::History;
                                    cx.notify();
                                });
                            })
                    }),
            );

        // Workspace main content
        let main_content = match self.active_tab {
            WorkspaceTab::QueryConsole => {
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
            WorkspaceTab::DataGrid => {
                let on_sort = {
                    let handle = app_handle.clone();
                    move |col: usize, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            if this.grid_sort_col == Some(col) {
                                this.grid_sort_dir = match this.grid_sort_dir {
                                    None => Some(SortDirection::Ascending),
                                    Some(SortDirection::Ascending) => Some(SortDirection::Descending),
                                    Some(SortDirection::Descending) => None,
                                };
                                if this.grid_sort_dir.is_none() {
                                    this.grid_sort_col = None;
                                }
                            } else {
                                this.grid_sort_col = Some(col);
                                this.grid_sort_dir = Some(SortDirection::Ascending);
                            }
                            cx.notify();
                        });
                    }
                };

                let on_page = {
                    let handle = app_handle.clone();
                    move |page: usize, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.grid_page = page;
                            cx.notify();
                        });
                    }
                };

                let on_export = {
                    let handle = app_handle.clone();
                    move |format: ExportFormat, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.export_grid_data(format, cx);
                        });
                    }
                };

                let grid_data = self.table_data.clone().or_else(|| self.console_result.clone());

                DataGrid::new(grid_data)
                    .table_name(self.selected_table.clone())
                    .page_size(self.grid_page_size)
                    .current_page(self.grid_page)
                    .sort(self.grid_sort_col, self.grid_sort_dir)
                    .filter_keyword(self.grid_filter.clone())
                    .on_sort(on_sort)
                    .on_page_change(on_page)
                    .on_export(on_export)
                    .into_any_element()
            }
            WorkspaceTab::Schema => {
                let on_quick = {
                    let handle = app_handle.clone();
                    move |sql: String, window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.execute_custom_sql(&sql, window, cx);
                        });
                    }
                };

                SchemaViewer::new(
                    self.selected_table.clone(),
                    self.schema_columns.clone(),
                    self.schema_indexes.clone(),
                    self.schema_ddl.clone(),
                )
                .on_quick_query(on_quick)
                .into_any_element()
            }
            WorkspaceTab::History => {
                let on_load = {
                    let handle = app_handle.clone();
                    move |sql: String, window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.load_sql_into_editor(&sql, window, cx);
                        });
                    }
                };

                let on_run = {
                    let handle = app_handle.clone();
                    move |sql: String, window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.execute_custom_sql(&sql, window, cx);
                        });
                    }
                };

                let on_copy = {
                    let handle = app_handle.clone();
                    move |sql: String, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            let len = sql.len();
                            cx.write_to_clipboard(ClipboardItem::new_string(sql));
                            this.status_message = Some(format!("Copied SQL to clipboard ({len} bytes)"));
                            cx.notify();
                        });
                    }
                };

                let on_clear = {
                    let handle = app_handle.clone();
                    move |_: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            let _ = this.history_manager.clear();
                            this.status_message = Some("Query history cleared".to_string());
                            cx.notify();
                        });
                    }
                };

                QueryHistoryView::new(self.history_manager.items().to_vec())
                    .filter_keyword(self.history_filter.clone())
                    .on_load_query(on_load)
                    .on_run_query(on_run)
                    .on_copy_query(on_copy)
                    .on_clear_history(on_clear)
                    .into_any_element()
            }
        };

        // Footer status bar
        let query_row_count = match self.active_tab {
            WorkspaceTab::QueryConsole => self.console_result.as_ref().map(|r| r.rows.len()),
            WorkspaceTab::DataGrid => self.table_data.as_ref().or(self.console_result.as_ref()).map(|r| r.rows.len()),
            WorkspaceTab::Schema => Some(self.schema_columns.len()),
            WorkspaceTab::History => Some(self.history_manager.items().len()),
        };
        let query_duration = self.console_result.as_ref().and_then(|r| r.execution_time_ms);

        let mut status_bar = AppStatusBar::new()
            .connected(is_connected)
            .read_only(is_read_only)
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
            let on_select_type = {
                let handle = app_handle.clone();
                move |db_type, window: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.set_dialog_db_type(db_type, window, cx);
                    });
                }
            };
            let on_toggle_ro = {
                let handle = app_handle.clone();
                move |ro, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.dialog_is_read_only = ro;
                        cx.notify();
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
                        this.save_dialog_connection(cx);
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
                .read_only(self.dialog_is_read_only)
                .testing(self.dialog_is_testing)
                .test_result(self.dialog_test_result.clone())
                .on_select_type(on_select_type)
                .on_toggle_read_only(on_toggle_ro)
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
