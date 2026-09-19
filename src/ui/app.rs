//! Main desktop application workspace coordinating navigation, query console, and data inspection.

use crate::db::changeset::GridChangeset;
use crate::db::explain::{parse_explain_result, wrap_explain_sql, ExplainPlan};
use crate::db::export::{export_result, ExportFormat, ExportOptions};
use crate::db::handle::ActiveConnection;
use crate::db::history::{QueryHistoryItem, QueryHistoryManager, QueryHistoryStatus};
use crate::db::manager::ConnectionManager;
use crate::db::sql_format::format_sql;
use crate::db::sql_gen::{
    extract_table_from_sql, generate_create_table_sql, generate_review_plan, ColumnDef,
    CreateTableDef, SqlReviewPlan,
};
use crate::db::types::{
    ColumnInfo, ConnectionConfig, DatabaseFamily, DatabaseType, IndexInfo, QueryResult,
    QueryValue, SortDirection, TableInfo,
};
use crate::ui::components::{
    create_table_modal::{CreateTableColumnState, CreateTableModal},
    data_grid::GridCellCoord,
    AppStatusBar, ConnectionDialog, ConsoleBottomTab, DataGrid, ExplainViewMode, QueryConsole,
    QueryHistoryView, SchemaViewer, Sidebar, SqlReviewModal,
};
use crate::ui::theme::ThemeColors;
use chrono::Utc;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
    input::{InputEvent, InputState, TextareaState},
    resizable::ResizableState,
};
use gpui_kit::gpui::{
    App, AsyncApp, ClipboardItem, Context, ElementId, Entity, FontWeight, IntoElement, ParentElement, Render,
    ScrollHandle, Styled, Window, div, point, prelude::*, px, transparent_black,
};
use uuid::Uuid;

gpui_kit::actions!(zqlcrab, [RunQuery, CloseDialog, FormatSql, ExplainQuery, SaveGridChanges, DeleteGridRow, AddNewRow, DuplicateGridRow]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTab {
    QueryConsole,
    DataGrid,
    Schema,
    History,
}

fn parse_edited_query_value(new_text: &str, orig_val: &QueryValue, col_type: &str) -> QueryValue {
    let trimmed = new_text.trim();
    if trimmed.eq_ignore_ascii_case("null") {
        return QueryValue::Null;
    }
    if trimmed.eq_ignore_ascii_case("<auto>") {
        return QueryValue::String("<auto>".to_string());
    }
    if trimmed.eq_ignore_ascii_case("<default>") {
        return QueryValue::String("<default>".to_string());
    }

    let type_lower = col_type.to_lowercase();
    if trimmed.is_empty() && !type_lower.contains("char") && !type_lower.contains("text") {
        return QueryValue::Null;
    }

    match orig_val {
        QueryValue::Null => {
            let type_lower = col_type.to_lowercase();
            if type_lower.contains("int") || type_lower.contains("serial") {
                if let Ok(i) = trimmed.parse::<i64>() {
                    return QueryValue::Int(i);
                }
            } else if type_lower.contains("float")
                || type_lower.contains("double")
                || type_lower.contains("numeric")
                || type_lower.contains("decimal")
            {
                if let Ok(f) = trimmed.parse::<f64>() {
                    return QueryValue::Float(f);
                }
            } else if type_lower.contains("bool") {
                if trimmed == "1" || trimmed.eq_ignore_ascii_case("true") {
                    return QueryValue::Bool(true);
                } else if trimmed == "0" || trimmed.eq_ignore_ascii_case("false") {
                    return QueryValue::Bool(false);
                }
            }
            QueryValue::String(new_text.to_string())
        }
        QueryValue::Bool(_) => {
            if trimmed == "1" || trimmed.eq_ignore_ascii_case("true") {
                QueryValue::Bool(true)
            } else if trimmed == "0" || trimmed.eq_ignore_ascii_case("false") {
                QueryValue::Bool(false)
            } else {
                QueryValue::String(new_text.to_string())
            }
        }
        QueryValue::Int(_) => {
            if let Ok(i) = trimmed.parse::<i64>() {
                QueryValue::Int(i)
            } else {
                QueryValue::String(new_text.to_string())
            }
        }
        QueryValue::Float(_) => {
            if let Ok(f) = trimmed.parse::<f64>() {
                QueryValue::Float(f)
            } else {
                QueryValue::String(new_text.to_string())
            }
        }
        QueryValue::DateTime(_) => QueryValue::DateTime(new_text.to_string()),
        QueryValue::Bytes(_) => QueryValue::String(new_text.to_string()),
        QueryValue::String(_) => QueryValue::String(new_text.to_string()),
    }
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
    console_split: Entity<ResizableState>,
    console_result: Option<QueryResult>,
    console_error: Option<String>,
    explain_plan: Option<ExplainPlan>,
    explain_error: Option<String>,
    is_executing_query: bool,
    is_explaining: bool,
    console_bottom_tab: ConsoleBottomTab,
    explain_view: ExplainViewMode,
    status_message: Option<String>,

    // History & DataGrid state
    history_manager: QueryHistoryManager,
    history_filter: String,
    grid_sort_col: Option<usize>,
    grid_sort_dir: Option<SortDirection>,
    grid_page: usize,
    grid_page_size: usize,
    grid_filter: String,
    grid_selected_cell: Option<GridCellCoord>,
    grid_inspector_open: bool,
    grid_modal_open: bool,
    grid_json_pretty: bool,
    grid_changeset: GridChangeset,
    grid_cell_edit_input: Entity<InputState>,
    grid_scroll_handle: ScrollHandle,
    sql_review_modal_open: bool,
    sql_review_plan: Option<SqlReviewPlan>,
    sql_review_is_executing: bool,
    sql_review_error: Option<String>,
    sql_review_copied: bool,
    sidebar_conn_filter: Entity<InputState>,
    sidebar_table_filter: Entity<InputState>,
    sidebar_split: Entity<ResizableState>,

    // Dialog state
    dialog_open: bool,
    dialog_editing_id: Option<String>,
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

    // Create Table state
    create_table_modal_open: bool,
    create_table_name_input: Entity<InputState>,
    create_table_schema_input: Entity<InputState>,
    create_table_comment_input: Entity<InputState>,
    create_table_columns: Vec<CreateTableColumnState>,
    create_table_is_executing: bool,
    create_table_error: Option<String>,
    create_table_copied: bool,
}

impl CrabStudioApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut manager = ConnectionManager::new();
        manager.ensure_default_presets();
        let saved = manager.list_configs();

        let handle = cx.entity().clone();
        let first_conn_id = saved
            .iter()
            .find(|c| c.name.contains("Docker MySQL"))
            .or_else(|| saved.iter().find(|c| c.db_type == DatabaseType::Sqlite))
            .or_else(|| saved.first())
            .map(|c| c.id.clone());
        if let Some(conn_id) = first_conn_id {
            cx.defer(move |cx| {
                handle.update(cx, |this, cx| {
                    this.select_connection(&conn_id, cx);
                });
            });
        }

        let query_editor = cx.new(|cx| {
            TextareaState::new(window, cx).default_value(
                "-- Press ⌘↵ (Ctrl+Enter) to run · Shift+Alt+F formats SQL\nSELECT 1 AS id, 'Welcome to CrabStudio' AS message;\n",
            )
        });
        let console_split = cx.new(|_cx| ResizableState::default());
        let sidebar_split = cx.new(|_cx| ResizableState::default());
        let sidebar_conn_filter = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search connections…")
        });
        cx.subscribe(&sidebar_conn_filter, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        let sidebar_table_filter = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search tables, views…")
        });
        cx.subscribe(&sidebar_table_filter, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();

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

        let grid_cell_edit_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Edit cell value...")
        });

        let create_table_name_input = cx.new(|cx| {
            InputState::new(window, cx).default_value("new_table")
        });
        let create_table_schema_input = cx.new(|cx| {
            InputState::new(window, cx).default_value("")
        });
        let create_table_comment_input = cx.new(|cx| {
            InputState::new(window, cx).default_value("")
        });

        let col1_name = cx.new(|cx| InputState::new(window, cx).default_value("id"));
        let col1_type = cx.new(|cx| InputState::new(window, cx).default_value("INTEGER"));
        let col1_def = cx.new(|cx| InputState::new(window, cx).default_value(""));

        let col2_name = cx.new(|cx| InputState::new(window, cx).default_value("name"));
        let col2_type = cx.new(|cx| InputState::new(window, cx).default_value("TEXT"));
        let col2_def = cx.new(|cx| InputState::new(window, cx).default_value(""));

        let create_table_columns = vec![
            CreateTableColumnState {
                name: col1_name,
                data_type: col1_type,
                is_primary_key: true,
                is_nullable: false,
                is_auto_increment: true,
                default_val: col1_def,
            },
            CreateTableColumnState {
                name: col2_name,
                data_type: col2_type,
                is_primary_key: false,
                is_nullable: false,
                is_auto_increment: false,
                default_val: col2_def,
            },
        ];

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
            console_split,
            console_result: None,
            console_error: None,
            explain_plan: None,
            explain_error: None,
            is_executing_query: false,
            is_explaining: false,
            console_bottom_tab: ConsoleBottomTab::Results,
            explain_view: ExplainViewMode::Tree,
            status_message: Some("Ready".to_string()),
            history_manager,
            history_filter: String::new(),
            grid_sort_col: None,
            grid_sort_dir: None,
            grid_page: 0,
            grid_page_size: 50,
            grid_filter: String::new(),
            grid_selected_cell: None,
            grid_inspector_open: false,
            grid_modal_open: false,
            grid_json_pretty: true,
            grid_changeset: GridChangeset::new(),
            grid_cell_edit_input,
            grid_scroll_handle: ScrollHandle::default(),
            sql_review_modal_open: false,
            sql_review_plan: None,
            sql_review_is_executing: false,
            sql_review_error: None,
            sql_review_copied: false,
            sidebar_conn_filter,
            sidebar_table_filter,
            sidebar_split,
            dialog_open: false,
            dialog_editing_id: None,
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
            create_table_modal_open: false,
            create_table_name_input,
            create_table_schema_input,
            create_table_comment_input,
            create_table_columns,
            create_table_is_executing: false,
            create_table_error: None,
            create_table_copied: false,
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
                        app.grid_changeset.clear();
                        app.sql_review_modal_open = false;
                        app.sql_review_plan = None;
                        app.schema_columns.clear();
                        app.schema_indexes.clear();
                        app.schema_ddl = None;
                        app.status_message = Some(format!("Connected to {name}"));
                        if let Some(target) = app.active_tables.first().cloned() {
                            app.select_table(target, cx);
                        }
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
    pub fn select_table(&mut self, table: TableInfo, cx: &mut Context<Self>) {
        self.selected_table = Some(table.name.clone());
        self.grid_page = 0;
        self.grid_sort_col = None;
        self.grid_sort_dir = None;
        self.grid_selected_cell = None;
        self.grid_inspector_open = false;
        self.grid_changeset.clear();
        self.sql_review_modal_open = false;
        self.sql_review_plan = None;
        self.status_message = Some(format!("Loading table {}...", table.name));
        cx.notify();

        let tbl = table.name.clone();
        let schema = table.schema.clone();
        let Some(conn) = self.active_connection.clone() else {
            return;
        };
        let family = conn.config.db_type.family();
        let qualified = table.qualified_name(family);

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let data_res = conn.execute_query(&format!("SELECT * FROM {qualified} LIMIT 100")).await;
            let cols = conn.list_columns(None, schema.as_deref(), &tbl).await.unwrap_or_default();
            let idxs = conn.list_indexes(None, schema.as_deref(), &tbl).await.unwrap_or_default();
            let ddl = conn.get_table_ddl(None, schema.as_deref(), &tbl).await.ok().flatten();

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
            self.grid_changeset.clear();
            self.sql_review_modal_open = false;
            self.sql_review_plan = None;
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

        // Automatically detect and select target table from query
        if let Some((schema_opt, tbl_name)) = extract_table_from_sql(&sql) {
            self.selected_table = Some(tbl_name.clone());
            let conn_meta = conn.clone();
            let tbl_for_cols = tbl_name.clone();
            let schema_for_cols = schema_opt.or_else(|| {
                self.active_tables
                    .iter()
                    .find(|t| t.name.eq_ignore_ascii_case(&tbl_for_cols))
                    .and_then(|t| t.schema.clone())
            });

            cx.spawn(async move |this, cx: &mut AsyncApp| {
                let cols = conn_meta.list_columns(None, schema_for_cols.as_deref(), &tbl_for_cols).await.unwrap_or_default();
                let idxs = conn_meta.list_indexes(None, schema_for_cols.as_deref(), &tbl_for_cols).await.unwrap_or_default();
                let ddl = conn_meta.get_table_ddl(None, schema_for_cols.as_deref(), &tbl_for_cols).await.ok().flatten();

                this.update(cx, |app, cx| {
                    if app.selected_table.as_deref() == Some(&tbl_for_cols) {
                        app.schema_columns = cols;
                        app.schema_indexes = idxs;
                        app.schema_ddl = ddl;
                        cx.notify();
                    }
                }).ok();
            }).detach();
        }

        let conn_id = Some(conn.config.id.clone());
        let conn_name = Some(conn.config.name.clone());
        let db_type = conn.config.db_type;

        self.is_executing_query = true;
        self.console_error = None;
        self.console_bottom_tab = ConsoleBottomTab::Results;
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
                        app.grid_selected_cell = None;
                        app.grid_inspector_open = false;
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

    /// Format the SQL currently in the editor.
    pub fn format_editor_sql(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sql = self.query_editor.read(cx).value().to_string();
        if sql.trim().is_empty() {
            return;
        }
        let formatted = format_sql(&sql);
        self.query_editor.update(cx, |editor, cx| {
            editor.set_value(&formatted, window, cx);
        });
        self.status_message = Some("Formatted SQL".to_string());
        cx.notify();
    }

    /// Run EXPLAIN on the editor SQL and show the plan panel.
    pub fn run_explain(&mut self, cx: &mut Context<Self>) {
        let sql = self.query_editor.read(cx).value().to_string();
        if sql.trim().is_empty() {
            return;
        }

        let Some(conn) = self.active_connection.clone() else {
            self.explain_error = Some("No active database connection. Please select or create a connection first.".to_string());
            self.console_bottom_tab = ConsoleBottomTab::Explain;
            cx.notify();
            return;
        };

        let family = conn.config.db_type.family();
        let explain_sql = wrap_explain_sql(&sql, family);
        if explain_sql.is_empty() {
            return;
        }

        self.is_explaining = true;
        self.explain_error = None;
        self.console_bottom_tab = ConsoleBottomTab::Explain;
        self.status_message = Some("Running EXPLAIN…".to_string());
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let res = conn.execute_query(&explain_sql).await;
            this.update(cx, |app, cx| {
                app.is_explaining = false;
                match res {
                    Ok(qr) => {
                        let plan = parse_explain_result(family, &qr);
                        let nodes = plan.node_count();
                        app.explain_plan = Some(plan);
                        app.explain_error = None;
                        app.status_message = Some(format!("Explain completed: {nodes} nodes"));
                    }
                    Err(err) => {
                        app.explain_plan = None;
                        app.explain_error = Some(err.to_string());
                        app.status_message = Some("Explain failed".to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
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

    /// Select a grid cell and sync inspector live editor input
    pub fn select_grid_cell(&mut self, coord: GridCellCoord, window: &mut Window, cx: &mut Context<Self>) {
        self.grid_selected_cell = Some(coord);
        self.grid_inspector_open = true;

        if coord.is_inserted {
            if let Some(val) = self.grid_changeset.get_inserted_cell_value(coord.row_idx, coord.col_idx) {
                let display_str = if val.is_null() {
                    String::new()
                } else {
                    val.to_display_string()
                };
                self.grid_cell_edit_input.update(cx, |inp, cx| {
                    inp.set_value(&display_str, window, cx);
                });
            }
        } else {
            let res = self.table_data.as_ref().or(self.console_result.as_ref());
            if let Some(res) = res {
                if let Some(row) = res.rows.get(coord.row_idx) {
                    if let Some(orig_val) = row.get(coord.col_idx) {
                        let eff_val = self.grid_changeset.get_effective_cell_value(coord.row_idx, coord.col_idx, orig_val);
                        let display_str = if eff_val.is_null() {
                            String::new()
                        } else {
                            eff_val.to_display_string()
                        };
                        self.grid_cell_edit_input.update(cx, |inp, cx| {
                            inp.set_value(&display_str, window, cx);
                        });
                    }
                }
            }
        }
        cx.notify();
    }

    /// Add a new uncommitted row staged for insertion
    pub fn add_new_grid_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_connection.as_ref().is_some_and(|c| c.config.is_read_only) {
            self.status_message = Some("Cannot add row: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }

        let res = self.table_data.as_ref().or(self.console_result.as_ref());
        let col_count = if let Some(r) = res {
            r.columns.len()
        } else if !self.schema_columns.is_empty() {
            self.schema_columns.len()
        } else {
            1
        };

        let mut default_values = Vec::with_capacity(col_count);
        let mut first_editable_col = 0;
        let mut found_editable = false;

        for i in 0..col_count {
            let col_name = res
                .and_then(|r| r.columns.get(i))
                .cloned()
                .or_else(|| self.schema_columns.get(i).map(|c| c.name.clone()))
                .unwrap_or_default();

            let col_meta = self
                .schema_columns
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(&col_name));

            let is_auto = col_meta.map(|c| {
                c.is_auto_increment
                    || c.data_type.to_lowercase().contains("serial")
                    || (c.is_primary_key && (c.data_type.to_lowercase().contains("int") || self.active_connection.as_ref().map(|conn| conn.config.db_type == DatabaseType::Sqlite).unwrap_or(false)))
            }).unwrap_or(false);

            if is_auto {
                default_values.push(QueryValue::String("<auto>".to_string()));
            } else {
                if !found_editable {
                    first_editable_col = i;
                    found_editable = true;
                }
                if let Some(def) = col_meta.and_then(|c| c.default_value.as_ref()) {
                    if def.trim().eq_ignore_ascii_case("null") {
                        default_values.push(QueryValue::Null);
                    } else {
                        default_values.push(QueryValue::String(def.clone()));
                    }
                } else {
                    default_values.push(QueryValue::Null);
                }
            }
        }

        let anchor = if let Some(coord) = self.grid_selected_cell {
            if coord.is_inserted {
                if let Some(ins) = self.grid_changeset.inserted_rows.get(coord.row_idx) {
                    crate::db::changeset::InsertAnchor::AfterInserted(ins.temp_id)
                } else {
                    crate::db::changeset::InsertAnchor::PageEnd(self.grid_page)
                }
            } else {
                crate::db::changeset::InsertAnchor::AfterRow(coord.row_idx)
            }
        } else {
            crate::db::changeset::InsertAnchor::PageEnd(self.grid_page)
        };

        let is_appended_at_end = self.grid_selected_cell.is_none();
        let temp_id = self.grid_changeset.add_inserted_row(default_values, anchor);
        let insert_idx = self
            .grid_changeset
            .inserted_rows
            .iter()
            .position(|r| r.temp_id == temp_id)
            .unwrap_or(0);
        let new_coord = GridCellCoord::inserted(insert_idx, first_editable_col);
        self.grid_selected_cell = Some(new_coord);
        self.grid_inspector_open = true;

        if is_appended_at_end {
            let curr = self.grid_scroll_handle.offset();
            self.grid_scroll_handle.set_offset(point(curr.x, -px(999999.0)));
        }

        let cur_val = self.grid_changeset.get_inserted_cell_value(insert_idx, first_editable_col);
        let display_str = cur_val.map(|v| if v.is_null() { String::new() } else { v.to_display_string() }).unwrap_or_default();
        self.grid_cell_edit_input.update(cx, |inp, cx| {
            inp.set_value(&display_str, window, cx);
        });

        let (updates, deletes, inserts) = self.grid_changeset.change_summary();
        self.status_message = Some(format!(
            "Added new row #{}. Staged: {inserts} new row(s), {updates} update(s), {deletes} deletion(s)",
            insert_idx + 1
        ));
        cx.notify();
    }

    /// Duplicate a selected grid row as an uncommitted inserted row template
    pub fn duplicate_grid_row(&mut self, coord: GridCellCoord, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_connection.as_ref().is_some_and(|c| c.config.is_read_only) {
            self.status_message = Some("Cannot duplicate row: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }

        let res = self.table_data.as_ref().or(self.console_result.as_ref());
        let Some(res) = res else { return; };

        let orig_values: Vec<QueryValue> = if coord.is_inserted {
            if let Some(ins) = self.grid_changeset.inserted_rows.get(coord.row_idx) {
                ins.values.clone()
            } else {
                return;
            }
        } else {
            let Some(row) = res.rows.get(coord.row_idx) else { return; };
            let mut vals = Vec::with_capacity(row.len());
            for (col_idx, orig_val) in row.iter().enumerate() {
                let eff = self.grid_changeset.get_effective_cell_value(coord.row_idx, col_idx, orig_val);
                vals.push(eff.clone());
            }
            vals
        };

        // Prepare values for duplicate insertion, resetting auto-increment / serial primary key columns
        let mut new_row_values = orig_values;
        let mut first_editable_col = 0;
        let mut found_editable = false;

        for (col_idx, val) in new_row_values.iter_mut().enumerate() {
            let col_name = res
                .columns
                .get(col_idx)
                .cloned()
                .or_else(|| self.schema_columns.get(col_idx).map(|c| c.name.clone()))
                .unwrap_or_default();

            let col_meta = self
                .schema_columns
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(&col_name));

            let is_auto = col_meta.map(|c| {
                c.is_auto_increment
                    || c.data_type.to_lowercase().contains("serial")
                    || (c.is_primary_key && (c.data_type.to_lowercase().contains("int") || self.active_connection.as_ref().map(|conn| conn.config.db_type == DatabaseType::Sqlite).unwrap_or(false)))
            }).unwrap_or(false);

            if is_auto {
                *val = QueryValue::String("<auto>".to_string());
            } else if !found_editable {
                first_editable_col = col_idx;
                found_editable = true;
            }
        }

        let anchor = if coord.is_inserted {
            if let Some(ins) = self.grid_changeset.inserted_rows.get(coord.row_idx) {
                crate::db::changeset::InsertAnchor::AfterInserted(ins.temp_id)
            } else {
                crate::db::changeset::InsertAnchor::PageEnd(self.grid_page)
            }
        } else {
            crate::db::changeset::InsertAnchor::AfterRow(coord.row_idx)
        };

        let temp_id = self.grid_changeset.add_inserted_row(new_row_values, anchor);
        let insert_idx = self
            .grid_changeset
            .inserted_rows
            .iter()
            .position(|r| r.temp_id == temp_id)
            .unwrap_or(0);
        let new_coord = GridCellCoord::inserted(insert_idx, first_editable_col);
        self.grid_selected_cell = Some(new_coord);
        self.grid_inspector_open = true;

        let cur_val = self.grid_changeset.get_inserted_cell_value(insert_idx, first_editable_col);
        let display_str = cur_val.map(|v| if v.is_null() { String::new() } else { v.to_display_string() }).unwrap_or_default();
        self.grid_cell_edit_input.update(cx, |inp, cx| {
            inp.set_value(&display_str, window, cx);
        });

        let (updates, deletes, inserts) = self.grid_changeset.change_summary();
        self.status_message = Some(format!(
            "Duplicated row as new row #{}. Staged: {inserts} new row(s), {updates} update(s), {deletes} deletion(s)",
            insert_idx + 1
        ));
        cx.notify();
    }

    /// Discard an uncommitted inserted row
    pub fn discard_inserted_row(&mut self, insert_idx: usize, cx: &mut Context<Self>) {
        self.grid_changeset.remove_inserted_row_by_index(insert_idx);
        if let Some(coord) = self.grid_selected_cell {
            if coord.is_inserted && coord.row_idx == insert_idx {
                self.grid_selected_cell = None;
            } else if coord.is_inserted && coord.row_idx > insert_idx {
                self.grid_selected_cell = Some(GridCellCoord::inserted(coord.row_idx - 1, coord.col_idx));
            }
        }
        let (updates, deletes, inserts) = self.grid_changeset.change_summary();
        self.status_message = Some(format!(
            "Discarded uncommitted row. Staged: {inserts} new row(s), {updates} update(s), {deletes} deletion(s)"
        ));
        cx.notify();
    }

    /// Apply edited value from live editor input to the grid changeset
    pub fn apply_grid_cell_edit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.active_connection.as_ref().is_some_and(|c| c.config.is_read_only) {
            self.status_message = Some("Cannot modify data: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }

        let Some(coord) = self.grid_selected_cell else {
            return;
        };
        let new_text = self.grid_cell_edit_input.read(cx).value().to_string();

        let res = self.table_data.as_ref().or(self.console_result.as_ref());
        let col_type = res
            .and_then(|r| r.column_types.get(coord.col_idx))
            .map(|s| s.as_str())
            .or_else(|| self.schema_columns.get(coord.col_idx).map(|c| c.data_type.as_str()))
            .unwrap_or("");

        if coord.is_inserted {
            let current_val = self
                .grid_changeset
                .get_inserted_cell_value(coord.row_idx, coord.col_idx)
                .cloned()
                .unwrap_or(QueryValue::Null);

            let new_val = parse_edited_query_value(&new_text, &current_val, col_type);
            self.grid_changeset
                .set_inserted_cell_value(coord.row_idx, coord.col_idx, new_val);

            let (updates, deletes, inserts) = self.grid_changeset.change_summary();
            self.status_message = Some(format!(
                "Updated new row #{} cell. Staged: {inserts} new row(s), {updates} update(s), {deletes} deletion(s)",
                coord.row_idx + 1
            ));
            cx.notify();
            return;
        }

        let Some(res) = res else { return; };
        let Some(row) = res.rows.get(coord.row_idx) else { return; };
        let Some(orig_val) = row.get(coord.col_idx) else { return; };
        let col_name = res.columns.get(coord.col_idx).cloned().unwrap_or_else(|| format!("col_{}", coord.col_idx));

        let new_val = parse_edited_query_value(&new_text, orig_val, col_type);

        self.grid_changeset.stage_cell_update(coord.row_idx, coord.col_idx, col_name, orig_val.clone(), new_val);
        let (updates, deletes, inserts) = self.grid_changeset.change_summary();
        self.status_message = Some(format!("Staged change: {updates} update(s), {deletes} deletion(s), {inserts} new row(s) pending"));
        cx.notify();
    }

    /// Set selected cell to NULL
    pub fn set_grid_cell_null(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_connection.as_ref().is_some_and(|c| c.config.is_read_only) {
            self.status_message = Some("Cannot modify data: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }
        let Some(coord) = self.grid_selected_cell else {
            return;
        };

        if coord.is_inserted {
            self.grid_changeset.set_inserted_cell_value(coord.row_idx, coord.col_idx, QueryValue::Null);
            self.grid_cell_edit_input.update(cx, |inp, cx| {
                inp.set_value("", window, cx);
            });
            let (updates, deletes, inserts) = self.grid_changeset.change_summary();
            self.status_message = Some(format!(
                "Set new row #{} cell to NULL. Staged: {inserts} new row(s), {updates} update(s), {deletes} deletion(s)",
                coord.row_idx + 1
            ));
            cx.notify();
            return;
        }

        let res = self.table_data.as_ref().or(self.console_result.as_ref());
        let Some(res) = res else { return; };
        let Some(row) = res.rows.get(coord.row_idx) else { return; };
        let Some(orig_val) = row.get(coord.col_idx) else { return; };
        let col_name = res.columns.get(coord.col_idx).cloned().unwrap_or_else(|| format!("col_{}", coord.col_idx));

        self.grid_changeset.stage_cell_update(coord.row_idx, coord.col_idx, col_name, orig_val.clone(), QueryValue::Null);
        self.grid_cell_edit_input.update(cx, |inp, cx| {
            inp.set_value("", window, cx);
        });
        let (updates, deletes, inserts) = self.grid_changeset.change_summary();
        self.status_message = Some(format!("Staged NULL: {updates} update(s), {deletes} deletion(s), {inserts} new row(s) pending"));
        cx.notify();
    }

    /// Revert a dirty cell to its original value
    pub fn revert_grid_cell(&mut self, row_idx: usize, col_idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.grid_changeset.revert_cell(row_idx, col_idx);
        let res = self.table_data.as_ref().or(self.console_result.as_ref());
        if let Some(res) = res {
            if let Some(row) = res.rows.get(row_idx) {
                if let Some(orig_val) = row.get(col_idx) {
                    let display_str = if orig_val.is_null() {
                        String::new()
                    } else {
                        orig_val.to_display_string()
                    };
                    self.grid_cell_edit_input.update(cx, |inp, cx| {
                        inp.set_value(&display_str, window, cx);
                    });
                }
            }
        }
        let (updates, deletes, inserts) = self.grid_changeset.change_summary();
        self.status_message = Some(format!("Reverted cell. {updates} update(s), {deletes} deletion(s), {inserts} new row(s) pending"));
        cx.notify();
    }

    /// Toggle a row's staged deletion status
    pub fn toggle_delete_grid_row(&mut self, row_idx: usize, cx: &mut Context<Self>) {
        if self.active_connection.as_ref().is_some_and(|c| c.config.is_read_only) {
            self.status_message = Some("Cannot modify data: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }
        let res = self.table_data.as_ref().or(self.console_result.as_ref());
        let Some(res) = res else { return; };
        let Some(row) = res.rows.get(row_idx) else { return; };

        let now_deleted = self.grid_changeset.toggle_delete_row(row_idx, row);
        let (updates, deletes, inserts) = self.grid_changeset.change_summary();
        if now_deleted {
            self.status_message = Some(format!(
                "Marked row #{} for deletion. Total: {updates} update(s), {deletes} deletion(s), {inserts} new row(s)",
                row_idx + 1
            ));
        } else {
            self.status_message = Some(format!(
                "Restored row #{}. Total: {updates} update(s), {deletes} deletion(s), {inserts} new row(s)",
                row_idx + 1
            ));
        }
        cx.notify();
    }

    /// Discard all staged edits and deletions
    pub fn discard_all_grid_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.grid_changeset.clear();
        self.sql_review_modal_open = false;
        self.sql_review_plan = None;
        if let Some(coord) = self.grid_selected_cell {
            if coord.is_inserted {
                self.grid_selected_cell = None;
                self.grid_cell_edit_input.update(cx, |inp, cx| {
                    inp.set_value("", window, cx);
                });
            } else {
                let res = self.table_data.as_ref().or(self.console_result.as_ref());
                if let Some(res) = res {
                    if let Some(row) = res.rows.get(coord.row_idx) {
                        if let Some(orig_val) = row.get(coord.col_idx) {
                            let display_str = if orig_val.is_null() {
                                String::new()
                            } else {
                                orig_val.to_display_string()
                            };
                            self.grid_cell_edit_input.update(cx, |inp, cx| {
                                inp.set_value(&display_str, window, cx);
                            });
                        }
                    }
                }
            }
        }
        self.status_message = Some("Discarded all pending staged changes".to_string());
        cx.notify();
    }

    /// Open the SQL Review and Confirmation Modal
    pub fn open_sql_review_modal(&mut self, cx: &mut Context<Self>) {
        if !self.grid_changeset.is_dirty() {
            self.status_message = Some("No pending changes to review or save".to_string());
            cx.notify();
            return;
        }

        let Some(conn) = self.active_connection.as_ref() else {
            self.status_message = Some("No active database connection".to_string());
            cx.notify();
            return;
        };

        if conn.config.is_read_only {
            self.status_message = Some("Cannot save changes: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }

        let mut table_name = self.selected_table.clone();
        let mut schema_name = None;

        if table_name.is_none() {
            let sql = self.query_editor.read(cx).value().to_string();
            if let Some((sch, tbl)) = extract_table_from_sql(&sql) {
                schema_name = sch;
                table_name = Some(tbl);
            }
        }

        let Some(table_name) = table_name else {
            self.status_message = Some("Cannot determine target table for pending changes. Please select a table from the sidebar.".to_string());
            cx.notify();
            return;
        };

        if schema_name.is_none() {
            schema_name = self.active_tables.iter().find(|t| t.name.eq_ignore_ascii_case(&table_name)).and_then(|t| t.schema.clone());
        }

        let family = conn.config.db_type.family();

        let empty_cols = Vec::new();
        let empty_rows = Vec::new();
        let (grid_cols, orig_rows) = if let Some(ref res) = self.table_data.as_ref().or(self.console_result.as_ref()) {
            (&res.columns, &res.rows)
        } else {
            (&empty_cols, &empty_rows)
        };

        let plan = generate_review_plan(
            &table_name,
            schema_name.as_deref(),
            family,
            &self.schema_columns,
            grid_cols,
            orig_rows,
            &self.grid_changeset,
        );

        self.sql_review_plan = Some(plan);
        self.sql_review_modal_open = true;
        self.sql_review_is_executing = false;
        self.sql_review_error = None;
        self.sql_review_copied = false;
        cx.notify();
    }

    /// Execute the generated SQL review plan atomically
    pub fn execute_sql_review_plan(&mut self, cx: &mut Context<Self>) {
        let Some(plan) = self.sql_review_plan.clone() else {
            return;
        };
        let Some(conn) = self.active_connection.clone() else {
            self.sql_review_error = Some("No active database connection".to_string());
            cx.notify();
            return;
        };

        if conn.config.is_read_only {
            self.sql_review_error = Some("Connection is read-only".to_string());
            cx.notify();
            return;
        }

        self.sql_review_is_executing = true;
        self.sql_review_error = None;
        cx.notify();

        let table_name = Some(plan.table_name.clone());
        let schema_name = plan.schema_name.clone();
        let full_script = plan.full_script.clone();
        let inserts_count = plan.inserts_count;
        let updates_count = plan.updates_count;
        let deletes_count = plan.deletes_count;
        let reload_sql = if self.active_tab == WorkspaceTab::QueryConsole {
            let s = self.query_editor.read(cx).value().trim().to_string();
            if !s.is_empty() {
                Some(s)
            } else {
                None
            }
        } else {
            None
        };

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let res = conn.execute_batch(&full_script).await;
            match res {
                Ok(_) => {
                    let query_sql = if let Some(ref s) = reload_sql {
                        s.clone()
                    } else if let Some(ref tbl) = table_name {
                        let family = conn.config.db_type.family();
                        let qualified = crate::db::types::TableInfo {
                            name: tbl.clone(),
                            schema: schema_name.clone(),
                            table_type: "BASE TABLE".to_string(),
                            comment: None,
                            row_count_estimate: None,
                        }.qualified_name(family);
                        format!("SELECT * FROM {qualified} LIMIT 100")
                    } else {
                        "SELECT 1;".to_string()
                    };
                    let reloaded = conn.execute_query(&query_sql).await.ok();

                    this.update(cx, |app, cx| {
                        app.sql_review_is_executing = false;
                        app.sql_review_modal_open = false;
                        app.sql_review_plan = None;
                        app.grid_changeset.clear();
                        if let Some(qr) = reloaded {
                            app.table_data = Some(qr.clone());
                            app.console_result = Some(qr);
                        }
                        app.status_message = Some(format!(
                            "Successfully applied {inserts_count} insertion(s), {updates_count} update(s), and {deletes_count} deletion(s)"
                        ));
                        cx.notify();
                    }).ok();
                }
                Err(err) => {
                    this.update(cx, |app, cx| {
                        app.sql_review_is_executing = false;
                        app.sql_review_error = Some(err.to_string());
                        app.status_message = Some("Execution failed. Transaction rolled back.".to_string());
                        cx.notify();
                    }).ok();
                }
            }
        }).detach();
    }

    /// Open the create table modal and configure default inputs according to database family
    pub fn open_create_table_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.active_connection.as_ref() else {
            self.status_message = Some("Please connect to a database first before creating a table".to_string());
            cx.notify();
            return;
        };

        if conn.config.is_read_only {
            self.status_message = Some("Cannot create table: connection is in Read-Only mode".to_string());
            cx.notify();
            return;
        }

        let family = conn.config.db_type.family();
        let default_schema = match family {
            DatabaseFamily::Postgres => "public",
            DatabaseFamily::MySql => conn.config.database.as_str(),
            DatabaseFamily::Sqlite => "",
        };

        self.create_table_name_input.update(cx, |inp, cx| {
            inp.set_value("new_table", window, cx);
        });
        self.create_table_schema_input.update(cx, |inp, cx| {
            inp.set_value(default_schema, window, cx);
        });
        self.create_table_comment_input.update(cx, |inp, cx| {
            inp.set_value("", window, cx);
        });

        let (id_type, name_type) = match family {
            DatabaseFamily::Sqlite => ("INTEGER", "TEXT"),
            DatabaseFamily::Postgres => ("SERIAL", "VARCHAR(255)"),
            DatabaseFamily::MySql => ("INT", "VARCHAR(255)"),
        };

        let col1_name = cx.new(|cx| InputState::new(window, cx).default_value("id"));
        let col1_type = cx.new(|cx| InputState::new(window, cx).default_value(id_type));
        let col1_def = cx.new(|cx| InputState::new(window, cx).default_value(""));

        let col2_name = cx.new(|cx| InputState::new(window, cx).default_value("name"));
        let col2_type = cx.new(|cx| InputState::new(window, cx).default_value(name_type));
        let col2_def = cx.new(|cx| InputState::new(window, cx).default_value(""));

        self.create_table_columns = vec![
            CreateTableColumnState {
                name: col1_name,
                data_type: col1_type,
                is_primary_key: true,
                is_nullable: false,
                is_auto_increment: true,
                default_val: col1_def,
            },
            CreateTableColumnState {
                name: col2_name,
                data_type: col2_type,
                is_primary_key: false,
                is_nullable: false,
                is_auto_increment: false,
                default_val: col2_def,
            },
        ];

        self.create_table_modal_open = true;
        self.create_table_is_executing = false;
        self.create_table_error = None;
        self.create_table_copied = false;
        cx.notify();
    }

    /// Close create table modal
    pub fn close_create_table_modal(&mut self, cx: &mut Context<Self>) {
        self.create_table_modal_open = false;
        self.create_table_is_executing = false;
        self.create_table_error = None;
        cx.notify();
    }

    /// Add a new column to the create table designer
    pub fn add_create_table_column(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let family = self
            .active_connection
            .as_ref()
            .map(|c| c.config.db_type.family())
            .unwrap_or(DatabaseFamily::Sqlite);
        let default_type = match family {
            DatabaseFamily::Sqlite => "TEXT",
            DatabaseFamily::Postgres => "VARCHAR(255)",
            DatabaseFamily::MySql => "VARCHAR(255)",
        };
        let col_name_str = format!("col_{}", self.create_table_columns.len() + 1);
        let name_inp = cx.new(|cx| InputState::new(window, cx).default_value(&col_name_str));
        let type_inp = cx.new(|cx| InputState::new(window, cx).default_value(default_type));
        let def_inp = cx.new(|cx| InputState::new(window, cx).default_value(""));

        self.create_table_columns.push(CreateTableColumnState {
            name: name_inp,
            data_type: type_inp,
            is_primary_key: false,
            is_nullable: true,
            is_auto_increment: false,
            default_val: def_inp,
        });
        cx.notify();
    }

    /// Remove a column from the create table designer
    pub fn remove_create_table_column(&mut self, idx: usize, cx: &mut Context<Self>) {
        if self.create_table_columns.len() > 1 && idx < self.create_table_columns.len() {
            self.create_table_columns.remove(idx);
            cx.notify();
        }
    }

    /// Toggle Primary Key flag for a column
    pub fn toggle_create_table_pk(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(col) = self.create_table_columns.get_mut(idx) {
            col.is_primary_key = !col.is_primary_key;
            if col.is_primary_key {
                col.is_nullable = false;
            } else {
                col.is_auto_increment = false;
            }
            cx.notify();
        }
    }

    /// Toggle Nullable constraint for a column
    pub fn toggle_create_table_nullable(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(col) = self.create_table_columns.get_mut(idx) {
            col.is_nullable = !col.is_nullable;
            if col.is_nullable {
                col.is_primary_key = false;
                col.is_auto_increment = false;
            }
            cx.notify();
        }
    }

    /// Toggle Auto-Increment constraint for a column
    pub fn toggle_create_table_auto_inc(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(col) = self.create_table_columns.get_mut(idx) {
            col.is_auto_increment = !col.is_auto_increment;
            if col.is_auto_increment {
                col.is_primary_key = true;
                col.is_nullable = false;
            }
            cx.notify();
        }
    }

    /// Quickly set column data type from presets
    pub fn set_create_table_quick_type(
        &mut self,
        idx: usize,
        data_type: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(col) = self.create_table_columns.get(idx) {
            col.data_type.update(cx, |inp, cx| {
                inp.set_value(&data_type, window, cx);
            });
            cx.notify();
        }
    }

    /// Build CreateTableDef from input states
    pub fn build_create_table_def(&self, cx: &App) -> CreateTableDef {
        let table_name = self.create_table_name_input.read(cx).value().to_string();
        let schema_raw = self.create_table_schema_input.read(cx).value().to_string();
        let schema = if schema_raw.trim().is_empty() {
            None
        } else {
            Some(schema_raw.trim().to_string())
        };
        let comment_raw = self.create_table_comment_input.read(cx).value().to_string();
        let comment = if comment_raw.trim().is_empty() {
            None
        } else {
            Some(comment_raw.trim().to_string())
        };

        let mut def = CreateTableDef::new(table_name)
            .schema(schema)
            .comment(comment);

        for col in &self.create_table_columns {
            let col_name = col.name.read(cx).value().to_string();
            let col_type = col.data_type.read(cx).value().to_string();
            let def_raw = col.default_val.read(cx).value().to_string();
            let default_val = if def_raw.trim().is_empty() {
                None
            } else {
                Some(def_raw.trim().to_string())
            };

            let col_def = ColumnDef::new(col_name, col_type)
                .primary_key(col.is_primary_key)
                .nullable(col.is_nullable)
                .auto_increment(col.is_auto_increment)
                .default_value(default_val);

            def = def.column(col_def);
        }

        def
    }

    /// Generate real-time preview SQL DDL and any validation error
    pub fn get_create_table_preview_sql(&self, cx: &App) -> (String, Option<String>) {
        let family = self
            .active_connection
            .as_ref()
            .map(|c| c.config.db_type.family())
            .unwrap_or(DatabaseFamily::Sqlite);

        let def = self.build_create_table_def(cx);
        match generate_create_table_sql(&def, family) {
            Ok(sql) => (sql, None),
            Err(err) => (format!("-- Validation notice: {err}"), Some(err)),
        }
    }

    /// Execute CREATE TABLE DDL atomically and refresh tables
    pub fn execute_create_table(&mut self, cx: &mut Context<Self>) {
        let (sql, validation_err) = self.get_create_table_preview_sql(cx);
        if let Some(err) = validation_err {
            self.create_table_error = Some(err);
            cx.notify();
            return;
        }

        let Some(conn) = self.active_connection.clone() else {
            self.create_table_error = Some("No active database connection".to_string());
            cx.notify();
            return;
        };

        if conn.config.is_read_only {
            self.create_table_error = Some("Connection is in Read-Only mode".to_string());
            cx.notify();
            return;
        }

        self.create_table_is_executing = true;
        self.create_table_error = None;
        cx.notify();

        let new_table_name = self.create_table_name_input.read(cx).value().trim().to_string();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let res = conn.execute_batch(&sql).await;

            this.update(cx, |app, cx| {
                app.create_table_is_executing = false;
                match res {
                    Ok(_) => {
                        app.create_table_modal_open = false;
                        app.status_message = Some(format!("Table '{new_table_name}' created successfully"));
                        app.refresh_schema(cx);
                        app.active_tab = WorkspaceTab::Schema;
                        cx.notify();
                    }
                    Err(err) => {
                        app.create_table_error = Some(err.to_string());
                        app.status_message = Some(format!("Create table failed: {err}"));
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// Transfer generated DDL to Query Console for custom editing
    pub fn open_create_table_in_console(
        &mut self,
        sql: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.create_table_modal_open = false;
        self.query_editor.update(cx, |editor, cx| {
            editor.set_value(&sql, window, cx);
        });
        self.active_tab = WorkspaceTab::QueryConsole;
        self.status_message = Some("Loaded Create Table DDL into Query Console".to_string());
        cx.notify();
    }

    /// Clear console editor & results
    pub fn clear_console(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query_editor.update(cx, |editor, cx| {
            editor.set_value("", window, cx);
        });
        self.console_result = None;
        self.console_error = None;
        self.explain_plan = None;
        self.explain_error = None;
        cx.notify();
    }

    /// Open new connection dialog
    pub fn open_connection_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog_open = true;
        self.dialog_editing_id = None;
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

    /// Open dialog to edit an existing connection profile
    pub fn open_edit_connection_dialog(&mut self, conn_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(config) = self.manager.get_config(conn_id) else {
            return;
        };

        self.dialog_open = true;
        self.dialog_editing_id = Some(conn_id.to_string());
        self.dialog_db_type = config.db_type;
        self.dialog_test_result = None;
        self.dialog_is_testing = false;
        self.dialog_is_read_only = config.is_read_only;

        let name = config.name.clone();
        self.dialog_name_input.update(cx, |inp, cx| {
            inp.set_value(&name, window, cx);
        });

        if config.db_type.is_file_based() {
            let path = config.database.clone();
            self.dialog_database_input.update(cx, |inp, cx| {
                inp.set_value(&path, window, cx);
            });
        } else {
            let host = config.host.clone();
            let port = config.port.to_string();
            let db = config.database.clone();
            let user = config.username.clone();
            let pass = config.password.clone().unwrap_or_default();

            self.dialog_host_input.update(cx, |inp, cx| {
                inp.set_value(&host, window, cx);
            });
            self.dialog_port_input.update(cx, |inp, cx| {
                inp.set_value(&port, window, cx);
            });
            self.dialog_database_input.update(cx, |inp, cx| {
                inp.set_value(&db, window, cx);
            });
            self.dialog_user_input.update(cx, |inp, cx| {
                inp.set_value(&user, window, cx);
            });
            self.dialog_pass_input.update(cx, |inp, cx| {
                inp.set_value(&pass, window, cx);
            });
        }
        cx.notify();
    }

    /// Duplicate an existing connection profile
    pub fn duplicate_connection(&mut self, conn_id: &str, cx: &mut Context<Self>) {
        let Some(mut config) = self.manager.get_config(conn_id) else {
            return;
        };
        config.id = Uuid::new_v4().to_string();
        config.name = format!("{} (Copy)", config.name);
        let name = config.name.clone();
        if let Err(err) = self.manager.save_config(config) {
            self.status_message = Some(format!("Failed to duplicate profile: {err}"));
            cx.notify();
            return;
        }
        self.saved_connections = self.manager.list_configs();
        self.status_message = Some(format!("Duplicated profile '{name}'"));
        cx.notify();
    }

    /// Delete an existing connection profile
    pub fn delete_connection(&mut self, conn_id: &str, cx: &mut Context<Self>) {
        if self.active_connection.as_ref().is_some_and(|c| c.config.id == conn_id) {
            self.disconnect(cx);
        }
        let config_name = self.manager.get_config(conn_id).map(|c| c.name).unwrap_or_else(|| conn_id.to_string());
        if let Err(err) = self.manager.delete_config(conn_id) {
            self.status_message = Some(format!("Failed to delete profile: {err}"));
            cx.notify();
            return;
        }
        self.saved_connections = self.manager.list_configs();
        self.status_message = Some(format!("Deleted profile '{config_name}'"));
        cx.notify();
    }

    /// Close connection dialog
    pub fn close_connection_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog_open = false;
        self.dialog_editing_id = None;
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
        let mut config = self.build_config_from_dialog(cx);
        let is_edit = self.dialog_editing_id.is_some();
        if let Some(ref edit_id) = self.dialog_editing_id {
            config.id = edit_id.clone();
        }
        let id = config.id.clone();
        if let Err(err) = self.manager.save_config(config) {
            self.dialog_test_result = Some(Err(format!("Failed to save profile: {err}")));
            cx.notify();
            return;
        }

        self.saved_connections = self.manager.list_configs();
        self.dialog_open = false;
        self.dialog_editing_id = None;

        if !is_edit {
            self.select_connection(&id, cx);
        } else {
            self.status_message = Some("Connection profile updated".to_string());
            cx.notify();
        }
    }

    /// Helper to construct a fully-interactive DataGrid bound to the application state
    pub fn build_data_grid(
        &self,
        app_handle: &Entity<Self>,
        grid_data: Option<QueryResult>,
        table_name: Option<String>,
        is_read_only: bool,
    ) -> DataGrid {
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

        let on_select_cell = {
            let handle = app_handle.clone();
            move |coord: GridCellCoord, window: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.select_grid_cell(coord, window, cx);
                });
            }
        };

        let on_add_row = {
            let handle = app_handle.clone();
            move |window: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.add_new_grid_row(window, cx);
                });
            }
        };

        let on_duplicate_row = {
            let handle = app_handle.clone();
            move |coord: GridCellCoord, window: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.duplicate_grid_row(coord, window, cx);
                });
            }
        };

        let on_discard_inserted_row = {
            let handle = app_handle.clone();
            move |insert_idx: usize, _: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.discard_inserted_row(insert_idx, cx);
                });
            }
        };

        let on_toggle_inspector = {
            let handle = app_handle.clone();
            move |open: bool, _: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.grid_inspector_open = open;
                    cx.notify();
                });
            }
        };

        let on_toggle_modal = {
            let handle = app_handle.clone();
            move |open: bool, _: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.grid_modal_open = open;
                    cx.notify();
                });
            }
        };

        let on_toggle_pretty = {
            let handle = app_handle.clone();
            move |pretty: bool, _: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.grid_json_pretty = pretty;
                    cx.notify();
                });
            }
        };

        let on_copy_val = {
            let handle = app_handle.clone();
            move |col_name: String, val: String, _: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    let len = val.len();
                    cx.write_to_clipboard(ClipboardItem::new_string(val));
                    this.status_message = Some(format!("Copied value of column '{col_name}' ({len} chars)"));
                    cx.notify();
                });
            }
        };

        let on_copy_row_json = {
            let handle = app_handle.clone();
            move |row_idx: usize, json_str: String, _: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    let len = json_str.len();
                    cx.write_to_clipboard(ClipboardItem::new_string(json_str));
                    this.status_message = Some(format!("Copied row #{} as JSON ({len} bytes)", row_idx + 1));
                    cx.notify();
                });
            }
        };

        let on_copy_row_tsv = {
            let handle = app_handle.clone();
            move |row_idx: usize, tsv_str: String, _: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    let len = tsv_str.len();
                    cx.write_to_clipboard(ClipboardItem::new_string(tsv_str));
                    this.status_message = Some(format!("Copied row #{} as TSV ({len} bytes)", row_idx + 1));
                    cx.notify();
                });
            }
        };

        let on_apply_cell_edit = {
            let handle = app_handle.clone();
            move |window: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.apply_grid_cell_edit(window, cx);
                });
            }
        };

        let on_set_cell_null = {
            let handle = app_handle.clone();
            move |window: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.set_grid_cell_null(window, cx);
                });
            }
        };

        let on_revert_cell = {
            let handle = app_handle.clone();
            move |row_idx: usize, col_idx: usize, window: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.revert_grid_cell(row_idx, col_idx, window, cx);
                });
            }
        };

        let on_toggle_del_row = {
            let handle = app_handle.clone();
            move |row_idx: usize, _: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.toggle_delete_grid_row(row_idx, cx);
                });
            }
        };

        let on_discard_all = {
            let handle = app_handle.clone();
            move |window: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.discard_all_grid_changes(window, cx);
                });
            }
        };

        let on_save_changes = {
            let handle = app_handle.clone();
            move |_: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.open_sql_review_modal(cx);
                });
            }
        };

        DataGrid::new(grid_data)
            .table_name(table_name)
            .scroll_handle(self.grid_scroll_handle.clone())
            .page_size(self.grid_page_size)
            .current_page(self.grid_page)
            .sort(self.grid_sort_col, self.grid_sort_dir)
            .filter_keyword(self.grid_filter.clone())
            .selected_cell(self.grid_selected_cell)
            .inspector_open(self.grid_inspector_open)
            .modal_open(self.grid_modal_open)
            .json_pretty(self.grid_json_pretty)
            .changeset(self.grid_changeset.clone())
            .read_only(is_read_only)
            .cell_edit_input(Some(self.grid_cell_edit_input.clone()))
            .on_sort(on_sort)
            .on_page_change(on_page)
            .on_export(on_export)
            .on_select_cell(on_select_cell)
            .on_toggle_inspector(on_toggle_inspector)
            .on_toggle_modal(on_toggle_modal)
            .on_toggle_json_pretty(on_toggle_pretty)
            .on_copy_value(on_copy_val)
            .on_copy_row_json(on_copy_row_json)
            .on_copy_row_tsv(on_copy_row_tsv)
            .on_apply_cell_edit(on_apply_cell_edit)
            .on_set_cell_null(on_set_cell_null)
            .on_revert_cell(on_revert_cell)
            .on_toggle_delete_row(on_toggle_del_row)
            .on_add_row(on_add_row)
            .on_duplicate_row(on_duplicate_row)
            .on_discard_inserted_row(on_discard_inserted_row)
            .on_discard_all_changes(on_discard_all)
            .on_save_changes(on_save_changes)
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

        let app_handle = cx.entity().clone();

        // Header TitleBar
        let title_bar = TitleBar::new().child(
            h_flex()
                .size_full()
                .justify_between()
                .items_center()
                .px_3()
                .gap_2()
                .overflow_hidden()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .flex_shrink_0()
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
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .flex_shrink(1.0)
                        .min_w_0()
                        .overflow_hidden()
                        .when(is_read_only, |this| {
                            this.child(
                                div()
                                    .flex_shrink_0()
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
                                    .flex_shrink(1.0)
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_MUTED)
                                    .child(msg.clone()),
                            )
                        }),
                ),
        );

        // Left Sidebar
        let sidebar = Sidebar::new(
            self.saved_connections.clone(),
            active_conn_id,
            self.active_tables.clone(),
            &self.sidebar_conn_filter,
            &self.sidebar_table_filter,
            &self.sidebar_split,
        )
        .selected_table(self.selected_table.clone())
        .on_create_table({
            let handle = app_handle.clone();
            move |window, cx| {
                handle.update(cx, |this, cx| {
                    this.open_create_table_modal(window, cx);
                });
            }
        })
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
            move |table, _, cx| {
                handle.update(cx, |this, cx| {
                    this.select_table(table, cx);
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
        })
        .on_edit_connection({
            let handle = app_handle.clone();
            move |conn_id, window, cx| {
                handle.update(cx, |this, cx| {
                    this.open_edit_connection_dialog(&conn_id, window, cx);
                });
            }
        })
        .on_duplicate_connection({
            let handle = app_handle.clone();
            move |conn_id, _, cx| {
                handle.update(cx, |this, cx| {
                    this.duplicate_connection(&conn_id, cx);
                });
            }
        })
        .on_delete_connection({
            let handle = app_handle.clone();
            move |conn_id, _, cx| {
                handle.update(cx, |this, cx| {
                    this.delete_connection(&conn_id, cx);
                });
            }
        });

        // Tabs navigation bar
        let selected_tbl_label = self.selected_table.as_deref();

        let tabs_bar = h_flex()
            .h(px(38.0))
            .w_full()
            .px_3()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .overflow_hidden()
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_1()
                    .overflow_hidden()
                    .child({
                        let is_active = self.active_tab == WorkspaceTab::QueryConsole;
                        let handle = app_handle.clone();
                        Button::new("tab_console")
                            .small()
                            .ghost()
                            .flex_shrink(1.0)
                            .min_w(px(36.0))
                            .overflow_hidden()
                            .icon(IconName::Terminal)
                            .label("SQL Console")
                            .tooltip("SQL Console")
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
                        let label = match selected_tbl_label {
                            Some(name) => format!("Data · {name}"),
                            None => "Data".to_string(),
                        };
                        let tooltip = label.clone();
                        Button::new("tab_data")
                            .small()
                            .ghost()
                            .flex_shrink(1.0)
                            .min_w(px(36.0))
                            .overflow_hidden()
                            .icon(IconName::Table)
                            .label(label)
                            .tooltip(tooltip)
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
                        let label = match selected_tbl_label {
                            Some(name) => format!("Schema · {name}"),
                            None => "Schema".to_string(),
                        };
                        let tooltip = label.clone();
                        Button::new("tab_schema")
                            .small()
                            .ghost()
                            .flex_shrink(1.0)
                            .min_w(px(36.0))
                            .overflow_hidden()
                            .icon(IconName::TableProperties)
                            .label(label)
                            .tooltip(tooltip)
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
                            .flex_shrink(1.0)
                            .min_w(px(36.0))
                            .overflow_hidden()
                            .icon(IconName::Clock)
                            .label(label)
                            .tooltip("Query History")
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
                let on_format = {
                    let handle = app_handle.clone();
                    move |window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.format_editor_sql(window, cx);
                        });
                    }
                };
                let on_explain = {
                    let handle = app_handle.clone();
                    move |_: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.run_explain(cx);
                        });
                    }
                };
                let on_bottom_tab = {
                    let handle = app_handle.clone();
                    move |tab: ConsoleBottomTab, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.console_bottom_tab = tab;
                            cx.notify();
                        });
                    }
                };
                let on_explain_view = {
                    let handle = app_handle.clone();
                    move |view: ExplainViewMode, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.explain_view = view;
                            cx.notify();
                        });
                    }
                };

                let connection_label = match (
                    self.active_connection.as_ref().map(|c| c.config.name.clone()),
                    self.active_connection.as_ref().map(|c| c.config.database.clone()),
                ) {
                    (Some(name), Some(db)) if !db.is_empty() => Some(format!("{name} / {db}")),
                    (Some(name), _) => Some(name),
                    _ => None,
                };

                let console_grid = self.build_data_grid(
                    &app_handle,
                    self.console_result.clone(),
                    self.selected_table.clone(),
                    is_read_only,
                );

                let console = QueryConsole::new(&self.query_editor, &self.console_split)
                    .result(self.console_result.clone())
                    .error(self.console_error.clone())
                    .explain_plan(self.explain_plan.clone())
                    .explain_error(self.explain_error.clone())
                    .executing(self.is_executing_query)
                    .explaining(self.is_explaining)
                    .bottom_tab(self.console_bottom_tab)
                    .explain_view(self.explain_view)
                    .connection_label(connection_label)
                    .results_view(console_grid)
                    .on_run(on_run)
                    .on_clear(on_clear)
                    .on_format(on_format)
                    .on_explain(on_explain)
                    .on_bottom_tab(on_bottom_tab)
                    .on_explain_view(on_explain_view);

                let quick_connect_banner = if !is_connected {
                    let mut conn_chips = h_flex().gap_2().items_center().flex_wrap().min_w_0();
                    for conn in self.saved_connections.iter().take(4) {
                        let conn_id = conn.id.clone();
                        let handle = app_handle.clone();
                        let icon = crate::ui::components::connection_dialog::database_icon(conn.db_type);
                        let chip = Button::new(ElementId::Name(format!("quick_conn_{}", conn.id).into()))
                            .outline()
                            .small()
                            .flex_shrink(1.0)
                            .min_w(px(32.0))
                            .overflow_hidden()
                            .icon(icon)
                            .label(format!("Connect: {}", conn.name))
                            .tooltip(format!("Connect to {}", conn.name))
                            .on_click(move |_, _, cx| {
                                handle.update(cx, |this, cx| {
                                    this.select_connection(&conn_id, cx);
                                });
                            });
                        conn_chips = conn_chips.child(chip);
                    }

                    let handle = app_handle.clone();
                    let new_profile_btn = Button::new("quick_new_profile")
                        .primary()
                        .small()
                        .icon(IconName::Plus)
                        .label("New Database Connection")
                        .on_click(move |_, window, cx| {
                            handle.update(cx, |this, cx| {
                                this.open_connection_dialog(window, cx);
                            });
                        });

                    Some(
                        v_flex()
                            .w_full()
                            .p_3()
                            .gap_2()
                            .bg(ThemeColors::BG_SURFACE)
                            .border_b_1()
                            .border_color(ThemeColors::BORDER)
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                Icon::new(IconName::Server)
                                                    .size(px(16.0))
                                                    .text_color(ThemeColors::PRIMARY_BORDER),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .child("Select a saved profile or create a new connection"),
                                            ),
                                    )
                                    .child(new_profile_btn),
                            )
                            .child(conn_chips),
                    )
                } else {
                    None
                };

                v_flex()
                    .size_full()
                    .children(quick_connect_banner)
                    .child(div().size_full().flex_1().min_h_0().child(console))
                    .into_any_element()
            }
            WorkspaceTab::DataGrid => {
                let grid_data = self.table_data.clone().or_else(|| self.console_result.clone());
                self.build_data_grid(
                    &app_handle,
                    grid_data,
                    self.selected_table.clone(),
                    is_read_only,
                )
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
                let on_create = {
                    let handle = app_handle.clone();
                    move |window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.open_create_table_modal(window, cx);
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
                .on_create_table(on_create)
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
                .editing(self.dialog_editing_id.is_some())
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

        // SQL Review Modal overlay if open
        let sql_review_overlay = if self.sql_review_modal_open {
            if let Some(ref plan) = self.sql_review_plan {
                let on_exec = {
                    let handle = app_handle.clone();
                    move |_: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.execute_sql_review_plan(cx);
                        });
                    }
                };
                let on_cancel = {
                    let handle = app_handle.clone();
                    move |_: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.sql_review_modal_open = false;
                            this.sql_review_error = None;
                            cx.notify();
                        });
                    }
                };
                let on_copy = {
                    let handle = app_handle.clone();
                    move |sql: String, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(sql));
                            this.sql_review_copied = true;
                            this.status_message = Some("Copied SQL script to clipboard".to_string());
                            cx.notify();
                        });
                    }
                };

                Some(
                    SqlReviewModal::new(plan.clone())
                        .executing(self.sql_review_is_executing)
                        .error(self.sql_review_error.clone())
                        .copied(self.sql_review_copied)
                        .on_execute(on_exec)
                        .on_cancel(on_cancel)
                        .on_copy(on_copy),
                )
            } else {
                None
            }
        } else {
            None
        };

        // Create Table Modal overlay if open
        let create_table_overlay = if self.create_table_modal_open {
            if let Some(ref conn) = self.active_connection {
                let family = conn.config.db_type.family();
                let db_name = conn.config.database.clone();
                let (preview_sql, validation_err) = self.get_create_table_preview_sql(cx);

                let on_cancel = {
                    let handle = app_handle.clone();
                    move |_: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.close_create_table_modal(cx);
                        });
                    }
                };
                let on_exec = {
                    let handle = app_handle.clone();
                    move |_: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.execute_create_table(cx);
                        });
                    }
                };
                let on_add_col = {
                    let handle = app_handle.clone();
                    move |window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.add_create_table_column(window, cx);
                        });
                    }
                };
                let on_remove_col = {
                    let handle = app_handle.clone();
                    move |idx: usize, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.remove_create_table_column(idx, cx);
                        });
                    }
                };
                let on_toggle_pk = {
                    let handle = app_handle.clone();
                    move |idx: usize, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.toggle_create_table_pk(idx, cx);
                        });
                    }
                };
                let on_toggle_nn = {
                    let handle = app_handle.clone();
                    move |idx: usize, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.toggle_create_table_nullable(idx, cx);
                        });
                    }
                };
                let on_toggle_ai = {
                    let handle = app_handle.clone();
                    move |idx: usize, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.toggle_create_table_auto_inc(idx, cx);
                        });
                    }
                };
                let on_quick_type = {
                    let handle = app_handle.clone();
                    move |idx: usize, dt: String, window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.set_create_table_quick_type(idx, dt, window, cx);
                        });
                    }
                };
                let on_copy = {
                    let handle = app_handle.clone();
                    move |sql: String, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(sql));
                            this.create_table_copied = true;
                            this.status_message = Some("Copied Create Table DDL to clipboard".to_string());
                            cx.notify();
                        });
                    }
                };
                let on_console = {
                    let handle = app_handle.clone();
                    move |sql: String, window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.open_create_table_in_console(sql, window, cx);
                        });
                    }
                };

                Some(
                    CreateTableModal::new(
                        family,
                        db_name,
                        &self.create_table_name_input,
                        &self.create_table_schema_input,
                        &self.create_table_comment_input,
                        self.create_table_columns.clone(),
                        preview_sql,
                    )
                    .validation_error(validation_err)
                    .error(self.create_table_error.clone())
                    .executing(self.create_table_is_executing)
                    .copied(self.create_table_copied)
                    .on_add_column(on_add_col)
                    .on_remove_column(on_remove_col)
                    .on_toggle_pk(on_toggle_pk)
                    .on_toggle_nullable(on_toggle_nn)
                    .on_toggle_auto_inc(on_toggle_ai)
                    .on_quick_type(on_quick_type)
                    .on_copy_sql(on_copy)
                    .on_open_in_console(on_console)
                    .on_execute(on_exec)
                    .on_cancel(on_cancel),
                )
            } else {
                None
            }
        } else {
            None
        };

        // Root layout
        v_flex()
            .id("crabstudio_root")
            .key_context("CrabStudio")
            .on_action(cx.listener(|this, _: &RunQuery, _, cx| {
                this.run_query(cx);
            }))
            .on_action(cx.listener(|this, _: &FormatSql, window, cx| {
                this.format_editor_sql(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ExplainQuery, _, cx| {
                this.run_explain(cx);
            }))
            .on_action(cx.listener(|this, _: &SaveGridChanges, _, cx| {
                if (this.active_tab == WorkspaceTab::DataGrid || this.active_tab == WorkspaceTab::QueryConsole) && this.grid_changeset.is_dirty() {
                    this.open_sql_review_modal(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &AddNewRow, window, cx| {
                if this.active_tab == WorkspaceTab::DataGrid || this.active_tab == WorkspaceTab::QueryConsole {
                    this.add_new_grid_row(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &DuplicateGridRow, window, cx| {
                if this.active_tab == WorkspaceTab::DataGrid || this.active_tab == WorkspaceTab::QueryConsole {
                    if let Some(coord) = this.grid_selected_cell {
                        this.duplicate_grid_row(coord, window, cx);
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &DeleteGridRow, _, cx| {
                if this.active_tab == WorkspaceTab::DataGrid || this.active_tab == WorkspaceTab::QueryConsole {
                    if let Some(coord) = this.grid_selected_cell {
                        if coord.is_inserted {
                            this.discard_inserted_row(coord.row_idx, cx);
                        } else {
                            this.toggle_delete_grid_row(coord.row_idx, cx);
                        }
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &CloseDialog, _, cx| {
                if this.create_table_modal_open {
                    this.create_table_modal_open = false;
                    this.create_table_error = None;
                    cx.notify();
                } else if this.sql_review_modal_open {
                    this.sql_review_modal_open = false;
                    this.sql_review_error = None;
                    cx.notify();
                } else if this.grid_modal_open {
                    this.grid_modal_open = false;
                    cx.notify();
                } else if this.dialog_open {
                    this.close_connection_dialog(cx);
                }
            }))
            .size_full()
            .bg(ThemeColors::BG_APP)
            .child(title_bar)
            .child(
                h_flex()
                    .items_stretch()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .child(sidebar)
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .min_h_0()
                            .child(tabs_bar)
                            .child(v_flex().size_full().flex_1().min_h_0().w_full().child(main_content)),
                    ),
            )
            .child(status_bar)
            .children(dialog_overlay)
            .children(sql_review_overlay)
            .children(create_table_overlay)
    }
}
