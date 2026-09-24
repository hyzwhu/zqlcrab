//! Main desktop application workspace coordinating navigation, query console, and data inspection.

use crate::db::changeset::GridChangeset;
use crate::db::explain::{ExplainPlan, parse_explain_result, wrap_explain_sql};
use crate::db::export::{
    ExportFormat, ExportOptions, ExportScope, TableDumpConfig, export_result,
    generate_export_preview, generate_table_dump, suggested_file_name,
};
use crate::db::handle::ActiveConnection;
use crate::db::history::{QueryHistoryItem, QueryHistoryManager, QueryHistoryStatus};
use crate::db::import::{
    ColumnMapping, CsvDelimiter, CsvImportConfig, CsvPreviewData, CsvSniffer, ErrorPolicy,
    FileEncoding, ImportExecutor, ImportFormat, ImportProgress, ImportResult, SqlPreviewData,
    auto_map_columns,
};
use crate::db::manager::ConnectionManager;
use crate::db::mock_data::{
    MockColumnConfig, MockGeneratorType, MockProgress, MockResult, execute_mock_seeding,
    generate_mock_preview, initialize_column_configs,
};
use crate::db::sql_format::format_sql_with_indent;
use crate::db::sql_gen::{
    AlterColumnTarget, ColumnDef, CreateTableDef, SqlReviewPlan, TableIndexDef, TableIndexType,
    column_matches_index_spec, extract_table_from_sql, generate_alter_table_plan,
    generate_create_table_sql, generate_review_plan, parse_create_table_sql, parse_sql_column_list,
};
use crate::db::types::{
    ColumnInfo, ConnectionConfig, DatabaseFamily, DatabaseType, IndexInfo, QueryResult, QueryValue,
    SortDirection, TableInfo,
};
use crate::settings::{SettingsManager, ThemePreference};
use crate::ui::components::{
    ActivityBar, ActivityNav, AppStatusBar, ConfirmActionKind, ConfirmDialog, ConnectionDialog,
    ConsoleBottomTab, DataGrid, ExplainViewMode, ExportDestination, ExportModal,
    ExportSuccessInfo, ExportWizardStep, ImportModal, ImportWizardStep, MockDataModal,
    MockWizardStep, QueryConsole, QueryHistoryView, QueryTabHeader, SchemaViewer, SettingsTab,
    SettingsView, Sidebar, SqlReviewModal,
    create_table_modal::{CreateTableColumnState, CreateTableIndexState, CreateTableModal},
    data_grid::GridCellCoord,
    schema_viewer::SchemaEditColumnState,
};
use crate::ui::i18n::t;
use crate::ui::theme::ThemeColors;
use chrono::Utc;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _, Theme, ThemeMode, TitleBar,
    button::{Button, ButtonVariants as _},
    input::{EditorState, InputEvent, InputState, TabSize},
    resizable::ResizableState,
};
use gpui_kit::gpui::{
    App, AsyncApp, ClipboardItem, Context, ElementId, Entity, FontWeight, Image, ImageFormat,
    IntoElement, ParentElement, Render, ScrollHandle, Styled, Window, div, img, point, prelude::*,
    px,
};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub const LOGO_PNG_BYTES: &[u8] = include_bytes!("../../assets/logo.png");
pub const TRAY_ICON_PNG_BYTES: &[u8] = include_bytes!("../../assets/tray-icon.png");

gpui_kit::actions!(
    zqlcrab,
    [
        Quit,
        CloseWindow,
        OpenSettings,
        NewConnection,
        NewQueryTab,
        RefreshTables,
        SelectConsoleTab,
        SelectGridTab,
        SelectSchemaTab,
        SelectHistoryTab,
        ToggleActivityBar,
        ToggleStatusBar,
        MinimizeWindow,
        ZoomWindow,
        ToggleFullscreen,
        AboutZqlcrab,
        CheckForUpdates,
        OpenDocs,
        OpenGithub,
        ReportIssue,
        RunQuery,
        CloseDialog,
        FormatSql,
        ExplainQuery,
        SaveGridChanges,
        DeleteGridRow,
        AddNewRow,
        DuplicateGridRow,
        OpenImportModal,
        FocusGridFilter
    ]
);

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

#[derive(Clone)]
pub struct TableConfirmActionState {
    pub kind: ConfirmActionKind,
    pub table: TableInfo,
    pub database_family: DatabaseFamily,
    pub sql_preview: String,
    pub is_executing: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct QueryTab {
    pub id: Uuid,
    pub title: String,
    pub connection_profile_id: Option<String>,
    pub editor: Entity<EditorState>,
    pub split_state: Entity<ResizableState>,
    pub result: Option<Arc<QueryResult>>,
    pub error: Option<String>,
    pub explain_plan: Option<ExplainPlan>,
    pub explain_error: Option<String>,
    pub is_executing: bool,
    pub is_explaining: bool,
    pub execution_time_ms: Option<u64>,
    pub bottom_tab: ConsoleBottomTab,
    pub explain_view: ExplainViewMode,
}

pub struct CrabStudioApp {
    manager: ConnectionManager,
    saved_connections: Vec<ConnectionConfig>,
    active_connection: Option<ActiveConnection>,
    active_tables: Vec<TableInfo>,
    selected_table: Option<String>,
    table_data: Option<Arc<QueryResult>>,
    schema_columns: Vec<ColumnInfo>,
    schema_indexes: Vec<IndexInfo>,
    schema_ddl: Option<String>,
    schema_is_editing: bool,
    schema_edit_columns: Vec<SchemaEditColumnState>,
    active_tab: WorkspaceTab,

    // Table confirm dialog state (Drop/Truncate)
    table_confirm_modal: Option<TableConfirmActionState>,

    // Connection Error Dialog state
    connection_error_modal: Option<crate::ui::components::ConnectionErrorInfo>,
    connection_error_copied: bool,
    connection_error_is_retrying: bool,

    // Multi-tab query console state
    query_tabs: Vec<QueryTab>,
    active_query_tab_id: Uuid,
    query_tab_counter: usize,
    sql_metadata_cache: Arc<RwLock<crate::db::autocomplete::SqlMetadataCache>>,
    status_message: Option<String>,

    // History & DataGrid state
    history_manager: QueryHistoryManager,
    history_filter: String,
    grid_sort_col: Option<usize>,
    grid_sort_dir: Option<SortDirection>,
    grid_page: usize,
    grid_page_size: usize,
    grid_filter: String,
    grid_filter_input: Entity<InputState>,
    grid_selected_cell: Option<GridCellCoord>,
    grid_inspector_open: bool,
    grid_modal_open: bool,
    grid_json_pretty: bool,
    grid_changeset: GridChangeset,
    grid_cell_edit_input: Entity<InputState>,
    grid_scroll_handle: ScrollHandle,
    grid_inspector_split: Entity<ResizableState>,
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
    create_table_indexes: Vec<CreateTableIndexState>,
    create_table_ddl_editor: Entity<EditorState>,
    create_table_sync_status: Option<Result<String, String>>,
    create_table_last_synced_sql: String,
    create_table_is_syncing: bool,
    create_table_is_executing: bool,
    create_table_error: Option<String>,
    create_table_copied: bool,

    // Data Import Wizard state
    import_modal_open: bool,
    import_step: ImportWizardStep,
    import_file_path_input: Entity<InputState>,
    import_file_path: Option<std::path::PathBuf>,
    import_format: ImportFormat,
    import_encoding: FileEncoding,
    import_delimiter: CsvDelimiter,
    import_has_headers: bool,
    import_target_table: Option<String>,
    import_available_tables: Vec<String>,
    import_table_columns: Vec<ColumnInfo>,
    import_csv_preview: Option<CsvPreviewData>,
    import_sql_preview: Option<SqlPreviewData>,
    import_mappings: Vec<ColumnMapping>,
    import_batch_size: usize,
    import_error_policy: ErrorPolicy,
    import_is_executing: bool,
    import_progress: Option<ImportProgress>,
    import_result: Option<ImportResult>,
    import_error: Option<String>,

    // Data & Schema Export Wizard state
    export_modal_open: bool,
    export_step: ExportWizardStep,
    export_target_table: String,
    export_available_tables: Vec<String>,
    export_config: TableDumpConfig,
    export_destination: ExportDestination,
    export_file_path_input: Entity<InputState>,
    export_file_path: Option<std::path::PathBuf>,
    export_where_input: Entity<InputState>,
    export_limit_input: Entity<InputState>,
    export_preview_content: Option<String>,
    export_is_loading_preview: bool,
    export_is_executing: bool,
    export_progress_rows: usize,
    export_success_info: Option<ExportSuccessInfo>,
    export_error: Option<String>,
    export_ddl_cache: Option<String>,

    // Visual Mock Data Generator Wizard state
    mock_modal_open: bool,
    mock_step: MockWizardStep,
    mock_target_table: String,
    mock_available_tables: Vec<String>,
    mock_columns: Vec<MockColumnConfig>,
    mock_row_count: usize,
    mock_batch_size: usize,
    mock_preview_headers: Vec<String>,
    mock_preview_rows: Vec<Vec<String>>,
    mock_is_loading_preview: bool,
    mock_is_executing: bool,
    mock_progress: Option<MockProgress>,
    mock_result: Option<MockResult>,
    mock_error: Option<String>,

    // Settings state
    settings_manager: SettingsManager,
    active_nav: ActivityNav,
    active_settings_tab: SettingsTab,
    is_checking_update: bool,
    update_status_msg: Option<String>,
    update_check_result: Option<crate::update::UpdateCheckResult>,
}

impl CrabStudioApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let manager = ConnectionManager::new();
        let saved = manager.list_configs();
        let settings_manager = SettingsManager::new();
        crate::ui::theme::set_active_theme_mode(
            settings_manager.settings().appearance.theme == ThemePreference::Light,
        );

        let handle = cx.entity().clone();
        let target_conn_id = settings_manager
            .settings()
            .last_connection_id
            .as_ref()
            .and_then(|last_id| {
                saved
                    .iter()
                    .find(|c| &c.id == last_id)
                    .map(|c| c.id.clone())
            });
        if let Some(conn_id) = target_conn_id {
            cx.defer(move |cx| {
                handle.update(cx, |this, cx| {
                    this.select_connection(&conn_id, cx);
                });
            });
        }

        if let Some(mut tray_rx) = crate::ui::tray::take_tray_receiver() {
            cx.spawn(async move |this, cx: &mut AsyncApp| {
                while let Some(action) = tray_rx.recv().await {
                    this.update(cx, |app, cx| match action {
                        crate::ui::tray::TrayAction::Connect(conn_id) => {
                            app.select_connection(&conn_id, cx);
                        }
                        crate::ui::tray::TrayAction::NewConnection => {
                            app.open_new_connection_dialog(cx);
                        }
                        crate::ui::tray::TrayAction::OpenSettings => {
                            app.active_nav = ActivityNav::Settings;
                            app.active_settings_tab = SettingsTab::Appearance;
                            cx.notify();
                        }
                    })
                    .ok();
                }
            })
            .detach();
        }

        let sql_metadata_cache = Arc::new(RwLock::new(
            crate::db::autocomplete::SqlMetadataCache::default(),
        ));

        let initial_tab_id = Uuid::new_v4();
        let initial_tab = {
            let cache_ref = sql_metadata_cache.clone();
            let ed_cfg = settings_manager.settings().editor.clone();
            let editor = cx.new(|cx| {
                let mut ed = EditorState::new(window, cx)
                    .language("sql")
                    .soft_wrap(ed_cfg.word_wrap)
                    .line_number(ed_cfg.line_numbers)
                    .tab_size(TabSize {
                        tab_size: ed_cfg.tab_size,
                        hard_tabs: false,
                    })
                    .auto_close(ed_cfg.bracket_matching);
                ed.set_value(
                    "-- Press ⌘↵ (Ctrl+Enter) to run · Shift+Alt+F formats SQL\nSELECT 1 AS id, 'Welcome to CrabStudio' AS message;\n",
                    window,
                    cx,
                );
                let provider = crate::db::autocomplete::SqlCompletionProvider::new(cache_ref);
                ed.lsp_mut().completion_provider = Some(Rc::new(provider));
                ed
            });
            let split_state = cx.new(|_cx| ResizableState::default());
            QueryTab {
                id: initial_tab_id,
                title: "Query 1".to_string(),
                connection_profile_id: None,
                editor,
                split_state,
                result: None,
                error: None,
                explain_plan: None,
                explain_error: None,
                is_executing: false,
                is_explaining: false,
                execution_time_ms: None,
                bottom_tab: ConsoleBottomTab::Results,
                explain_view: ExplainViewMode::Tree,
            }
        };
        let query_tabs = vec![initial_tab];
        let active_query_tab_id = initial_tab_id;
        let query_tab_counter = 2;
        let sidebar_split = cx.new(|_cx| ResizableState::default());
        let grid_inspector_split = cx.new(|_cx| ResizableState::default());
        let sidebar_conn_filter =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search connections…"));
        cx.subscribe(&sidebar_conn_filter, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        let sidebar_table_filter =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search tables, views…"));
        cx.subscribe(&sidebar_table_filter, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();

        let dialog_name_input = cx.new(|cx| InputState::new(window, cx).placeholder("My Database"));
        let dialog_host_input = cx.new(|cx| InputState::new(window, cx).default_value("127.0.0.1"));
        let dialog_port_input = cx.new(|cx| InputState::new(window, cx).default_value("5432"));
        let dialog_database_input =
            cx.new(|cx| InputState::new(window, cx).default_value(":memory:"));
        let dialog_user_input = cx.new(|cx| InputState::new(window, cx).default_value("postgres"));
        let dialog_pass_input = cx.new(|cx| InputState::new(window, cx).masked(true));

        let grid_cell_edit_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Edit cell value..."));
        let grid_filter_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Filter rows (text, num, col:val)..."));

        cx.subscribe(&grid_filter_input, |this, input_handle, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let val = input_handle.read(cx).value().to_string();
                this.grid_filter = val;
                this.grid_page = 0;
                cx.notify();
            }
        })
        .detach();

        let create_table_name_input =
            cx.new(|cx| InputState::new(window, cx).default_value("new_table"));
        let create_table_schema_input = cx.new(|cx| InputState::new(window, cx).default_value(""));
        let create_table_comment_input = cx.new(|cx| InputState::new(window, cx).default_value(""));

        let col1_name = cx.new(|cx| InputState::new(window, cx).default_value("id"));
        let col1_type = cx.new(|cx| InputState::new(window, cx).default_value("INTEGER"));
        let col1_def = cx.new(|cx| InputState::new(window, cx).default_value(""));
        let col1_comment = cx.new(|cx| InputState::new(window, cx).default_value(""));

        let col2_name = cx.new(|cx| InputState::new(window, cx).default_value("name"));
        let col2_type = cx.new(|cx| InputState::new(window, cx).default_value("TEXT"));
        let col2_def = cx.new(|cx| InputState::new(window, cx).default_value(""));
        let col2_comment = cx.new(|cx| InputState::new(window, cx).default_value(""));

        let create_table_columns = vec![
            CreateTableColumnState {
                name: col1_name,
                data_type: col1_type,
                is_primary_key: true,
                is_nullable: false,
                is_auto_increment: true,
                default_val: col1_def,
                comment: col1_comment,
            },
            CreateTableColumnState {
                name: col2_name,
                data_type: col2_type,
                is_primary_key: false,
                is_nullable: false,
                is_auto_increment: false,
                default_val: col2_def,
                comment: col2_comment,
            },
        ];

        let create_table_ddl_editor = cx.new(|cx| {
            let mut ed = EditorState::new(window, cx)
                .language("sql")
                .line_number(true);
            ed.set_value("", window, cx);
            ed
        });
        cx.subscribe(&create_table_ddl_editor, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();

        let import_file_path_input = cx.new(|cx| InputState::new(window, cx));
        let export_file_path_input = cx.new(|cx| InputState::new(window, cx));
        let export_where_input = cx.new(|cx| InputState::new(window, cx));
        let export_limit_input = cx.new(|cx| InputState::new(window, cx));

        let mut history_manager = QueryHistoryManager::new();
        history_manager.set_max_entries(settings_manager.settings().query.history_limit);

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
            schema_is_editing: false,
            schema_edit_columns: Vec::new(),
            active_tab: WorkspaceTab::QueryConsole,
            table_confirm_modal: None,
            connection_error_modal: None,
            connection_error_copied: false,
            connection_error_is_retrying: false,
            query_tabs,
            active_query_tab_id,
            query_tab_counter,
            sql_metadata_cache,
            status_message: Some("Ready".to_string()),
            history_manager,
            history_filter: String::new(),
            grid_sort_col: None,
            grid_sort_dir: None,
            grid_page: 0,
            grid_page_size: 50,
            grid_filter: String::new(),
            grid_filter_input,
            grid_selected_cell: None,
            grid_inspector_open: false,
            grid_modal_open: false,
            grid_json_pretty: true,
            grid_changeset: GridChangeset::new(),
            grid_cell_edit_input,
            grid_scroll_handle: ScrollHandle::default(),
            grid_inspector_split,
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
            create_table_indexes: Vec::new(),
            create_table_ddl_editor,
            create_table_sync_status: None,
            create_table_last_synced_sql: String::new(),
            create_table_is_syncing: false,
            create_table_is_executing: false,
            create_table_error: None,
            create_table_copied: false,
            import_modal_open: false,
            import_step: ImportWizardStep::Step1Source,
            import_file_path_input,
            import_file_path: None,
            import_format: ImportFormat::Csv,
            import_encoding: FileEncoding::Utf8,
            import_delimiter: CsvDelimiter::Comma,
            import_has_headers: true,
            import_target_table: None,
            import_available_tables: Vec::new(),
            import_table_columns: Vec::new(),
            import_csv_preview: None,
            import_sql_preview: None,
            import_mappings: Vec::new(),
            import_batch_size: 500,
            import_error_policy: ErrorPolicy::Skip,
            import_is_executing: false,
            import_progress: None,
            import_result: None,
            import_error: None,
            export_modal_open: false,
            export_step: ExportWizardStep::Step1Config,
            export_target_table: String::new(),
            export_available_tables: Vec::new(),
            export_config: TableDumpConfig::default(),
            export_destination: ExportDestination::File,
            export_file_path_input,
            export_file_path: None,
            export_where_input,
            export_limit_input,
            export_preview_content: None,
            export_is_loading_preview: false,
            export_is_executing: false,
            export_progress_rows: 0,
            export_success_info: None,
            export_error: None,
            export_ddl_cache: None,
            mock_modal_open: false,
            mock_step: MockWizardStep::Step1Config,
            mock_target_table: String::new(),
            mock_available_tables: Vec::new(),
            mock_columns: Vec::new(),
            mock_row_count: 500,
            mock_batch_size: 200,
            mock_preview_headers: Vec::new(),
            mock_preview_rows: Vec::new(),
            mock_is_loading_preview: false,
            mock_is_executing: false,
            mock_progress: None,
            mock_result: None,
            mock_error: None,
            settings_manager,
            active_nav: ActivityNav::Databases,
            active_settings_tab: SettingsTab::Appearance,
            is_checking_update: false,
            update_status_msg: None,
            update_check_result: None,
        }
    }

    /// Get a reference to the active query tab (falling back to first tab if present)
    pub fn active_query_tab(&self) -> Option<&QueryTab> {
        self.query_tabs
            .iter()
            .find(|t| t.id == self.active_query_tab_id)
            .or_else(|| self.query_tabs.first())
    }

    /// Get a mutable reference to the active query tab
    pub fn active_query_tab_mut(&mut self) -> Option<&mut QueryTab> {
        let id = self.active_query_tab_id;
        if let Some(pos) = self.query_tabs.iter().position(|t| t.id == id) {
            return Some(&mut self.query_tabs[pos]);
        }
        self.query_tabs.first_mut()
    }

    /// Factory method to build a new SQL EditorState entity
    pub fn create_query_editor(
        &self,
        initial_sql: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<EditorState> {
        let cache_ref = self.sql_metadata_cache.clone();
        let ed_cfg = self.settings_manager.settings().editor.clone();
        cx.new(|cx| {
            let mut ed = EditorState::new(window, cx)
                .language("sql")
                .soft_wrap(ed_cfg.word_wrap)
                .line_number(ed_cfg.line_numbers)
                .tab_size(TabSize {
                    tab_size: ed_cfg.tab_size,
                    hard_tabs: false,
                })
                .auto_close(ed_cfg.bracket_matching);
            if !initial_sql.is_empty() {
                ed.set_value(initial_sql, window, cx);
            }
            let provider = crate::db::autocomplete::SqlCompletionProvider::new(cache_ref);
            ed.lsp_mut().completion_provider = Some(Rc::new(provider));
            ed
        })
    }

    /// Create a new query tab, activate it, and switch to QueryConsole workspace tab
    pub fn create_query_tab(
        &mut self,
        title: Option<String>,
        initial_sql: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Uuid {
        let tab_id = Uuid::new_v4();
        let title = title.unwrap_or_else(|| {
            let name = format!("Query {}", self.query_tab_counter);
            self.query_tab_counter += 1;
            name
        });
        let sql = initial_sql.unwrap_or("");
        let editor = self.create_query_editor(sql, window, cx);
        let split_state = cx.new(|_cx| ResizableState::default());

        let conn_id = self.active_connection.as_ref().map(|c| c.config.id.clone());

        let tab = QueryTab {
            id: tab_id,
            title,
            connection_profile_id: conn_id,
            editor,
            split_state,
            result: None,
            error: None,
            explain_plan: None,
            explain_error: None,
            is_executing: false,
            is_explaining: false,
            execution_time_ms: None,
            bottom_tab: ConsoleBottomTab::Results,
            explain_view: ExplainViewMode::Tree,
        };
        self.query_tabs.push(tab);
        self.active_query_tab_id = tab_id;
        self.active_tab = WorkspaceTab::QueryConsole;
        self.active_nav = ActivityNav::Console;
        cx.notify();
        tab_id
    }

    /// Switch active query tab by UUID
    pub fn switch_query_tab(&mut self, tab_id: Uuid, cx: &mut Context<Self>) {
        if self.query_tabs.iter().any(|t| t.id == tab_id) {
            self.active_query_tab_id = tab_id;
            self.active_tab = WorkspaceTab::QueryConsole;
            cx.notify();
        }
    }

    /// Retrieve the effective database connection for a specific tab.
    /// If the tab has a bound `connection_profile_id` and it matches `self.active_connection`, returns that.
    /// Otherwise falls back to `self.active_connection`.
    pub fn connection_for_tab(&self, tab: &QueryTab) -> Option<ActiveConnection> {
        if let Some(ref bound_id) = tab.connection_profile_id {
            if let Some(ref conn) = self.active_connection {
                if &conn.config.id == bound_id {
                    return Some(conn.clone());
                }
            }
        }
        self.active_connection.clone()
    }

    /// Set or change the bound database connection profile for a query tab
    pub fn set_query_tab_connection(
        &mut self,
        tab_id: Uuid,
        conn_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.query_tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.connection_profile_id = conn_id;
            cx.notify();
        }
    }

    /// Close a query tab. If it is the last tab, reset it to a clean empty state.
    pub fn close_query_tab(&mut self, tab_id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(idx) = self.query_tabs.iter().position(|t| t.id == tab_id) else {
            return;
        };

        if self.query_tabs.len() <= 1 {
            let tab = &mut self.query_tabs[0];
            tab.title = "Query 1".to_string();
            tab.editor.update(cx, |ed, cx| {
                ed.set_value("", window, cx);
            });
            tab.result = None;
            tab.error = None;
            tab.explain_plan = None;
            tab.explain_error = None;
            tab.execution_time_ms = None;
            tab.is_executing = false;
            tab.is_explaining = false;
            self.status_message = Some("Reset query session".to_string());
            cx.notify();
            return;
        }

        self.query_tabs.remove(idx);

        if self.active_query_tab_id == tab_id {
            let new_idx = if idx >= self.query_tabs.len() {
                self.query_tabs.len() - 1
            } else {
                idx
            };
            self.active_query_tab_id = self.query_tabs[new_idx].id;
        }
        cx.notify();
    }

    /// Close currently active query tab
    pub fn close_active_query_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active_id = self.active_query_tab_id;
        self.close_query_tab(active_id, window, cx);
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
            let conn_res = ActiveConnection::connect_config(config_clone.clone()).await;
            match conn_res {
                Ok(conn) => {
                    let tables_res = conn.list_tables(None, None).await.unwrap_or_default();
                    this.update(cx, |app, cx| {
                        let name = conn.config.name.clone();
                        let conn_id = conn.config.id.clone();
                        let db_family = conn.config.db_type.family();
                        let db_name = conn
                            .status
                            .as_ref()
                            .and_then(|s| s.current_database.clone())
                            .or_else(|| {
                                if conn.config.database.is_empty() {
                                    None
                                } else {
                                    Some(conn.config.database.clone())
                                }
                            });
                        let ping_ms = conn.status.as_ref().and_then(|s| s.ping_ms);
                        crate::ui::tray::update_tray_status(Some(crate::ui::tray::TrayStatus {
                            active_conn_id: Some(conn_id.clone()),
                            active_name: Some(name.clone()),
                            db_name: db_name.clone(),
                            ping_ms,
                            memory_mb: crate::ui::tray::get_process_memory_mb(),
                        }));
                        // Asynchronously prefetch column metadata for cached tables so dot completion is instant
                        let prefetch_conn = conn.clone();
                        let prefetch_cache = app.sql_metadata_cache.clone();
                        let prefetch_tables = tables_res.clone();
                        if let Some(old_conn) = app.active_connection.take() {
                            cx.spawn(async move |_, _| {
                                let _ = old_conn.disconnect().await;
                            })
                            .detach();
                        }
                        app.active_connection = Some(conn);
                        app.settings_manager.settings_mut().last_connection_id =
                            Some(conn_id.clone());
                        let _ = app.settings_manager.save();
                        app.active_tables = tables_res.clone();

                        // Automatically bind connection to active query tab if it does not have one
                        if let Some(active_tab) = app
                            .query_tabs
                            .iter_mut()
                            .find(|t| t.id == app.active_query_tab_id)
                        {
                            if active_tab.connection_profile_id.is_none() {
                                active_tab.connection_profile_id = Some(conn_id);
                            }
                        }

                        // Sync autocomplete metadata cache
                        if let Ok(mut cache) = app.sql_metadata_cache.write() {
                            cache.set_family(Some(db_family));
                            if let Some(ref db) = db_name {
                                cache.set_databases(vec![db.clone()]);
                            }
                            cache.set_tables(tables_res);
                        }

                        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
                            for tbl in prefetch_tables {
                                if let Ok(cols) = prefetch_conn
                                    .list_columns(None, tbl.schema.as_deref(), &tbl.name)
                                    .await
                                {
                                    if let Ok(mut cache) = prefetch_cache.write() {
                                        cache.set_columns_for_table(&tbl.name, cols);
                                    }
                                }
                            }
                        })
                        .detach();
                        app.selected_table = None;
                        app.table_data = None;
                        app.grid_changeset.clear();
                        app.sql_review_modal_open = false;
                        app.sql_review_plan = None;
                        app.schema_columns.clear();
                        app.schema_indexes.clear();
                        app.schema_ddl = None;
                        app.connection_error_modal = None;
                        app.connection_error_is_retrying = false;
                        app.status_message = Some(format!("Connected to {name}"));
                        if let Some(target) = app.active_tables.first().cloned() {
                            app.select_table(target, cx);
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Err(err) => {
                    this.update(cx, |app, cx| {
                        let err_msg = err.to_string();
                        app.status_message = Some(format!("Connection error: {err_msg}"));
                        app.connection_error_is_retrying = false;
                        app.connection_error_copied = false;
                        app.connection_error_modal =
                            Some(crate::ui::components::ConnectionErrorInfo {
                                connection_id: config_clone.id.clone(),
                                connection_name: config_clone.name.clone(),
                                database_type: config_clone.db_type,
                                host: config_clone.host.clone(),
                                port: config_clone.port,
                                database: config_clone.database.clone(),
                                error_message: err_msg,
                            });
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Close the connection error modal
    pub fn close_connection_error_modal(&mut self, cx: &mut Context<Self>) {
        self.connection_error_modal = None;
        self.connection_error_copied = false;
        self.connection_error_is_retrying = false;
        cx.notify();
    }

    /// Retry connecting to a profile from the connection error modal
    pub fn retry_connection_error(&mut self, conn_id: &str, cx: &mut Context<Self>) {
        self.connection_error_is_retrying = true;
        cx.notify();
        self.select_connection(conn_id, cx);
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
        self.schema_is_editing = false;
        self.schema_edit_columns.clear();
        self.status_message = Some(format!("Loading table {}...", table.name));
        cx.notify();

        let tbl = table.name.clone();
        let schema = table.schema.clone();
        let Some(conn) = self.active_connection.clone() else {
            return;
        };
        let family = conn.config.db_type.family();
        let qualified = table.qualified_name(family);
        let default_limit = self.settings_manager.settings().query.default_limit;

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let data_res = conn
                .execute_query(&format!("SELECT * FROM {qualified} LIMIT {default_limit}"))
                .await;
            let cols = conn
                .list_columns(None, schema.as_deref(), &tbl)
                .await
                .unwrap_or_default();
            let idxs = conn
                .list_indexes(None, schema.as_deref(), &tbl)
                .await
                .unwrap_or_default();
            let ddl = conn
                .get_table_ddl(None, schema.as_deref(), &tbl)
                .await
                .ok()
                .flatten();

            this.update(cx, |app, cx| {
                app.table_data = data_res.ok().map(Arc::new);
                // Cache columns for autocomplete
                let mut cols = cols;
                ColumnInfo::apply_primary_key_index(&mut cols, &idxs);
                if let Ok(mut cache) = app.sql_metadata_cache.write() {
                    cache.set_columns_for_table(&tbl, cols.clone());
                }
                app.schema_columns = cols;
                app.schema_indexes = idxs;
                app.schema_ddl = ddl;
                if app.active_tab == WorkspaceTab::QueryConsole {
                    app.active_tab = WorkspaceTab::DataGrid;
                }
                app.status_message = Some(format!("Loaded table {tbl}"));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Disconnect from the active database
    pub fn disconnect(&mut self, cx: &mut Context<Self>) {
        if let Some(conn) = self.active_connection.take() {
            crate::ui::tray::update_tray_status(None);
            let name = conn.config.name.clone();
            cx.spawn(async move |_, _| {
                let _ = conn.disconnect().await;
            })
            .detach();

            self.active_tables.clear();
            if let Ok(mut cache) = self.sql_metadata_cache.write() {
                cache.clear();
            }
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
                if let Ok(mut cache) = app.sql_metadata_cache.write() {
                    cache.set_tables(tables.clone());
                }
                app.active_tables = tables.clone();

                let prefetch_conn = conn.clone();
                let prefetch_cache = app.sql_metadata_cache.clone();
                cx.spawn(async move |_this, _cx: &mut AsyncApp| {
                    for tbl in tables {
                        if let Ok(cols) = prefetch_conn
                            .list_columns(None, tbl.schema.as_deref(), &tbl.name)
                            .await
                        {
                            if let Ok(mut cache) = prefetch_cache.write() {
                                cache.set_columns_for_table(&tbl.name, cols);
                            }
                        }
                    }
                })
                .detach();

                app.status_message = Some("Schema refreshed".to_string());
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Trigger application update check against GitHub Releases API
    pub fn trigger_check_for_updates(&mut self, cx: &mut Context<Self>) {
        self.is_checking_update = true;
        self.update_status_msg = None;
        cx.notify();

        let current_ver = env!("CARGO_PKG_VERSION").to_string();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let res = crate::update::check_for_updates(&current_ver).await;
            this.update(cx, |app, cx| {
                app.is_checking_update = false;
                match &res {
                    crate::update::UpdateCheckResult::NewVersionAvailable {
                        latest_version,
                        ..
                    } => {
                        app.status_message = Some(format!("Update available: v{latest_version}"));
                    }
                    crate::update::UpdateCheckResult::UpToDate {
                        current_version, ..
                    } => {
                        app.status_message =
                            Some(format!("zqlcrab v{current_version} is up to date"));
                    }
                    crate::update::UpdateCheckResult::Failed { error } => {
                        app.status_message = Some(format!("Check for updates failed: {error}"));
                    }
                }
                app.update_check_result = Some(res);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Synchronize settings into the live SQL query editor instance
    pub fn sync_editor_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.settings_manager.settings().editor.clone();
        for tab in &self.query_tabs {
            tab.editor.update(cx, |ed, cx| {
                ed.set_soft_wrap(s.word_wrap, window, cx);
                ed.set_line_number(s.line_numbers, window, cx);
                ed.set_tab_size(
                    TabSize {
                        tab_size: s.tab_size,
                        hard_tabs: false,
                    },
                    cx,
                );
                ed.set_auto_close(s.bracket_matching, window, cx);
            });
        }
        cx.notify();
    }

    /// Execute the query written in the query editor (or currently selected text range if active)
    pub fn run_query(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        let Some(active_tab) = self.active_query_tab() else {
            return;
        };
        let tab_id = active_tab.id;
        let editor_read = active_tab.editor.read(cx);
        let selected_text = editor_read.selected_text().to_string();
        let is_selected_exec = !selected_text.trim().is_empty();
        let mut sql = if is_selected_exec {
            selected_text
        } else {
            editor_read.value().to_string()
        };
        if sql.trim().is_empty() {
            return;
        }

        let settings = self.settings_manager.settings();

        // Format SQL before execution if configured (only full text format to avoid breaking selected ranges)
        if settings.editor.format_on_run && !is_selected_exec {
            let formatted = format_sql_with_indent(&sql, settings.editor.tab_size);
            if formatted != sql {
                if let Some(win) = window {
                    if let Some(tab) = self.query_tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.editor.update(cx, |editor, cx| {
                            editor.replace_all(&formatted, win, cx);
                        });
                    }
                }
                sql = formatted;
            }
        }

        // Safe mode protection: confirm/block unbounded destructive queries
        if settings.query.safe_mode {
            let upper = sql.trim().to_uppercase();
            let is_unbounded_del = upper.starts_with("DELETE") && !upper.contains("WHERE");
            let is_unbounded_upd = upper.starts_with("UPDATE") && !upper.contains("WHERE");
            let is_drop = upper.starts_with("DROP");
            let is_truncate = upper.starts_with("TRUNCATE");

            if is_unbounded_del || is_unbounded_upd || is_drop || is_truncate {
                if let Some(tab) = self.query_tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.error = Some(
                        "Safe Mode Protection: Detected destructive or unbounded mutation query without WHERE clause. Execution halted. (Disable Safe Mode in Settings to bypass)."
                            .to_string(),
                    );
                    tab.bottom_tab = ConsoleBottomTab::Results;
                }
                self.status_message = Some("Blocked by Safe Mode".to_string());
                cx.notify();
                return;
            }
        }

        let Some(conn) = self
            .query_tabs
            .iter()
            .find(|t| t.id == tab_id)
            .and_then(|t| self.connection_for_tab(t))
        else {
            if let Some(tab) = self.query_tabs.iter_mut().find(|t| t.id == tab_id) {
                tab.error = Some(
                    "No active database connection. Please select or create a connection first."
                        .to_string(),
                );
            }
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
                let cols = conn_meta
                    .list_columns(None, schema_for_cols.as_deref(), &tbl_for_cols)
                    .await
                    .unwrap_or_default();
                let idxs = conn_meta
                    .list_indexes(None, schema_for_cols.as_deref(), &tbl_for_cols)
                    .await
                    .unwrap_or_default();
                let ddl = conn_meta
                    .get_table_ddl(None, schema_for_cols.as_deref(), &tbl_for_cols)
                    .await
                    .ok()
                    .flatten();

                this.update(cx, |app, cx| {
                    let mut cols = cols;
                    ColumnInfo::apply_primary_key_index(&mut cols, &idxs);
                    if let Ok(mut cache) = app.sql_metadata_cache.write() {
                        cache.set_columns_for_table(&tbl_for_cols, cols.clone());
                    }
                    if app.selected_table.as_deref() == Some(&tbl_for_cols) {
                        app.schema_columns = cols;
                        app.schema_indexes = idxs;
                        app.schema_ddl = ddl;
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }

        let conn_id = Some(conn.config.id.clone());
        let conn_name = Some(conn.config.name.clone());
        let db_type = conn.config.db_type;

        if let Some(tab) = self.query_tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.is_executing = true;
            tab.error = None;
            tab.bottom_tab = ConsoleBottomTab::Results;
        }
        self.status_message = Some(if is_selected_exec {
            "Executing selected query...".to_string()
        } else {
            "Executing query...".to_string()
        });
        cx.notify();

        let sql_for_exec = sql.clone();
        let timeout_secs = settings.query.query_timeout_secs;
        let auto_explain = settings.query.auto_explain_slow;

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let start = std::time::Instant::now();
            let timeout_duration = std::time::Duration::from_secs(timeout_secs);
            let timed_res =
                tokio::time::timeout(timeout_duration, conn.execute_query(&sql_for_exec)).await;
            let duration = start.elapsed().as_millis() as u64;

            let res = match timed_res {
                Ok(inner) => inner,
                Err(_) => Err(crate::db::error::DbError::query(format!(
                    "Query timed out after {timeout_secs}s"
                ))),
            };

            this.update(cx, |app, cx| {
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

                if let Some(tab) = app.query_tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.is_executing = false;
                    tab.execution_time_ms = Some(duration);

                    match res {
                        Ok(qr) => {
                            let rows = qr.rows.len();
                            let dur = qr.execution_time_ms.unwrap_or(duration);
                            if let Some(mut tray) = crate::ui::tray::get_tray_status() {
                                tray.ping_ms = Some(dur);
                                tray.memory_mb = crate::ui::tray::get_process_memory_mb();
                                crate::ui::tray::update_tray_status(Some(tray));
                            }
                            let affected = qr.rows_affected;
                            let qr_arc = Arc::new(qr);
                            tab.result = Some(qr_arc.clone());
                            app.table_data = Some(qr_arc);
                            app.grid_selected_cell = None;
                            app.grid_inspector_open = false;
                            tab.error = None;
                            app.status_message = Some(match affected {
                                Some(n) if rows == 0 => {
                                    format!("Query completed: {n} row(s) affected in {dur}ms")
                                }
                                _ => format!("Query completed: {rows} rows returned in {dur}ms"),
                            });

                            // Auto explain slow queries (>500ms) if enabled in settings
                            if auto_explain && dur >= 500 {
                                app.run_explain_for_tab(tab_id, cx);
                            }
                        }
                        Err(err) => {
                            tab.error = Some(err.to_string());
                            app.status_message = Some("Query execution failed".to_string());
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Format the SQL currently in the editor.
    pub fn format_editor_sql(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active_tab) = self.active_query_tab() else {
            return;
        };
        let sql = active_tab.editor.read(cx).value().to_string();
        if sql.trim().is_empty() {
            return;
        }
        let tab_size = self.settings_manager.settings().editor.tab_size;
        let formatted = format_sql_with_indent(&sql, tab_size);
        let tab_id = active_tab.id;
        if let Some(tab) = self.query_tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.editor.update(cx, |editor, cx| {
                editor.set_value(&formatted, window, cx);
            });
        }
        self.status_message = Some("Formatted SQL".to_string());
        cx.notify();
    }

    /// Run EXPLAIN on the active tab editor SQL (or currently selected SQL) and show the plan panel.
    pub fn run_explain(&mut self, cx: &mut Context<Self>) {
        if let Some(active_tab) = self.active_query_tab() {
            let tab_id = active_tab.id;
            self.run_explain_for_tab(tab_id, cx);
        }
    }

    /// Run EXPLAIN on a specific tab's SQL
    pub fn run_explain_for_tab(&mut self, tab_id: Uuid, cx: &mut Context<Self>) {
        let Some(target_tab) = self.query_tabs.iter().find(|t| t.id == tab_id) else {
            return;
        };
        let editor_read = target_tab.editor.read(cx);
        let selected_text = editor_read.selected_text().to_string();
        let sql = if !selected_text.trim().is_empty() {
            selected_text
        } else {
            editor_read.value().to_string()
        };
        if sql.trim().is_empty() {
            return;
        }

        let Some(conn) = self
            .query_tabs
            .iter()
            .find(|t| t.id == tab_id)
            .and_then(|t| self.connection_for_tab(t))
        else {
            if let Some(tab) = self.query_tabs.iter_mut().find(|t| t.id == tab_id) {
                tab.explain_error = Some(
                    "No active database connection. Please select or create a connection first."
                        .to_string(),
                );
                tab.bottom_tab = ConsoleBottomTab::Explain;
            }
            cx.notify();
            return;
        };

        let family = conn.config.db_type.family();
        let explain_sql = wrap_explain_sql(&sql, family);
        if explain_sql.is_empty() {
            return;
        }

        if let Some(tab) = self.query_tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.is_explaining = true;
            tab.explain_error = None;
            tab.bottom_tab = ConsoleBottomTab::Explain;
        }
        self.status_message = Some("Running EXPLAIN…".to_string());
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let res = conn.execute_query(&explain_sql).await;
            this.update(cx, |app, cx| {
                if let Some(tab) = app.query_tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.is_explaining = false;
                    match res {
                        Ok(qr) => {
                            let plan = parse_explain_result(family, &qr);
                            let nodes = plan.node_count();
                            tab.explain_plan = Some(plan);
                            tab.explain_error = None;
                            app.status_message = Some(format!("Explain completed: {nodes} nodes"));
                        }
                        Err(err) => {
                            tab.explain_plan = None;
                            tab.explain_error = Some(err.to_string());
                            app.status_message = Some("Explain failed".to_string());
                        }
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
        if let Some(active_tab) = self.active_query_tab_mut() {
            active_tab.editor.update(cx, |editor, cx| {
                editor.set_value(&sql_str, window, cx);
            });
        }
        self.active_tab = WorkspaceTab::QueryConsole;
        self.run_query(Some(window), cx);
    }

    /// Load a SQL string into the editor without executing
    pub fn load_sql_into_editor(&mut self, sql: &str, window: &mut Window, cx: &mut Context<Self>) {
        let sql_str = sql.to_string();
        if let Some(active_tab) = self.active_query_tab_mut() {
            active_tab.editor.update(cx, |editor, cx| {
                editor.set_value(&sql_str, window, cx);
            });
        }
        self.active_tab = WorkspaceTab::QueryConsole;
        self.status_message = Some("Loaded query into editor".to_string());
        cx.notify();
    }

    /// Get current active QueryResult (either from inspected table or active query tab)
    pub fn current_data_result(&self) -> Option<Arc<QueryResult>> {
        self.table_data
            .clone()
            .or_else(|| self.active_query_tab().and_then(|t| t.result.clone()))
    }

    /// Export DataGrid results to system clipboard
    pub fn export_grid_data(&mut self, format: ExportFormat, cx: &mut Context<Self>) {
        let res_arc = self.current_data_result();
        let Some(data) = res_arc.as_ref() else {
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
        self.status_message = Some(format!(
            "Exported {format:?} copied to clipboard ({len} bytes)"
        ));
        cx.notify();
    }

    /// Select a grid cell and sync inspector live editor input
    pub fn select_grid_cell(
        &mut self,
        coord: GridCellCoord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.grid_selected_cell = Some(coord);
        self.grid_inspector_open = true;

        if coord.is_inserted {
            if let Some(val) = self
                .grid_changeset
                .get_inserted_cell_value(coord.row_idx, coord.col_idx)
            {
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
            let res_arc = self.current_data_result();
            if let Some(res) = res_arc.as_ref() {
                if let Some(row) = res.rows.get(coord.row_idx) {
                    if let Some(orig_val) = row.get(coord.col_idx) {
                        let eff_val = self.grid_changeset.get_effective_cell_value(
                            coord.row_idx,
                            coord.col_idx,
                            orig_val,
                        );
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
        if self
            .active_connection
            .as_ref()
            .is_some_and(|c| c.config.is_read_only)
        {
            self.status_message =
                Some("Cannot add row: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }

        let res_arc = self.current_data_result();
        let res = res_arc.as_ref();
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

            let is_sqlite = self
                .active_connection
                .as_ref()
                .map(|conn| conn.config.db_type.family() == DatabaseFamily::Sqlite)
                .unwrap_or(false);

            let is_auto = col_meta
                .map(|c| {
                    c.is_auto_increment
                        || c.data_type.to_lowercase().contains("serial")
                        || (is_sqlite
                            && c.is_primary_key
                            && c.data_type.to_lowercase().contains("int"))
                })
                .unwrap_or(false);

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
            self.grid_scroll_handle
                .set_offset(point(curr.x, -px(999999.0)));
        }

        let cur_val = self
            .grid_changeset
            .get_inserted_cell_value(insert_idx, first_editable_col);
        let display_str = cur_val
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    v.to_display_string()
                }
            })
            .unwrap_or_default();
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
    pub fn duplicate_grid_row(
        &mut self,
        coord: GridCellCoord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .active_connection
            .as_ref()
            .is_some_and(|c| c.config.is_read_only)
        {
            self.status_message =
                Some("Cannot duplicate row: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }

        let res_arc = self.current_data_result();
        let res = res_arc.as_ref();
        let Some(res) = res else {
            return;
        };

        let orig_values: Vec<QueryValue> = if coord.is_inserted {
            if let Some(ins) = self.grid_changeset.inserted_rows.get(coord.row_idx) {
                ins.values.clone()
            } else {
                return;
            }
        } else {
            let Some(row) = res.rows.get(coord.row_idx) else {
                return;
            };
            let mut vals = Vec::with_capacity(row.len());
            for (col_idx, orig_val) in row.iter().enumerate() {
                let eff =
                    self.grid_changeset
                        .get_effective_cell_value(coord.row_idx, col_idx, orig_val);
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

            let is_sqlite = self
                .active_connection
                .as_ref()
                .map(|conn| conn.config.db_type.family() == DatabaseFamily::Sqlite)
                .unwrap_or(false);

            let is_auto = col_meta
                .map(|c| {
                    c.is_auto_increment
                        || c.data_type.to_lowercase().contains("serial")
                        || (is_sqlite
                            && c.is_primary_key
                            && c.data_type.to_lowercase().contains("int"))
                })
                .unwrap_or(false);

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

        let cur_val = self
            .grid_changeset
            .get_inserted_cell_value(insert_idx, first_editable_col);
        let display_str = cur_val
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    v.to_display_string()
                }
            })
            .unwrap_or_default();
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
                self.grid_selected_cell =
                    Some(GridCellCoord::inserted(coord.row_idx - 1, coord.col_idx));
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
        if self
            .active_connection
            .as_ref()
            .is_some_and(|c| c.config.is_read_only)
        {
            self.status_message =
                Some("Cannot modify data: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }

        let Some(coord) = self.grid_selected_cell else {
            return;
        };
        let new_text = self.grid_cell_edit_input.read(cx).value().to_string();

        let res_arc = self.current_data_result();
        let res = res_arc.as_ref();
        let col_type = res
            .and_then(|r| r.column_types.get(coord.col_idx))
            .map(|s| s.as_str())
            .or_else(|| {
                self.schema_columns
                    .get(coord.col_idx)
                    .map(|c| c.data_type.as_str())
            })
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

        let Some(res) = res else {
            return;
        };
        let Some(row) = res.rows.get(coord.row_idx) else {
            return;
        };
        let Some(orig_val) = row.get(coord.col_idx) else {
            return;
        };
        let col_name = res
            .columns
            .get(coord.col_idx)
            .cloned()
            .unwrap_or_else(|| format!("col_{}", coord.col_idx));

        let new_val = parse_edited_query_value(&new_text, orig_val, col_type);

        self.grid_changeset.stage_cell_update(
            coord.row_idx,
            coord.col_idx,
            col_name,
            orig_val.clone(),
            new_val,
        );
        let (updates, deletes, inserts) = self.grid_changeset.change_summary();
        self.status_message = Some(format!(
            "Staged change: {updates} update(s), {deletes} deletion(s), {inserts} new row(s) pending"
        ));
        cx.notify();
    }

    /// Set selected cell to NULL
    pub fn set_grid_cell_null(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .active_connection
            .as_ref()
            .is_some_and(|c| c.config.is_read_only)
        {
            self.status_message =
                Some("Cannot modify data: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }
        let Some(coord) = self.grid_selected_cell else {
            return;
        };

        if coord.is_inserted {
            self.grid_changeset.set_inserted_cell_value(
                coord.row_idx,
                coord.col_idx,
                QueryValue::Null,
            );
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

        let res_arc = self.current_data_result();
        let res = res_arc.as_ref();
        let Some(res) = res else {
            return;
        };
        let Some(row) = res.rows.get(coord.row_idx) else {
            return;
        };
        let Some(orig_val) = row.get(coord.col_idx) else {
            return;
        };
        let col_name = res
            .columns
            .get(coord.col_idx)
            .cloned()
            .unwrap_or_else(|| format!("col_{}", coord.col_idx));

        self.grid_changeset.stage_cell_update(
            coord.row_idx,
            coord.col_idx,
            col_name,
            orig_val.clone(),
            QueryValue::Null,
        );
        self.grid_cell_edit_input.update(cx, |inp, cx| {
            inp.set_value("", window, cx);
        });
        let (updates, deletes, inserts) = self.grid_changeset.change_summary();
        self.status_message = Some(format!(
            "Staged NULL: {updates} update(s), {deletes} deletion(s), {inserts} new row(s) pending"
        ));
        cx.notify();
    }

    /// Revert a dirty cell to its original value
    pub fn revert_grid_cell(
        &mut self,
        row_idx: usize,
        col_idx: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.grid_changeset.revert_cell(row_idx, col_idx);
        let res_arc = self.current_data_result();
        let res = res_arc.as_ref();
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
        self.status_message = Some(format!(
            "Reverted cell. {updates} update(s), {deletes} deletion(s), {inserts} new row(s) pending"
        ));
        cx.notify();
    }

    /// Toggle a row's staged deletion status
    pub fn toggle_delete_grid_row(&mut self, row_idx: usize, cx: &mut Context<Self>) {
        if self
            .active_connection
            .as_ref()
            .is_some_and(|c| c.config.is_read_only)
        {
            self.status_message =
                Some("Cannot modify data: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }
        let res_arc = self.current_data_result();
        let res = res_arc.as_ref();
        let Some(res) = res else {
            return;
        };
        let Some(row) = res.rows.get(row_idx) else {
            return;
        };

        let now_deleted = self.grid_changeset.toggle_delete_row(row_idx, row);
        let (updates, deletes, inserts) = self.grid_changeset.change_summary();
        if now_deleted {
            self.status_message = Some(format!(
                "Row #{} staged for deletion. Review the DELETE statement to remove it. Total: {updates} update(s), {deletes} deletion(s), {inserts} new row(s)",
                row_idx + 1
            ));
            // Staging alone left the row in the database, so open the review
            // modal immediately and let the user commit the DELETE.
            self.open_sql_review_modal(cx);
        } else {
            self.status_message = Some(format!(
                "Restored row #{}. Total: {updates} update(s), {deletes} deletion(s), {inserts} new row(s)",
                row_idx + 1
            ));
            if !self.grid_changeset.is_dirty() {
                self.sql_review_modal_open = false;
                self.sql_review_plan = None;
            } else if self.sql_review_modal_open {
                self.open_sql_review_modal(cx);
                return;
            }
            cx.notify();
        }
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
                let res_arc = self.current_data_result();
                let res = res_arc.as_ref();
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
            self.status_message =
                Some("Cannot save changes: Connection is in read-only mode".to_string());
            cx.notify();
            return;
        }

        let mut table_name = self.selected_table.clone();
        let mut schema_name = None;

        if table_name.is_none() {
            let sql = self
                .active_query_tab()
                .map(|t| t.editor.read(cx).value().to_string())
                .unwrap_or_default();
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
            schema_name = self
                .active_tables
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(&table_name))
                .and_then(|t| t.schema.clone());
        }

        let family = conn.config.db_type.family();

        let empty_cols = Vec::new();
        let empty_rows = Vec::new();
        let current_res = self.current_data_result();
        let (grid_cols, orig_rows) = if let Some(ref res) = current_res.as_ref() {
            (&res.columns, &res.rows)
        } else {
            (&empty_cols, &empty_rows)
        };

        let mut review_columns = self.schema_columns.clone();
        ColumnInfo::apply_primary_key_index(&mut review_columns, &self.schema_indexes);
        let plan = generate_review_plan(
            &table_name,
            schema_name.as_deref(),
            family,
            &review_columns,
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
        // Re-running a DELETE/UPDATE from the editor after the batch already
        // applied it reports 0 rows affected and replaces the grid with that
        // success panel. Only reload the editor when it is a row-returning query.
        let reload_sql = if self.active_tab == WorkspaceTab::QueryConsole {
            self.active_query_tab()
                .map(|t| t.editor.read(cx).value().trim().to_string())
                .filter(|s| {
                    !s.is_empty()
                        && crate::db::safety::QuerySafetyValidator::is_row_returning_script(s)
                })
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
                        if app.schema_is_editing {
                            app.schema_is_editing = false;
                            app.schema_edit_columns.clear();
                            if let Some(ref tbl) = table_name {
                                app.refresh_table_schema_after_alteration(tbl.clone(), cx);
                            }
                        }
                        if let Some(qr) = reloaded {
                            let qr_arc = Arc::new(qr);
                            app.table_data = Some(qr_arc.clone());
                            if let Some(tab) = app.active_query_tab_mut() {
                                tab.result = Some(qr_arc);
                            }
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

    /// Refresh table columns, indexes, and DDL metadata after a schema alteration
    pub fn refresh_table_schema_after_alteration(
        &mut self,
        table_name: String,
        cx: &mut Context<Self>,
    ) {
        let Some(conn) = self.active_connection.clone() else {
            return;
        };
        let schema = self
            .active_tables
            .iter()
            .find(|t| t.name == table_name)
            .and_then(|t| t.schema.clone());

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let cols = conn
                .list_columns(None, schema.as_deref(), &table_name)
                .await
                .unwrap_or_default();
            let idxs = conn
                .list_indexes(None, schema.as_deref(), &table_name)
                .await
                .unwrap_or_default();
            let ddl = conn
                .get_table_ddl(None, schema.as_deref(), &table_name)
                .await
                .ok()
                .flatten();

            this.update(cx, |app, cx| {
                let mut cols = cols;
                ColumnInfo::apply_primary_key_index(&mut cols, &idxs);
                if let Ok(mut cache) = app.sql_metadata_cache.write() {
                    cache.set_columns_for_table(&table_name, cols.clone());
                }
                if app.selected_table.as_deref() == Some(&table_name) {
                    app.schema_columns = cols;
                    app.schema_indexes = idxs;
                    app.schema_ddl = ddl;
                }
                app.refresh_schema(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Enter interactive schema structure editing mode for the active table
    pub fn start_schema_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.schema_columns.is_empty() {
            self.status_message = Some("No columns available to edit".to_string());
            cx.notify();
            return;
        }

        self.schema_edit_columns = self
            .schema_columns
            .iter()
            .map(|col| {
                let name = cx.new(|cx| InputState::new(window, cx).default_value(&col.name));
                let data_type =
                    cx.new(|cx| InputState::new(window, cx).default_value(&col.data_type));
                let default_val = cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(col.default_value.as_deref().unwrap_or(""))
                });
                let comment = cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(col.description.as_deref().unwrap_or(""))
                });

                cx.subscribe(&name, |_, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
                .detach();
                cx.subscribe(&data_type, |_, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
                .detach();
                cx.subscribe(&default_val, |_, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
                .detach();
                cx.subscribe(&comment, |_, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
                .detach();

                SchemaEditColumnState {
                    original_name: Some(col.name.clone()),
                    name,
                    data_type,
                    is_primary_key: col.is_primary_key,
                    is_nullable: col.is_nullable,
                    is_auto_increment: col.is_auto_increment,
                    default_val,
                    comment,
                    is_deleted: false,
                }
            })
            .collect();

        self.schema_is_editing = true;
        self.status_message = Some("Entered table structure edit mode".to_string());
        cx.notify();
    }

    /// Discard all uncommitted schema changes and exit edit mode
    pub fn cancel_schema_editing(&mut self, cx: &mut Context<Self>) {
        self.schema_is_editing = false;
        self.schema_edit_columns.clear();
        self.status_message = Some("Discarded table structure changes".to_string());
        cx.notify();
    }

    /// Add a new column to the schema editor
    pub fn add_schema_edit_column(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

        let new_col_name = format!("new_col_{}", self.schema_edit_columns.len() + 1);

        let name_inp = cx.new(|cx| InputState::new(window, cx).default_value(&new_col_name));
        let type_inp = cx.new(|cx| InputState::new(window, cx).default_value(default_type));
        let def_inp = cx.new(|cx| InputState::new(window, cx).default_value(""));
        let comm_inp = cx.new(|cx| InputState::new(window, cx).default_value(""));

        cx.subscribe(&name_inp, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&type_inp, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&def_inp, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&comm_inp, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();

        self.schema_edit_columns.push(SchemaEditColumnState {
            original_name: None,
            name: name_inp,
            data_type: type_inp,
            is_primary_key: false,
            is_nullable: true,
            is_auto_increment: false,
            default_val: def_inp,
            comment: comm_inp,
            is_deleted: false,
        });

        cx.notify();
    }

    pub fn toggle_schema_column_delete(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.schema_edit_columns.len() {
            let is_new = self.schema_edit_columns[idx].original_name.is_none();
            if is_new && !self.schema_edit_columns[idx].is_deleted {
                self.schema_edit_columns.remove(idx);
            } else {
                self.schema_edit_columns[idx].is_deleted =
                    !self.schema_edit_columns[idx].is_deleted;
            }
            cx.notify();
        }
    }

    pub fn toggle_schema_column_nullable(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.schema_edit_columns.len() {
            let cur = self.schema_edit_columns[idx].is_nullable;
            if cur {
                self.schema_edit_columns[idx].is_nullable = false;
            } else if !self.schema_edit_columns[idx].is_primary_key {
                self.schema_edit_columns[idx].is_nullable = true;
            }
            cx.notify();
        }
    }

    pub fn toggle_schema_column_pk(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.schema_edit_columns.len() {
            let cur = self.schema_edit_columns[idx].is_primary_key;
            self.schema_edit_columns[idx].is_primary_key = !cur;
            if !cur {
                self.schema_edit_columns[idx].is_nullable = false;
            }
            cx.notify();
        }
    }

    pub fn toggle_schema_column_auto_increment(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.schema_edit_columns.len() {
            let cur = self.schema_edit_columns[idx].is_auto_increment;
            self.schema_edit_columns[idx].is_auto_increment = !cur;
            if !cur {
                self.schema_edit_columns[idx].is_primary_key = true;
                self.schema_edit_columns[idx].is_nullable = false;
            }
            cx.notify();
        }
    }

    pub fn collect_alter_column_targets(&self, cx: &App) -> Vec<AlterColumnTarget> {
        let mut targets = Vec::new();
        for col in &self.schema_edit_columns {
            if col.is_deleted {
                continue;
            }
            let name = col.name.read(cx).value().trim().to_string();
            if name.is_empty() {
                continue;
            }
            let data_type = col.data_type.read(cx).value().trim().to_string();
            let def_val = {
                let s = col.default_val.read(cx).value().trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            };
            let comment = {
                let s = col.comment.read(cx).value().trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            };

            let def = ColumnDef::new(name, data_type)
                .primary_key(col.is_primary_key)
                .nullable(col.is_nullable)
                .auto_increment(col.is_auto_increment)
                .default_value(def_val)
                .comment(comment);

            targets.push(AlterColumnTarget::new(col.original_name.clone(), def));
        }
        targets
    }

    pub fn count_pending_schema_alterations(&self, cx: &App) -> usize {
        if !self.schema_is_editing {
            return 0;
        }
        let Some(ref tbl) = self.selected_table else {
            return 0;
        };
        let family = self
            .active_connection
            .as_ref()
            .map(|c| c.config.db_type.family())
            .unwrap_or(DatabaseFamily::Sqlite);

        let targets = self.collect_alter_column_targets(cx);
        let plan = generate_alter_table_plan(tbl, None, family, &self.schema_columns, &targets);
        plan.alterations.len()
    }

    pub fn review_schema_alterations(&mut self, cx: &mut Context<Self>) {
        let Some(conn) = self.active_connection.as_ref() else {
            self.status_message = Some("No active database connection".to_string());
            cx.notify();
            return;
        };

        if conn.config.is_read_only {
            self.status_message =
                Some("Cannot alter table: connection is in Read-Only mode".to_string());
            cx.notify();
            return;
        }

        let Some(ref tbl) = self.selected_table else {
            return;
        };

        let family = conn.config.db_type.family();
        let schema = self
            .active_tables
            .iter()
            .find(|t| t.name == *tbl)
            .and_then(|t| t.schema.as_deref());

        let targets = self.collect_alter_column_targets(cx);
        let alter_plan =
            generate_alter_table_plan(tbl, schema, family, &self.schema_columns, &targets);

        if alter_plan.alterations.is_empty() {
            self.status_message = Some("No schema alterations detected to apply".to_string());
            cx.notify();
            return;
        }

        let review_plan = alter_plan.to_review_plan();
        self.sql_review_plan = Some(review_plan);
        self.sql_review_modal_open = true;
        self.sql_review_is_executing = false;
        self.sql_review_error = None;
        self.sql_review_copied = false;
        cx.notify();
    }

    /// Open the create table modal and configure default inputs according to database family
    pub fn open_create_table_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conn) = self.active_connection.as_ref() else {
            self.status_message =
                Some("Please connect to a database first before creating a table".to_string());
            cx.notify();
            return;
        };

        if conn.config.is_read_only {
            self.status_message =
                Some("Cannot create table: connection is in Read-Only mode".to_string());
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
        let col1_comment = cx.new(|cx| InputState::new(window, cx).default_value(""));

        let col2_name = cx.new(|cx| InputState::new(window, cx).default_value("name"));
        let col2_type = cx.new(|cx| InputState::new(window, cx).default_value(name_type));
        let col2_def = cx.new(|cx| InputState::new(window, cx).default_value(""));
        let col2_comment = cx.new(|cx| InputState::new(window, cx).default_value(""));

        self.create_table_columns = vec![
            CreateTableColumnState {
                name: col1_name,
                data_type: col1_type,
                is_primary_key: true,
                is_nullable: false,
                is_auto_increment: true,
                default_val: col1_def,
                comment: col1_comment,
            },
            CreateTableColumnState {
                name: col2_name,
                data_type: col2_type,
                is_primary_key: false,
                is_nullable: false,
                is_auto_increment: false,
                default_val: col2_def,
                comment: col2_comment,
            },
        ];
        self.create_table_indexes.clear();

        for col in &self.create_table_columns {
            cx.subscribe(&col.name, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();
            cx.subscribe(&col.data_type, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();
            cx.subscribe(&col.default_val, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();
            cx.subscribe(&col.comment, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();
        }

        let (initial_sql, _) = self.get_create_table_preview_sql(cx);
        self.create_table_ddl_editor.update(cx, |ed, cx| {
            ed.replace_all(&initial_sql, window, cx);
        });
        self.create_table_last_synced_sql = initial_sql;
        self.create_table_sync_status = None;
        self.create_table_is_syncing = false;

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
        let comm_inp = cx.new(|cx| InputState::new(window, cx).default_value(""));

        cx.subscribe(&name_inp, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();
        cx.subscribe(&type_inp, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();
        cx.subscribe(&def_inp, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();
        cx.subscribe(&comm_inp, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();

        self.create_table_columns.push(CreateTableColumnState {
            name: name_inp,
            data_type: type_inp,
            is_primary_key: false,
            is_nullable: true,
            is_auto_increment: false,
            default_val: def_inp,
            comment: comm_inp,
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

    /// Add a new index to the create table designer
    pub fn add_create_table_index(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.create_table_indexes.len() + 1;
        let default_name = format!("idx_tbl_col_{count}");
        // Default target column to second column if available, else first
        let default_target = if self.create_table_columns.len() > 1 {
            self.create_table_columns[1]
                .name
                .read(cx)
                .value()
                .to_string()
        } else if !self.create_table_columns.is_empty() {
            self.create_table_columns[0]
                .name
                .read(cx)
                .value()
                .to_string()
        } else {
            String::new()
        };

        let name_inp = cx.new(|cx| InputState::new(window, cx).default_value(&default_name));
        cx.subscribe(&name_inp, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();

        let cols_inp = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(&default_target)
                .placeholder("e.g. col1, col2")
        });
        cx.subscribe(&cols_inp, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();

        self.create_table_indexes.push(CreateTableIndexState {
            name: name_inp,
            index_type: TableIndexType::Normal,
            columns: cols_inp,
        });
        cx.notify();
    }

    /// Remove an index from the create table designer
    pub fn remove_create_table_index(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.create_table_indexes.len() {
            self.create_table_indexes.remove(idx);
            cx.notify();
        }
    }

    /// Toggle an index between Normal (INDEX) and Unique (UNIQUE)
    pub fn toggle_create_table_index_type(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(index_item) = self.create_table_indexes.get_mut(idx) {
            index_item.index_type = match index_item.index_type {
                TableIndexType::Normal => TableIndexType::Unique,
                TableIndexType::Unique => TableIndexType::Normal,
            };
            cx.notify();
        }
    }

    /// Toggle a column name in an index's columns specification
    pub fn toggle_create_table_index_column(
        &mut self,
        idx: usize,
        column_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index_item) = self.create_table_indexes.get(idx) {
            let current_raw = index_item.columns.read(cx).value().to_string();
            let mut cols = parse_sql_column_list(&current_raw);

            if let Some(pos) = cols
                .iter()
                .position(|c| column_matches_index_spec(c, &column_name))
            {
                cols.remove(pos);
            } else {
                cols.push(column_name);
            }

            let new_val = cols.join(", ");
            index_item.columns.update(cx, |inp, cx| {
                inp.set_value(&new_val, window, cx);
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
            let comment_raw = col.comment.read(cx).value().to_string();
            let col_comment = if comment_raw.trim().is_empty() {
                None
            } else {
                Some(comment_raw.trim().to_string())
            };

            let col_def = ColumnDef::new(col_name, col_type)
                .primary_key(col.is_primary_key)
                .nullable(col.is_nullable)
                .auto_increment(col.is_auto_increment)
                .default_value(default_val)
                .comment(col_comment);

            def = def.column(col_def);
        }

        for idx_state in &self.create_table_indexes {
            let idx_name = idx_state.name.read(cx).value().to_string();
            let cols_raw = idx_state.columns.read(cx).value().to_string();
            let cols = parse_sql_column_list(&cols_raw);
            if !cols.is_empty() {
                let idx_def = TableIndexDef::new(idx_name, cols)
                    .unique(idx_state.index_type == TableIndexType::Unique);
                def = def.index(idx_def);
            }
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

    /// Synchronize Create Table columns, types, and indexes from editable DDL text
    pub fn sync_create_table_from_ddl(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        manual: bool,
    ) -> bool {
        let current_sql = self.create_table_ddl_editor.read(cx).value().to_string();
        let trimmed_sql = current_sql.trim();
        if trimmed_sql.is_empty() {
            if manual {
                self.create_table_error = Some("DDL editor is empty".to_string());
                cx.notify();
            }
            return false;
        }

        let family = self
            .active_connection
            .as_ref()
            .map(|c| c.config.db_type.family())
            .unwrap_or(DatabaseFamily::Sqlite);

        match parse_create_table_sql(trimmed_sql, family) {
            Ok(def) => {
                self.create_table_is_syncing = true;

                // Sync Table Name
                self.create_table_name_input.update(cx, |inp, cx| {
                    inp.set_value(&def.table_name, window, cx);
                });

                // Sync Schema if present
                if let Some(ref schema) = def.schema {
                    self.create_table_schema_input.update(cx, |inp, cx| {
                        inp.set_value(schema, window, cx);
                    });
                }

                // Sync Table Comment if present
                if let Some(ref comment) = def.comment {
                    self.create_table_comment_input.update(cx, |inp, cx| {
                        inp.set_value(comment, window, cx);
                    });
                }

                // Sync Columns
                self.create_table_columns.clear();
                for col in def.columns {
                    let name_inp = cx.new(|cx| InputState::new(window, cx).default_value(&col.name));
                    let type_inp = cx.new(|cx| InputState::new(window, cx).default_value(&col.data_type));
                    let def_val_str = col.default_value.unwrap_or_default();
                    let def_inp = cx.new(|cx| InputState::new(window, cx).default_value(&def_val_str));
                    let comm_str = col.comment.unwrap_or_default();
                    let comm_inp = cx.new(|cx| InputState::new(window, cx).default_value(&comm_str));

                    cx.subscribe(&name_inp, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();
                    cx.subscribe(&type_inp, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();
                    cx.subscribe(&def_inp, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();
                    cx.subscribe(&comm_inp, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();

                    self.create_table_columns.push(CreateTableColumnState {
                        name: name_inp,
                        data_type: type_inp,
                        is_primary_key: col.is_primary_key,
                        is_nullable: col.is_nullable,
                        is_auto_increment: col.is_auto_increment,
                        default_val: def_inp,
                        comment: comm_inp,
                    });
                }

                // Sync Indexes
                self.create_table_indexes.clear();
                for idx in def.indexes {
                    let name_inp = cx.new(|cx| InputState::new(window, cx).default_value(&idx.name));
                    let cols_str = idx.columns.join(", ");
                    let cols_inp = cx.new(|cx| InputState::new(window, cx).default_value(&cols_str));

                    cx.subscribe(&name_inp, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();
                    cx.subscribe(&cols_inp, |_, _, event: &InputEvent, cx| if matches!(event, InputEvent::Change) { cx.notify(); }).detach();

                    self.create_table_indexes.push(CreateTableIndexState {
                        name: name_inp,
                        index_type: idx.index_type,
                        columns: cols_inp,
                    });
                }

                self.create_table_last_synced_sql = current_sql;
                self.create_table_sync_status = Some(Ok(format!(
                    "Synced {} column(s), {} index(es)",
                    self.create_table_columns.len(),
                    self.create_table_indexes.len()
                )));
                self.create_table_error = None;
                self.create_table_is_syncing = false;
                cx.notify();
                true
            }
            Err(err) => {
                self.create_table_sync_status = Some(Err(err.clone()));
                if manual {
                    self.create_table_error = Some(format!("DDL parse failed: {err}"));
                }
                cx.notify();
                false
            }
        }
    }

    /// Execute CREATE TABLE DDL atomically and refresh tables
    pub fn execute_create_table(&mut self, cx: &mut Context<Self>) {
        let editor_sql = self.create_table_ddl_editor.read(cx).value().to_string();
        let (sql, validation_err) = if !editor_sql.trim().is_empty() {
            (editor_sql, None)
        } else {
            self.get_create_table_preview_sql(cx)
        };
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

        let new_table_name = self
            .create_table_name_input
            .read(cx)
            .value()
            .trim()
            .to_string();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let res = conn.execute_batch(&sql).await;

            this.update(cx, |app, cx| {
                app.create_table_is_executing = false;
                match res {
                    Ok(_) => {
                        app.create_table_modal_open = false;
                        app.status_message =
                            Some(format!("Table '{new_table_name}' created successfully"));
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
        self.create_query_tab(Some("create_table.sql".to_string()), Some(&sql), window, cx);
        self.status_message = Some("Loaded Create Table DDL into new Query Tab".to_string());
        cx.notify();
    }

    /// Extract table DDL and open it in a new Query Console tab
    pub fn show_table_ddl(
        &mut self,
        table: TableInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ref conn) = self.active_connection else {
            return;
        };
        let family = conn.config.db_type.family();
        let tbl_name = table.name.clone();

        let ddl = if self.selected_table.as_deref() == Some(&tbl_name) && self.schema_ddl.is_some() {
            self.schema_ddl.clone().unwrap()
        } else if let Ok(cache) = self.sql_metadata_cache.read() {
            if let Some(cols) = cache.get_columns_for_table(&tbl_name) {
                let mut def = crate::db::sql_gen::CreateTableDef::new(&tbl_name)
                    .schema(table.schema.clone());
                for col in cols {
                    let mut cdef = crate::db::sql_gen::ColumnDef::new(&col.name, &col.data_type)
                        .primary_key(col.is_primary_key)
                        .nullable(col.is_nullable)
                        .auto_increment(col.is_auto_increment)
                        .default_value(col.default_value.clone());
                    if let Some(ref comment) = col.description {
                        cdef = cdef.comment(Some(comment.clone()));
                    }
                    def = def.column(cdef);
                }
                crate::db::sql_gen::generate_create_table_sql(&def, family)
                    .unwrap_or_else(|_| format!("-- Table: {}\n", tbl_name))
            } else {
                format!("-- DDL for {}\n-- Note: Select table in sidebar to inspect live schema", tbl_name)
            }
        } else {
            format!("-- Table: {}\n", tbl_name)
        };

        let tab_title = format!("DDL: {}", tbl_name);
        self.create_query_tab(Some(tab_title), Some(&ddl), window, cx);
        self.status_message = Some(format!("Opened DDL for {}", tbl_name));
        cx.notify();
    }

    /// Generate an INSERT INTO template for a table and copy it to clipboard
    pub fn copy_insert_template(&mut self, table: TableInfo, cx: &mut Context<Self>) {
        let Some(ref conn) = self.active_connection else {
            return;
        };
        let family = conn.config.db_type.family();
        let conn = conn.clone();
        let schema = table.schema.clone();
        let tbl_name = table.name.clone();

        let cached_cols = if self.selected_table.as_deref() == Some(&tbl_name) && !self.schema_columns.is_empty() {
            Some(self.schema_columns.clone())
        } else if let Ok(cache) = self.sql_metadata_cache.read() {
            cache.get_columns_for_table(&tbl_name).cloned()
        } else {
            None
        };

        if let Some(cols) = cached_cols {
            let template = crate::db::sql_gen::build_insert_template(&table, &cols, family);
            cx.write_to_clipboard(ClipboardItem::new_string(template));
            self.status_message = Some(format!("Copied INSERT template for {tbl_name}"));
            cx.notify();
            return;
        }

        self.status_message = Some(format!("Generating INSERT template for {}...", tbl_name));
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let cols = conn
                .list_columns(None, schema.as_deref(), &tbl_name)
                .await
                .unwrap_or_default();

            this.update(cx, |app, cx| {
                let template = crate::db::sql_gen::build_insert_template(&table, &cols, family);
                cx.write_to_clipboard(ClipboardItem::new_string(template));
                app.status_message = Some(format!("Copied INSERT template for {tbl_name}"));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Open Drop Table / Drop View confirmation dialog
    pub fn open_drop_table_confirm(&mut self, table: TableInfo, cx: &mut Context<Self>) {
        let family = self
            .active_connection
            .as_ref()
            .map(|c| c.config.db_type.family())
            .unwrap_or(DatabaseFamily::Sqlite);
        let qualified = table.qualified_name(family);
        let is_view = table.is_view();
        let kind = if is_view {
            ConfirmActionKind::DropView
        } else {
            ConfirmActionKind::DropTable
        };
        let sql_preview = if is_view {
            format!("DROP VIEW {qualified};")
        } else {
            format!("DROP TABLE {qualified};")
        };

        self.table_confirm_modal = Some(TableConfirmActionState {
            kind,
            table,
            database_family: family,
            sql_preview,
            is_executing: false,
            error: None,
        });
        cx.notify();
    }

    /// Open Truncate Table confirmation dialog
    pub fn open_truncate_table_confirm(&mut self, table: TableInfo, cx: &mut Context<Self>) {
        let family = self
            .active_connection
            .as_ref()
            .map(|c| c.config.db_type.family())
            .unwrap_or(DatabaseFamily::Sqlite);
        let qualified = table.qualified_name(family);
        let sql_preview = match family {
            DatabaseFamily::Sqlite => format!("DELETE FROM {qualified};"),
            _ => format!("TRUNCATE TABLE {qualified};"),
        };

        self.table_confirm_modal = Some(TableConfirmActionState {
            kind: ConfirmActionKind::TruncateTable,
            table,
            database_family: family,
            sql_preview,
            is_executing: false,
            error: None,
        });
        cx.notify();
    }

    /// Execute the confirmed table action (Drop/Truncate)
    pub fn execute_table_confirm_action(&mut self, cx: &mut Context<Self>) {
        let Some(ref mut modal_state) = self.table_confirm_modal else {
            return;
        };

        let Some(conn) = self.active_connection.clone() else {
            modal_state.error = Some("No active database connection".to_string());
            cx.notify();
            return;
        };

        if conn.config.is_read_only {
            modal_state.error = Some("Connection is in Read-Only mode".to_string());
            cx.notify();
            return;
        }

        modal_state.is_executing = true;
        modal_state.error = None;
        cx.notify();

        let sql = modal_state.sql_preview.clone();
        let kind = modal_state.kind;
        let table_name = modal_state.table.name.clone();
        let is_drop = kind.is_drop();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let res = conn.execute_batch(&sql).await;

            this.update(cx, |app, cx| match res {
                Ok(_) => {
                    app.table_confirm_modal = None;
                    let action_desc = match kind {
                        ConfirmActionKind::DropTable => {
                            format!("Table '{table_name}' dropped successfully")
                        }
                        ConfirmActionKind::DropView => {
                            format!("View '{table_name}' dropped successfully")
                        }
                        ConfirmActionKind::TruncateTable => {
                            format!("Table '{table_name}' truncated successfully")
                        }
                    };
                    app.status_message = Some(action_desc);

                    if is_drop {
                        if app.selected_table.as_deref() == Some(&table_name) {
                            app.selected_table = None;
                            app.table_data = None;
                            app.schema_columns.clear();
                            app.schema_indexes.clear();
                            app.schema_ddl = None;
                        }
                    } else if app.selected_table.as_deref() == Some(&table_name) {
                        if let Some(tbl) = app
                            .active_tables
                            .iter()
                            .find(|t| t.name == table_name)
                            .cloned()
                        {
                            app.select_table(tbl, cx);
                        }
                    }

                    app.refresh_schema(cx);
                    cx.notify();
                }
                Err(err) => {
                    if let Some(ref mut state) = app.table_confirm_modal {
                        state.is_executing = false;
                        state.error = Some(err.to_string());
                    }
                    app.status_message = Some(format!("Action failed: {err}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Clear console editor & results
    pub fn clear_console(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_query_tab_mut() {
            tab.editor.update(cx, |editor, cx| {
                editor.set_value("", window, cx);
            });
            tab.result = None;
            tab.error = None;
            tab.explain_plan = None;
            tab.explain_error = None;
            tab.execution_time_ms = None;
        }
        cx.notify();
    }

    /// Open new connection dialog without window handle (e.g. triggered from tray or action)
    pub fn open_new_connection_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog_open = true;
        self.dialog_editing_id = None;
        self.dialog_db_type = DatabaseType::Sqlite;
        self.dialog_test_result = None;
        self.dialog_is_testing = false;
        self.dialog_is_read_only = false;
        cx.notify();
    }

    /// Open Data Import Wizard Modal
    pub fn open_import_modal(&mut self, target_table: Option<String>, cx: &mut Context<Self>) {
        self.import_modal_open = true;
        self.import_step = ImportWizardStep::Step1Source;
        self.import_error = None;
        self.import_result = None;
        self.import_progress = None;
        self.import_is_executing = false;

        self.import_available_tables = self
            .active_tables
            .iter()
            .filter(|t| !t.is_view())
            .map(|t| t.name.clone())
            .collect();

        let target = target_table
            .or_else(|| self.selected_table.clone())
            .or_else(|| self.import_available_tables.first().cloned());

        if let Some(tbl) = target {
            self.select_import_table(tbl, cx);
        }

        // If file input already has a path, analyze it
        let raw_path = self
            .import_file_path_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        if !raw_path.is_empty() {
            self.inspect_import_file(cx);
        }

        cx.notify();
    }

    /// Close Data Import Wizard Modal
    pub fn close_import_modal(&mut self, cx: &mut Context<Self>) {
        self.import_modal_open = false;
        self.import_is_executing = false;
        self.import_error = None;
        cx.notify();
    }

    /// Open native file dialog to browse for import data files
    pub fn browse_import_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = rfd::FileDialog::new()
            .add_filter("Data Files (*.csv, *.tsv, *.sql)", &["csv", "tsv", "sql"])
            .add_filter("CSV Files (*.csv)", &["csv"])
            .add_filter("TSV Files (*.tsv)", &["tsv"])
            .add_filter("SQL Scripts (*.sql)", &["sql"])
            .add_filter("All Files (*.*)", &["*"])
            .pick_file();

        if let Some(path) = picked {
            let path_str = path.display().to_string();
            self.import_file_path_input.update(cx, |inp, cx| {
                inp.set_value(&path_str, window, cx);
            });
            self.import_file_path = Some(path);
            self.inspect_import_file(cx);
        }
    }

    /// Inspect and parse the selected import file
    pub fn inspect_import_file(&mut self, cx: &mut Context<Self>) {
        let raw_path = self
            .import_file_path_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        if raw_path.is_empty() {
            self.import_error = Some("Please enter or select a valid data file path".to_string());
            cx.notify();
            return;
        }

        let path = std::path::PathBuf::from(raw_path);
        if !path.exists() {
            self.import_error = Some(format!("File does not exist: {}", path.display()));
            cx.notify();
            return;
        }

        self.import_file_path = Some(path.clone());
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext == "sql" {
            self.import_format = ImportFormat::Sql;
            match CsvSniffer::inspect_sql_file(&path) {
                Ok(sql_preview) => {
                    self.import_sql_preview = Some(sql_preview);
                    self.import_csv_preview = None;
                    self.import_error = None;
                }
                Err(err) => {
                    self.import_error = Some(format!("Failed to parse SQL file: {err}"));
                }
            }
        } else {
            self.import_format = if ext == "tsv" {
                ImportFormat::Tsv
            } else {
                ImportFormat::Csv
            };
            match CsvSniffer::inspect_csv_file(
                &path,
                Some(self.import_delimiter),
                Some(self.import_encoding),
                Some(self.import_has_headers),
            ) {
                Ok(csv_preview) => {
                    self.import_delimiter = csv_preview.delimiter;
                    self.import_encoding = csv_preview.encoding;
                    self.import_has_headers = csv_preview.has_headers;
                    self.import_mappings =
                        auto_map_columns(&csv_preview.headers, &self.import_table_columns);
                    self.import_csv_preview = Some(csv_preview);
                    self.import_sql_preview = None;
                    self.import_error = None;
                }
                Err(err) => {
                    self.import_error = Some(format!("Failed to parse CSV file: {err}"));
                }
            }
        }
        cx.notify();
    }

    /// Select target database table for CSV/TSV import
    pub fn select_import_table(&mut self, table_name: String, cx: &mut Context<Self>) {
        self.import_target_table = Some(table_name.clone());
        let table_info = self
            .active_tables
            .iter()
            .find(|t| t.name == table_name)
            .cloned();
        let schema = table_info.as_ref().and_then(|t| t.schema.clone());

        if let Ok(cache) = self.sql_metadata_cache.read() {
            if let Some(cols) = cache.get_columns_for_table(&table_name) {
                self.import_table_columns = cols.clone();
                if let Some(ref preview) = self.import_csv_preview {
                    self.import_mappings =
                        auto_map_columns(&preview.headers, &self.import_table_columns);
                }
            }
        }

        if let Some(conn) = self.active_connection.clone() {
            let tbl = table_name.clone();
            cx.spawn(async move |this, cx: &mut AsyncApp| {
                let cols = conn
                    .list_columns(None, schema.as_deref(), &tbl)
                    .await
                    .unwrap_or_default();
                this.update(cx, |app, cx| {
                    if app.import_target_table.as_deref() == Some(&tbl) {
                        app.import_table_columns = cols;
                        if let Some(ref preview) = app.import_csv_preview {
                            app.import_mappings =
                                auto_map_columns(&preview.headers, &app.import_table_columns);
                        }
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    /// Update import delimiter setting
    pub fn update_import_delimiter(&mut self, delimiter: CsvDelimiter, cx: &mut Context<Self>) {
        self.import_delimiter = delimiter;
        if let Some(ref path) = self.import_file_path {
            if self.import_format != ImportFormat::Sql {
                if let Ok(csv_preview) = CsvSniffer::inspect_csv_file(
                    path,
                    Some(self.import_delimiter),
                    Some(self.import_encoding),
                    Some(self.import_has_headers),
                ) {
                    self.import_mappings =
                        auto_map_columns(&csv_preview.headers, &self.import_table_columns);
                    self.import_csv_preview = Some(csv_preview);
                }
            }
        }
        cx.notify();
    }

    /// Update import character encoding setting
    pub fn update_import_encoding(&mut self, encoding: FileEncoding, cx: &mut Context<Self>) {
        self.import_encoding = encoding;
        if let Some(ref path) = self.import_file_path {
            if self.import_format != ImportFormat::Sql {
                if let Ok(csv_preview) = CsvSniffer::inspect_csv_file(
                    path,
                    Some(self.import_delimiter),
                    Some(self.import_encoding),
                    Some(self.import_has_headers),
                ) {
                    self.import_mappings =
                        auto_map_columns(&csv_preview.headers, &self.import_table_columns);
                    self.import_csv_preview = Some(csv_preview);
                }
            }
        }
        cx.notify();
    }

    /// Toggle first line as header row
    pub fn toggle_import_headers(&mut self, has_headers: bool, cx: &mut Context<Self>) {
        self.import_has_headers = has_headers;
        if let Some(ref path) = self.import_file_path {
            if self.import_format != ImportFormat::Sql {
                if let Ok(csv_preview) = CsvSniffer::inspect_csv_file(
                    path,
                    Some(self.import_delimiter),
                    Some(self.import_encoding),
                    Some(self.import_has_headers),
                ) {
                    self.import_mappings =
                        auto_map_columns(&csv_preview.headers, &self.import_table_columns);
                    self.import_csv_preview = Some(csv_preview);
                }
            }
        }
        cx.notify();
    }

    /// Update column-to-field mapping
    pub fn update_import_mapping(
        &mut self,
        source_idx: usize,
        target_col: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(mapping) = self
            .import_mappings
            .iter_mut()
            .find(|m| m.source_index == source_idx)
        {
            mapping.target_column = target_col;
            cx.notify();
        }
    }

    /// Execute the configured import operation in the background with progress streaming
    pub fn start_import_execution(&mut self, cx: &mut Context<Self>) {
        let Some(conn) = self.active_connection.clone() else {
            self.import_error = Some("No active database connection".to_string());
            cx.notify();
            return;
        };

        let Some(path) = self.import_file_path.clone() else {
            self.import_error = Some("No input file specified".to_string());
            cx.notify();
            return;
        };

        self.import_is_executing = true;
        self.import_error = None;
        self.import_result = None;
        self.import_progress = None;
        cx.notify();

        let format = self.import_format;
        let error_policy = self.import_error_policy;

        let (progress_tx, mut progress_rx) =
            tokio::sync::mpsc::unbounded_channel::<ImportProgress>();
        let progress_cb = Arc::new(move |prog: ImportProgress| {
            let _ = progress_tx.send(prog);
        });

        if format == ImportFormat::Sql {
            cx.spawn(async move |this, cx: &mut AsyncApp| {
                let this_prog = this.clone();
                cx.spawn(async move |cx: &mut AsyncApp| {
                    while let Some(prog) = progress_rx.recv().await {
                        let res = this_prog.update(cx, |app, cx| {
                            app.import_progress = Some(prog);
                            cx.notify();
                        });
                        if res.is_err() {
                            break;
                        }
                    }
                })
                .detach();

                let res = ImportExecutor::execute_sql_import(
                    &conn,
                    &path,
                    error_policy,
                    Some(progress_cb),
                )
                .await;

                this.update(cx, |app, cx| {
                    app.import_is_executing = false;
                    match res {
                        Ok(result) => {
                            let succ = result.total_succeeded;
                            let fail = result.total_failed;
                            app.import_result = Some(result);
                            app.import_step = ImportWizardStep::Step4Done;
                            app.status_message = Some(format!(
                                "SQL script import finished: {succ} statements succeeded, {fail} failed"
                            ));
                            app.refresh_schema(cx);
                            cx.notify();
                        }
                        Err(err) => {
                            app.import_error = Some(err.to_string());
                            app.status_message = Some(format!("Import error: {err}"));
                            cx.notify();
                        }
                    }
                })
                .ok();
            })
            .detach();
        } else {
            let target_table = match self.import_target_table.clone() {
                Some(tbl) if !tbl.is_empty() => tbl,
                _ => {
                    self.import_is_executing = false;
                    self.import_error = Some("Target database table must be selected".to_string());
                    cx.notify();
                    return;
                }
            };

            let config = CsvImportConfig {
                target_table: target_table.clone(),
                mappings: self.import_mappings.clone(),
                delimiter: self.import_delimiter,
                encoding: self.import_encoding,
                has_headers: self.import_has_headers,
                batch_size: self.import_batch_size,
                error_policy,
            };

            let cols = self.import_table_columns.clone();

            cx.spawn(async move |this, cx: &mut AsyncApp| {
                let this_prog = this.clone();
                cx.spawn(async move |cx: &mut AsyncApp| {
                    while let Some(prog) = progress_rx.recv().await {
                        let res = this_prog.update(cx, |app, cx| {
                            app.import_progress = Some(prog);
                            cx.notify();
                        });
                        if res.is_err() {
                            break;
                        }
                    }
                })
                .detach();

                let res = ImportExecutor::execute_csv_import(
                    &conn,
                    &path,
                    &config,
                    &cols,
                    Some(progress_cb),
                )
                .await;

                this.update(cx, |app, cx| {
                    app.import_is_executing = false;
                    match res {
                        Ok(result) => {
                            let succ = result.total_succeeded;
                            let fail = result.total_failed;
                            app.import_result = Some(result);
                            app.import_step = ImportWizardStep::Step4Done;
                            app.status_message = Some(format!(
                                "CSV import complete: {succ} rows inserted, {fail} failed"
                            ));
                            app.refresh_schema(cx);
                            cx.notify();
                        }
                        Err(err) => {
                            app.import_error = Some(err.to_string());
                            app.status_message = Some(format!("Import error: {err}"));
                            cx.notify();
                        }
                    }
                })
                .ok();
            })
            .detach();
        }
    }

    /// View imported table data in DataGrid
    pub fn view_imported_table(&mut self, table_name: String, cx: &mut Context<Self>) {
        self.close_import_modal(cx);
        if let Some(table) = self
            .active_tables
            .iter()
            .find(|t| t.name == table_name)
            .cloned()
        {
            self.select_table(table, cx);
            self.active_tab = WorkspaceTab::DataGrid;
        }
        cx.notify();
    }

    /// Open Data & Schema Export Wizard Modal
    pub fn open_export_modal(
        &mut self,
        target_table: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.export_modal_open = true;
        self.export_step = ExportWizardStep::Step1Config;
        self.export_error = None;
        self.export_success_info = None;
        self.export_is_executing = false;
        self.export_progress_rows = 0;
        self.export_preview_content = None;

        self.export_available_tables = self
            .active_tables
            .iter()
            .map(|t| t.name.clone())
            .collect();

        let table = target_table
            .or_else(|| self.selected_table.clone())
            .or_else(|| self.export_available_tables.first().cloned())
            .unwrap_or_else(|| "exported_table".to_string());

        let family = self
            .active_connection
            .as_ref()
            .map(|c| c.config.db_type.family())
            .unwrap_or(DatabaseFamily::Sqlite);

        self.export_target_table = table.clone();
        self.export_config.table_name = table.clone();
        self.export_config.family = family;

        // Populate default file path
        let def_filename = suggested_file_name(&table, self.export_config.format);
        let base_dir = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(std::env::temp_dir);
        let full_path = base_dir.join(&def_filename);
        let path_str = full_path.display().to_string();
        self.export_file_path = Some(full_path);
        self.export_file_path_input.update(cx, |inp, cx| {
            inp.set_value(&path_str, window, cx);
        });

        // Trigger DDL fetch & preview load
        self.load_export_preview(cx);
        cx.notify();
    }

    /// Close Export Wizard Modal
    pub fn close_export_modal(&mut self, cx: &mut Context<Self>) {
        self.export_modal_open = false;
        self.export_is_executing = false;
        self.export_error = None;
        cx.notify();
    }

    /// Select export target table
    pub fn select_export_table(
        &mut self,
        table_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.export_target_table = table_name.clone();
        self.export_config.table_name = table_name.clone();
        self.export_ddl_cache = None;

        let def_filename = suggested_file_name(&table_name, self.export_config.format);
        let base_dir = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(std::env::temp_dir);
        let full_path = base_dir.join(&def_filename);
        let path_str = full_path.display().to_string();
        self.export_file_path = Some(full_path);
        self.export_file_path_input.update(cx, |inp, cx| {
            inp.set_value(&path_str, window, cx);
        });

        self.load_export_preview(cx);
        cx.notify();
    }

    /// Update export format
    pub fn update_export_format(
        &mut self,
        format: ExportFormat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.export_config.format = format;
        if format != ExportFormat::SqlDump && format != ExportFormat::SqlInsert {
            self.export_config.scope = ExportScope::DataOnly;
        }

        // Update default file extension in file path
        let def_filename = suggested_file_name(&self.export_target_table, format);
        let base_dir = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(std::env::temp_dir);
        let full_path = base_dir.join(&def_filename);
        let path_str = full_path.display().to_string();
        self.export_file_path = Some(full_path);
        self.export_file_path_input.update(cx, |inp, cx| {
            inp.set_value(&path_str, window, cx);
        });

        self.load_export_preview(cx);
        cx.notify();
    }

    /// Browse for save destination file path
    pub fn browse_export_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let def_name = suggested_file_name(&self.export_target_table, self.export_config.format);
        let ext = self.export_config.format.extension();
        let picked = rfd::FileDialog::new()
            .set_file_name(&def_name)
            .add_filter(self.export_config.format.display_name(), &[ext])
            .add_filter("All Files (*.*)", &["*"])
            .save_file();

        if let Some(path) = picked {
            let path_str = path.display().to_string();
            self.export_file_path = Some(path);
            self.export_file_path_input.update(cx, |inp, cx| {
                inp.set_value(&path_str, window, cx);
            });
            cx.notify();
        }
    }

    /// Load or regenerate live preview
    pub fn load_export_preview(&mut self, cx: &mut Context<Self>) {
        let Some(conn) = self.active_connection.clone() else {
            return;
        };

        let table_name = self.export_target_table.clone();
        if table_name.is_empty() {
            return;
        }

        let family = self.export_config.family;
        let where_clause = self.export_where_input.read(cx).value().trim().to_string();
        let limit_clause = self.export_limit_input.read(cx).value().trim().to_string();
        let limit_num: usize = limit_clause.parse().unwrap_or(20).clamp(1, 50);

        let mut query_sql = format!(
            "SELECT * FROM {}",
            crate::db::types::quote_ident(&table_name, family)
        );
        if !where_clause.is_empty() {
            if where_clause.to_uppercase().starts_with("WHERE") {
                query_sql.push_str(&format!(" {where_clause}"));
            } else {
                query_sql.push_str(&format!(" WHERE {where_clause}"));
            }
        }
        query_sql.push_str(&format!(" LIMIT {limit_num};"));

        self.export_is_loading_preview = true;
        let config = self.export_config.clone();
        let ddl_cache = self.export_ddl_cache.clone();
        let schema = self
            .active_tables
            .iter()
            .find(|t| t.name == table_name)
            .and_then(|t| t.schema.clone());

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let ddl = if let Some(cached) = ddl_cache {
                Some(cached)
            } else {
                let tbl = table_name.clone();
                conn.get_table_ddl(None, schema.as_deref(), &tbl)
                    .await
                    .ok()
                    .flatten()
            };

            let data_res = conn.execute_query(&query_sql).await.ok();

            this.update(cx, |app, cx| {
                if let Some(ref d) = ddl {
                    app.export_ddl_cache = Some(d.clone());
                }
                let preview = generate_export_preview(
                    ddl.as_deref(),
                    data_res.as_ref(),
                    &config,
                    20,
                );
                app.export_preview_content = Some(preview);
                app.export_is_loading_preview = false;
                cx.notify();
            })
            .ok();
        })
        .detach();

        cx.notify();
    }

    /// Execute the full export operation (to file or to clipboard)
    pub fn start_export_execution(&mut self, cx: &mut Context<Self>) {
        let Some(conn) = self.active_connection.clone() else {
            self.export_error = Some("No active database connection".to_string());
            cx.notify();
            return;
        };

        let table_name = self.export_target_table.clone();
        if table_name.is_empty() {
            self.export_error = Some("Target table cannot be empty".to_string());
            cx.notify();
            return;
        }

        self.export_step = ExportWizardStep::Step3Progress;
        self.export_is_executing = true;
        self.export_error = None;
        self.export_success_info = None;

        let family = self.export_config.family;
        let where_clause = self.export_where_input.read(cx).value().trim().to_string();
        let limit_clause = self.export_limit_input.read(cx).value().trim().to_string();

        let mut query_sql = format!(
            "SELECT * FROM {}",
            crate::db::types::quote_ident(&table_name, family)
        );
        if !where_clause.is_empty() {
            if where_clause.to_uppercase().starts_with("WHERE") {
                query_sql.push_str(&format!(" {where_clause}"));
            } else {
                query_sql.push_str(&format!(" WHERE {where_clause}"));
            }
        }
        if let Ok(limit_val) = limit_clause.parse::<usize>() {
            if limit_val > 0 {
                query_sql.push_str(&format!(" LIMIT {limit_val}"));
            }
        }
        query_sql.push(';');

        let destination = self.export_destination;
        let file_path = if destination == ExportDestination::File {
            let raw_path = self.export_file_path_input.read(cx).value().trim().to_string();
            if raw_path.is_empty() {
                self.export_is_executing = false;
                self.export_error = Some("Please specify an output file path".to_string());
                cx.notify();
                return;
            }
            Some(std::path::PathBuf::from(raw_path))
        } else {
            None
        };

        let config = self.export_config.clone();
        let ddl_cache = self.export_ddl_cache.clone();
        let schema = self
            .active_tables
            .iter()
            .find(|t| t.name == table_name)
            .and_then(|t| t.schema.clone());

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let start_time = std::time::Instant::now();

            let ddl = if let Some(cached) = ddl_cache {
                Some(cached)
            } else {
                let tbl = table_name.clone();
                conn.get_table_ddl(None, schema.as_deref(), &tbl)
                    .await
                    .ok()
                    .flatten()
            };

            let data_res = if config.scope != ExportScope::SchemaOnly {
                conn.execute_query(&query_sql).await
            } else {
                Ok(QueryResult::default())
            };

            let elapsed_ms = start_time.elapsed().as_millis() as u64;

            match data_res {
                Ok(data) => {
                    let total_rows = data.rows.len();
                    let dump_output = generate_table_dump(
                        ddl.as_deref(),
                        Some(&data),
                        &config,
                    );
                    let bytes = dump_output.as_bytes().len();

                    match destination {
                        ExportDestination::File => {
                            let path = file_path.clone().unwrap();
                            let write_res = std::fs::write(&path, dump_output);
                            this.update(cx, |app, cx| {
                                app.export_is_executing = false;
                                match write_res {
                                    Ok(_) => {
                                        app.export_success_info = Some(ExportSuccessInfo {
                                            rows_count: total_rows,
                                            bytes_written: bytes,
                                            elapsed_millis: elapsed_ms,
                                            file_path: Some(path.display().to_string()),
                                            copied_to_clipboard: false,
                                        });
                                        app.status_message = Some(format!(
                                            "Successfully exported {total_rows} rows to {}",
                                            path.display()
                                        ));
                                        cx.notify();
                                    }
                                    Err(e) => {
                                        app.export_error = Some(format!("Failed to write file: {e}"));
                                        cx.notify();
                                    }
                                }
                            })
                            .ok();
                        }
                        ExportDestination::Clipboard => {
                            this.update(cx, |app, cx| {
                                app.export_is_executing = false;
                                cx.write_to_clipboard(ClipboardItem::new_string(dump_output));
                                app.export_success_info = Some(ExportSuccessInfo {
                                    rows_count: total_rows,
                                    bytes_written: bytes,
                                    elapsed_millis: elapsed_ms,
                                    file_path: None,
                                    copied_to_clipboard: true,
                                });
                                app.status_message = Some(format!(
                                    "Exported {total_rows} rows copied to clipboard"
                                ));
                                cx.notify();
                            })
                            .ok();
                        }
                    }
                }
                Err(err) => {
                    this.update(cx, |app, cx| {
                        app.export_is_executing = false;
                        app.export_error = Some(err.to_string());
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();

        cx.notify();
    }

    /// Reveal exported file in system file manager (Finder / Explorer / File Manager)
    pub fn reveal_exported_file(&self, path: &str) {
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer").arg(format!("/select,\"{path}\"")).spawn();
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if let Some(parent) = std::path::Path::new(path).parent() {
                let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
            }
        }
    }

    /// Open Visual Mock Data Generator Wizard Modal
    pub fn open_mock_modal(
        &mut self,
        target_table: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.mock_modal_open = true;
        self.mock_step = MockWizardStep::Step1Config;
        self.mock_error = None;
        self.mock_result = None;
        self.mock_progress = None;
        self.mock_is_executing = false;
        self.mock_is_loading_preview = false;
        self.mock_row_count = 500;
        self.mock_batch_size = 200;

        self.mock_available_tables = self
            .active_tables
            .iter()
            .filter(|t| !t.is_view())
            .map(|t| t.name.clone())
            .collect();

        let target = target_table
            .or_else(|| self.selected_table.clone())
            .or_else(|| self.mock_available_tables.first().cloned())
            .unwrap_or_default();

        if !target.is_empty() {
            self.select_mock_table(target, cx);
        } else {
            self.mock_target_table = String::new();
            self.mock_columns.clear();
            self.mock_preview_headers.clear();
            self.mock_preview_rows.clear();
        }

        cx.notify();
    }

    /// Close Visual Mock Data Generator Wizard Modal
    pub fn close_mock_modal(&mut self, cx: &mut Context<Self>) {
        self.mock_modal_open = false;
        self.mock_is_executing = false;
        self.mock_error = None;
        cx.notify();
    }

    /// Select target table in mock data wizard and initialize column rules
    pub fn select_mock_table(&mut self, table_name: String, cx: &mut Context<Self>) {
        self.mock_target_table = table_name.clone();

        let table_info = self
            .active_tables
            .iter()
            .find(|t| t.name == table_name)
            .cloned();

        let mut found_cols = false;
        if let Ok(cache) = self.sql_metadata_cache.read() {
            if let Some(cols) = cache.get_columns_for_table(&table_name) {
                if !cols.is_empty() {
                    self.mock_columns = initialize_column_configs(cols);
                    found_cols = true;
                }
            }
        }

        if !found_cols
            && self.selected_table.as_deref() == Some(&table_name)
            && !self.schema_columns.is_empty()
        {
            self.mock_columns = initialize_column_configs(&self.schema_columns);
        }

        self.refresh_mock_preview(cx);

        // Async fetch if connection available to ensure freshest column definitions
        let schema = table_info.as_ref().and_then(|t| t.schema.clone());
        if let Some(conn) = self.active_connection.clone() {
            let tbl = table_name.clone();
            cx.spawn(async move |this, cx: &mut AsyncApp| {
                let cols = conn
                    .list_columns(None, schema.as_deref(), &tbl)
                    .await
                    .unwrap_or_default();
                this.update(cx, |app, cx| {
                    if app.mock_target_table == tbl && !cols.is_empty() {
                        app.mock_columns = initialize_column_configs(&cols);
                        app.refresh_mock_preview(cx);
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }

        cx.notify();
    }

    /// Generate or refresh preview rows using current column rules
    pub fn refresh_mock_preview(&mut self, cx: &mut Context<Self>) {
        let (headers, rows) = generate_mock_preview(&self.mock_columns, 8);
        self.mock_preview_headers = headers;
        self.mock_preview_rows = rows;
        self.mock_is_loading_preview = false;
        cx.notify();
    }

    /// Start generating and batch inserting mock data into the database
    pub fn start_mock_execution(&mut self, cx: &mut Context<Self>) {
        let conn = match self.active_connection.clone() {
            Some(c) => c,
            None => {
                self.mock_error = Some("No active database connection".to_string());
                cx.notify();
                return;
            }
        };

        let target_table = self.mock_target_table.clone();
        if target_table.is_empty() {
            self.mock_error = Some("Please select a target table".to_string());
            cx.notify();
            return;
        }

        let total_rows = self.mock_row_count;
        let batch_size = self.mock_batch_size;
        let columns = self.mock_columns.clone();

        self.mock_step = MockWizardStep::Step3Progress;
        self.mock_is_executing = true;
        self.mock_error = None;
        self.mock_result = None;
        self.mock_progress = Some(MockProgress {
            inserted: 0,
            total: total_rows,
            percent: 0.0,
        });
        cx.notify();

        let (progress_tx, mut progress_rx) =
            tokio::sync::mpsc::unbounded_channel::<MockProgress>();

        let tbl_name = target_table.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let this_prog = this.clone();
            cx.spawn(async move |cx: &mut AsyncApp| {
                while let Some(prog) = progress_rx.recv().await {
                    let res = this_prog.update(cx, |app, cx| {
                        app.mock_progress = Some(prog);
                        cx.notify();
                    });
                    if res.is_err() {
                        break;
                    }
                }
            })
            .detach();

            let res = execute_mock_seeding(
                &conn,
                &target_table,
                &columns,
                total_rows,
                batch_size,
                move |prog| {
                    let _ = progress_tx.send(prog);
                },
            )
            .await;

            this.update(cx, |app, cx| {
                app.mock_is_executing = false;
                match res {
                    Ok(result) => {
                        let inserted = result.total_inserted;
                        let secs = result.elapsed_ms as f64 / 1000.0;
                        app.mock_result = Some(result);
                        app.status_message = Some(format!(
                            "Mock data generation complete: {inserted} rows inserted in {secs:.2}s"
                        ));
                        // Auto-refresh table grid if current table is open
                        if app.selected_table.as_deref() == Some(&tbl_name) {
                            if let Some(t) = app.active_tables.iter().find(|t| t.name == tbl_name).cloned() {
                                app.select_table(t, cx);
                            }
                        }
                        cx.notify();
                    }
                    Err(err) => {
                        app.mock_error = Some(err.to_string());
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
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
    pub fn open_edit_connection_dialog(
        &mut self,
        conn_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        if self
            .active_connection
            .as_ref()
            .is_some_and(|c| c.config.id == conn_id)
        {
            self.disconnect(cx);
        }
        let config_name = self
            .manager
            .get_config(conn_id)
            .map(|c| c.name)
            .unwrap_or_else(|| conn_id.to_string());
        if let Err(err) = self.manager.delete_config(conn_id) {
            self.status_message = Some(format!("Failed to delete profile: {err}"));
            cx.notify();
            return;
        }
        self.saved_connections = self.manager.list_configs();
        if self
            .settings_manager
            .settings()
            .last_connection_id
            .as_deref()
            == Some(conn_id)
        {
            self.settings_manager.settings_mut().last_connection_id = None;
            let _ = self.settings_manager.save();
        }
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
    pub fn set_dialog_db_type(
        &mut self,
        db_type: DatabaseType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
                let path = if db_path.trim().is_empty() {
                    ":memory:".to_string()
                } else {
                    db_path
                };
                let mut c = ConnectionConfig::sqlite(name, path);
                c.db_type = self.dialog_db_type;
                c
            }
            DatabaseFamily::Postgres => {
                let host = self.dialog_host_input.read(cx).value().to_string();
                let port_str = self.dialog_port_input.read(cx).value().to_string();
                let port = port_str
                    .trim()
                    .parse::<u16>()
                    .unwrap_or_else(|_| self.dialog_db_type.default_port());
                let db = self.dialog_database_input.read(cx).value().to_string();
                let user = self.dialog_user_input.read(cx).value().to_string();
                let pass_str = self.dialog_pass_input.read(cx).value().to_string();
                let pass = if pass_str.is_empty() {
                    None
                } else {
                    Some(pass_str)
                };
                let mut c = ConnectionConfig::postgres(name, host, port, db, user, pass);
                c.db_type = self.dialog_db_type;
                c
            }
            DatabaseFamily::MySql => {
                let host = self.dialog_host_input.read(cx).value().to_string();
                let port_str = self.dialog_port_input.read(cx).value().to_string();
                let port = port_str
                    .trim()
                    .parse::<u16>()
                    .unwrap_or_else(|_| self.dialog_db_type.default_port());
                let db = self.dialog_database_input.read(cx).value().to_string();
                let user = self.dialog_user_input.read(cx).value().to_string();
                let pass_str = self.dialog_pass_input.read(cx).value().to_string();
                let pass = if pass_str.is_empty() {
                    None
                } else {
                    Some(pass_str)
                };
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
                        let version = status
                            .server_version
                            .unwrap_or_else(|| "Unknown".to_string());
                        let ping = status.ping_ms.unwrap_or(0);
                        app.dialog_test_result =
                            Some(Ok(format!("Connected! {version} ({ping} ms)")));
                    }
                    Err(err) => {
                        app.dialog_test_result = Some(Err(format!("Connection failed: {err}")));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
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
        grid_data: Option<Arc<QueryResult>>,
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
                    this.status_message =
                        Some(format!("Copied value of column '{col_name}' ({len} chars)"));
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
                    this.status_message =
                        Some(format!("Copied row #{} as JSON ({len} bytes)", row_idx + 1));
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
                    this.status_message =
                        Some(format!("Copied row #{} as TSV ({len} bytes)", row_idx + 1));
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

        let on_import = {
            let handle = app_handle.clone();
            let cur_tbl = self.selected_table.clone();
            move |_: &mut Window, cx: &mut App| {
                let target = cur_tbl.clone();
                handle.update(cx, |this, cx| {
                    this.open_import_modal(target, cx);
                });
            }
        };

        let on_set_filter = {
            let handle = app_handle.clone();
            move |flt: String, window: &mut Window, cx: &mut App| {
                handle.update(cx, |this, cx| {
                    this.grid_filter = flt.clone();
                    this.grid_page = 0;
                    this.grid_filter_input.update(cx, |inp, cx| {
                        inp.set_value(&flt, window, cx);
                    });
                    cx.notify();
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
            .filter_input(Some(self.grid_filter_input.clone()))
            .selected_cell(self.grid_selected_cell)
            .inspector_open(self.grid_inspector_open)
            .modal_open(self.grid_modal_open)
            .json_pretty(self.grid_json_pretty)
            .changeset(self.grid_changeset.clone())
            .read_only(is_read_only)
            .cell_edit_input(Some(self.grid_cell_edit_input.clone()))
            .inspector_split(Some(self.grid_inspector_split.clone()))
            .on_set_filter(on_set_filter)
            .on_sort(on_sort)
            .on_page_change(on_page)
            .on_export(on_export)
            .on_import(on_import)
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_conn_id = self.active_connection.as_ref().map(|c| c.config.id.clone());
        let is_connected = self.active_connection.is_some();
        let is_read_only = self
            .active_connection
            .as_ref()
            .map(|c| c.config.is_read_only)
            .unwrap_or(false);
        let conn_name = self
            .active_connection
            .as_ref()
            .map(|c| c.config.name.clone());
        let db_type_str = self
            .active_connection
            .as_ref()
            .map(|c| c.config.db_type.to_string());
        let active_status = self
            .active_connection
            .as_ref()
            .and_then(|c| c.status.clone());

        let app_handle = cx.entity().clone();

        let logo_img = Arc::new(Image {
            format: ImageFormat::Png,
            bytes: LOGO_PNG_BYTES.to_vec(),
            id: 0x7a716c63726162,
        });

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
                        .child(img(logo_img).size(px(18.0)).rounded(px(3.0)))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child("zqlcrab"),
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
        .language(self.settings_manager.settings().language)
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
        .on_view_schema({
            let handle = app_handle.clone();
            move |table, _, cx| {
                handle.update(cx, |this, cx| {
                    this.select_table(table, cx);
                    this.active_tab = WorkspaceTab::Schema;
                    cx.notify();
                });
            }
        })
        .on_query_table({
            let handle = app_handle.clone();
            move |table, window, cx| {
                handle.update(cx, |this, cx| {
                    let family = this
                        .active_connection
                        .as_ref()
                        .map(|c| c.config.db_type.family())
                        .unwrap_or(DatabaseFamily::Sqlite);
                    let qualified = table.qualified_name(family);
                    let sql = format!("SELECT * FROM {qualified} LIMIT 100;\n");
                    let tab_title = format!("{}.sql", table.name);
                    this.create_query_tab(Some(tab_title), Some(&sql), window, cx);
                });
            }
        })
        .on_show_ddl({
            let handle = app_handle.clone();
            move |table, window, cx| {
                handle.update(cx, |this, cx| {
                    this.show_table_ddl(table, window, cx);
                });
            }
        })
        .on_copy_table_name({
            let handle = app_handle.clone();
            move |text, _, cx| {
                handle.update(cx, |this, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                    this.status_message = Some(format!("Copied to clipboard: {text}"));
                    cx.notify();
                });
            }
        })
        .on_copy_insert_template({
            let handle = app_handle.clone();
            move |table, _, cx| {
                handle.update(cx, |this, cx| {
                    this.copy_insert_template(table, cx);
                });
            }
        })
        .on_truncate_table({
            let handle = app_handle.clone();
            move |table, _, cx| {
                handle.update(cx, |this, cx| {
                    this.open_truncate_table_confirm(table, cx);
                });
            }
        })
        .on_drop_table({
            let handle = app_handle.clone();
            move |table, _, cx| {
                handle.update(cx, |this, cx| {
                    this.open_drop_table_confirm(table, cx);
                });
            }
        })
        .on_import_table({
            let handle = app_handle.clone();
            move |table, _, cx| {
                handle.update(cx, |this, cx| {
                    this.open_import_modal(Some(table.name), cx);
                });
            }
        })
        .on_export_table({
            let handle = app_handle.clone();
            move |table, window, cx| {
                handle.update(cx, |this, cx| {
                    this.open_export_modal(Some(table.name), window, cx);
                });
            }
        })
        .on_mock_data_table({
            let handle = app_handle.clone();
            move |table, _, cx| {
                handle.update(cx, |this, cx| {
                    this.open_mock_modal(Some(table.name), cx);
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
        let lang = self.settings_manager.settings().language;

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
                        let console_label = t("workspace.console", lang);
                        Button::new("tab_console")
                            .small()
                            .ghost()
                            .flex_shrink(1.0)
                            .min_w(px(36.0))
                            .overflow_hidden()
                            .icon(IconName::Terminal)
                            .label(console_label)
                            .tooltip(console_label)
                            .border_b_2()
                            .border_color(if is_active {
                                ThemeColors::PRIMARY_BORDER
                            } else {
                                ThemeColors::TRANSPARENT
                            })
                            .on_click(move |_, _, cx| {
                                handle.update(cx, |this, cx| {
                                    this.active_tab = WorkspaceTab::QueryConsole;
                                    this.active_nav = ActivityNav::Console;
                                    cx.notify();
                                });
                            })
                    })
                    .child({
                        let is_active = self.active_tab == WorkspaceTab::DataGrid;
                        let handle = app_handle.clone();
                        let data_str = t("workspace.data", lang);
                        let label = match selected_tbl_label {
                            Some(name) => format!("{data_str} · {name}"),
                            None => data_str.to_string(),
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
                                ThemeColors::TRANSPARENT
                            })
                            .on_click(move |_, _, cx| {
                                handle.update(cx, |this, cx| {
                                    this.active_tab = WorkspaceTab::DataGrid;
                                    this.active_nav = ActivityNav::Databases;
                                    cx.notify();
                                });
                            })
                    })
                    .child({
                        let is_active = self.active_tab == WorkspaceTab::Schema;
                        let handle = app_handle.clone();
                        let schema_str = t("workspace.schema", lang);
                        let label = match selected_tbl_label {
                            Some(name) => format!("{schema_str} · {name}"),
                            None => schema_str.to_string(),
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
                                ThemeColors::TRANSPARENT
                            })
                            .on_click(move |_, _, cx| {
                                handle.update(cx, |this, cx| {
                                    this.active_tab = WorkspaceTab::Schema;
                                    this.active_nav = ActivityNav::Databases;
                                    cx.notify();
                                });
                            })
                    })
                    .child({
                        let is_active = self.active_tab == WorkspaceTab::History;
                        let handle = app_handle.clone();
                        let count = self.history_manager.items().len();
                        let hist_str = t("workspace.history", lang);
                        let label = format!("{hist_str} ({count})");
                        let tooltip = format!("{hist_str} ({count})");
                        Button::new("tab_history")
                            .small()
                            .ghost()
                            .flex_shrink(1.0)
                            .min_w(px(36.0))
                            .overflow_hidden()
                            .icon(IconName::Clock)
                            .label(label)
                            .tooltip(tooltip)
                            .border_b_2()
                            .border_color(if is_active {
                                ThemeColors::PRIMARY_BORDER
                            } else {
                                ThemeColors::TRANSPARENT
                            })
                            .on_click(move |_, _, cx| {
                                handle.update(cx, |this, cx| {
                                    this.active_tab = WorkspaceTab::History;
                                    this.active_nav = ActivityNav::Databases;
                                    cx.notify();
                                });
                            })
                    })
                    .when(
                        !self
                            .settings_manager
                            .settings()
                            .appearance
                            .show_activity_bar,
                        |this| {
                            let handle = app_handle.clone();
                            this.child(
                                h_flex().ml_auto().child(
                                    Button::new("btn_open_settings_fallback")
                                        .ghost()
                                        .small()
                                        .icon(IconName::Settings)
                                        .tooltip("Settings")
                                        .on_click(move |_, _, cx| {
                                            handle.update(cx, |this, cx| {
                                                this.active_nav = ActivityNav::Settings;
                                                cx.notify();
                                            });
                                        }),
                                ),
                            )
                        },
                    ),
            );

        // Workspace main content
        let main_content = if !is_connected && self.saved_connections.is_empty() {
            let handle = app_handle.clone();
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    Icon::new(IconName::Database)
                        .size(px(56.0))
                        .text_color(ThemeColors::TEXT_FAINT),
                )
                .child(
                    v_flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child("No connections yet"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ThemeColors::TEXT_MUTED)
                                .child(
                                    "Add your first database connection to start exploring schemas and queries.",
                                ),
                        ),
                )
                .child(
                    Button::new("empty_add_conn")
                        .primary()
                        .icon(IconName::Plus)
                        .label("Add connection")
                        .on_click(move |_, window, cx| {
                            handle.update(cx, |this, cx| {
                                this.open_connection_dialog(window, cx);
                            });
                        }),
                )
                .into_any_element()
        } else {
            match self.active_tab {
                WorkspaceTab::QueryConsole => {
                    let active_query_tab = self.active_query_tab().cloned();
                    let tab_headers: Vec<QueryTabHeader> = self
                        .query_tabs
                        .iter()
                        .map(|t| {
                            let conn_name = t.connection_profile_id.as_ref().and_then(|id| {
                                self.saved_connections
                                    .iter()
                                    .find(|c| &c.id == id)
                                    .map(|c| c.name.clone())
                            });
                            QueryTabHeader {
                                id: t.id,
                                title: t.title.clone(),
                                connection_name: conn_name,
                                is_executing: t.is_executing,
                                has_error: t.error.is_some() || t.explain_error.is_some(),
                            }
                        })
                        .collect();
                    let active_tab_id = self.active_query_tab_id;

                    let on_select_tab = {
                        let handle = app_handle.clone();
                        move |tab_id: Uuid, _: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.switch_query_tab(tab_id, cx);
                            });
                        }
                    };
                    let on_close_tab = {
                        let handle = app_handle.clone();
                        move |tab_id: Uuid, window: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.close_query_tab(tab_id, window, cx);
                            });
                        }
                    };
                    let on_new_tab = {
                        let handle = app_handle.clone();
                        move |window: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.create_query_tab(None, None, window, cx);
                            });
                        }
                    };
                    let on_run = {
                        let handle = app_handle.clone();
                        move |window: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.run_query(Some(window), cx);
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
                                if let Some(active) = this.active_query_tab_mut() {
                                    active.bottom_tab = tab;
                                    cx.notify();
                                }
                            });
                        }
                    };
                    let on_explain_view = {
                        let handle = app_handle.clone();
                        move |view: ExplainViewMode, _: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                if let Some(active) = this.active_query_tab_mut() {
                                    active.explain_view = view;
                                    cx.notify();
                                }
                            });
                        }
                    };

                    let active_tab_ref = self
                        .query_tabs
                        .iter()
                        .find(|t| t.id == self.active_query_tab_id)
                        .or_else(|| self.query_tabs.first())
                        .expect("at least one query tab must exist");

                    let connection_label = match active_tab_ref
                        .connection_profile_id
                        .as_ref()
                        .and_then(|id| self.saved_connections.iter().find(|c| &c.id == id))
                    {
                        Some(cfg) if !cfg.database.is_empty() => {
                            Some(format!("{} / {}", cfg.name, cfg.database))
                        }
                        Some(cfg) => Some(cfg.name.clone()),
                        None => match (
                            self.active_connection
                                .as_ref()
                                .map(|c| c.config.name.clone()),
                            self.active_connection
                                .as_ref()
                                .map(|c| c.config.database.clone()),
                        ) {
                            (Some(name), Some(db)) if !db.is_empty() => {
                                Some(format!("{name} / {db}"))
                            }
                            (Some(name), _) => Some(name),
                            _ => None,
                        },
                    };

                    let active_tab_result =
                        active_query_tab.as_ref().and_then(|t| t.result.clone());
                    let console_grid = self.build_data_grid(
                        &app_handle,
                        active_tab_result,
                        self.selected_table.clone(),
                        is_read_only,
                    );

                    let console =
                        QueryConsole::new(&active_tab_ref.editor, &active_tab_ref.split_state)
                            .tabs(tab_headers)
                            .active_tab_id(Some(active_tab_id))
                            .result(active_tab_ref.result.clone())
                            .error(active_tab_ref.error.clone())
                            .explain_plan(active_tab_ref.explain_plan.clone())
                            .explain_error(active_tab_ref.explain_error.clone())
                            .executing(active_tab_ref.is_executing)
                            .explaining(active_tab_ref.is_explaining)
                            .bottom_tab(active_tab_ref.bottom_tab)
                            .explain_view(active_tab_ref.explain_view)
                            .connection_label(connection_label)
                            .results_view(console_grid)
                            .editor_settings(self.settings_manager.settings().editor.clone())
                            .language(self.settings_manager.settings().language)
                            .on_select_tab(on_select_tab)
                            .on_close_tab(on_close_tab)
                            .on_new_tab(on_new_tab)
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
                            let icon = crate::ui::components::connection_dialog::database_icon(
                                conn.db_type,
                            );
                            let chip = Button::new(ElementId::Name(
                                format!("quick_conn_{}", conn.id).into(),
                            ))
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
                    let grid_data = self.current_data_result();
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
                    let on_start_edit = {
                        let handle = app_handle.clone();
                        move |window: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.start_schema_editing(window, cx);
                            });
                        }
                    };
                    let on_cancel_edit = {
                        let handle = app_handle.clone();
                        move |_: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.cancel_schema_editing(cx);
                            });
                        }
                    };
                    let on_add_col = {
                        let handle = app_handle.clone();
                        move |window: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.add_schema_edit_column(window, cx);
                            });
                        }
                    };
                    let on_toggle_del = {
                        let handle = app_handle.clone();
                        move |idx: usize, _: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.toggle_schema_column_delete(idx, cx);
                            });
                        }
                    };
                    let on_toggle_null = {
                        let handle = app_handle.clone();
                        move |idx: usize, _: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.toggle_schema_column_nullable(idx, cx);
                            });
                        }
                    };
                    let on_toggle_pk = {
                        let handle = app_handle.clone();
                        move |idx: usize, _: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.toggle_schema_column_pk(idx, cx);
                            });
                        }
                    };
                    let on_toggle_auto = {
                        let handle = app_handle.clone();
                        move |idx: usize, _: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.toggle_schema_column_auto_increment(idx, cx);
                            });
                        }
                    };
                    let on_review_alter = {
                        let handle = app_handle.clone();
                        move |_: &mut Window, cx: &mut App| {
                            handle.update(cx, |this, cx| {
                                this.review_schema_alterations(cx);
                            });
                        }
                    };

                    let db_family = self
                        .active_connection
                        .as_ref()
                        .map(|c| c.config.db_type.family())
                        .unwrap_or(DatabaseFamily::Sqlite);
                    let pending_alterations = self.count_pending_schema_alterations(cx);

                    SchemaViewer::new(
                        self.selected_table.clone(),
                        self.schema_columns.clone(),
                        self.schema_indexes.clone(),
                        self.schema_ddl.clone(),
                    )
                    .family(db_family)
                    .is_editing(self.schema_is_editing)
                    .edit_columns(self.schema_edit_columns.clone())
                    .pending_alterations_count(pending_alterations)
                    .on_quick_query(on_quick)
                    .on_create_table(on_create)
                    .on_start_edit(on_start_edit)
                    .on_cancel_edit(on_cancel_edit)
                    .on_add_column(on_add_col)
                    .on_toggle_delete_column(on_toggle_del)
                    .on_toggle_nullable(on_toggle_null)
                    .on_toggle_pk(on_toggle_pk)
                    .on_toggle_auto_increment(on_toggle_auto)
                    .on_review_alterations(on_review_alter)
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
                                this.status_message =
                                    Some(format!("Copied SQL to clipboard ({len} bytes)"));
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
            }
        };

        // Footer status bar
        let query_row_count = match self.active_tab {
            WorkspaceTab::QueryConsole => self
                .active_query_tab()
                .and_then(|t| t.result.as_ref())
                .map(|r| r.rows.len()),
            WorkspaceTab::DataGrid => self.current_data_result().as_ref().map(|r| r.rows.len()),
            WorkspaceTab::Schema => Some(self.schema_columns.len()),
            WorkspaceTab::History => Some(self.history_manager.items().len()),
        };
        let query_duration = self
            .active_query_tab()
            .and_then(|t| t.execution_time_ms)
            .or_else(|| {
                self.active_query_tab()
                    .and_then(|t| t.result.as_ref())
                    .and_then(|r| r.execution_time_ms)
            });

        let mut status_bar = AppStatusBar::new()
            .connected(is_connected)
            .read_only(is_read_only)
            .status(active_status)
            .language(self.settings_manager.settings().language)
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
                            this.status_message =
                                Some("Copied SQL script to clipboard".to_string());
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

                // Bidirectional synchronization between visual form and DDL code editor
                let editor_val = self.create_table_ddl_editor.read(cx).value().to_string();
                if !self.create_table_is_syncing {
                    if editor_val != self.create_table_last_synced_sql {
                        // User typed or pasted in DDL editor -> attempt auto sync to visual columns
                        self.sync_create_table_from_ddl(window, cx, false);
                    } else if preview_sql != self.create_table_last_synced_sql {
                        // User changed visual form and editor has not been modified -> update DDL editor
                        self.create_table_is_syncing = true;
                        self.create_table_ddl_editor.update(cx, |ed, cx| {
                            ed.set_value(&preview_sql, window, cx);
                        });
                        self.create_table_last_synced_sql = preview_sql.clone();
                        self.create_table_sync_status = None;
                        self.create_table_is_syncing = false;
                    }
                }

                let current_editor_sql = self.create_table_ddl_editor.read(cx).value().to_string();
                let effective_sql = if !current_editor_sql.trim().is_empty() {
                    current_editor_sql
                } else {
                    preview_sql
                };

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
                let on_sync_ddl = {
                    let handle = app_handle.clone();
                    move |window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.sync_create_table_from_ddl(window, cx, true);
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
                            this.status_message =
                                Some("Copied Create Table DDL to clipboard".to_string());
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
                let on_add_idx = {
                    let handle = app_handle.clone();
                    move |window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.add_create_table_index(window, cx);
                        });
                    }
                };
                let on_remove_idx = {
                    let handle = app_handle.clone();
                    move |idx: usize, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.remove_create_table_index(idx, cx);
                        });
                    }
                };
                let on_toggle_idx_type = {
                    let handle = app_handle.clone();
                    move |idx: usize, _: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.toggle_create_table_index_type(idx, cx);
                        });
                    }
                };
                let on_toggle_idx_col = {
                    let handle = app_handle.clone();
                    move |idx: usize, col_name: String, window: &mut Window, cx: &mut App| {
                        handle.update(cx, |this, cx| {
                            this.toggle_create_table_index_column(idx, col_name, window, cx);
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
                        effective_sql,
                    )
                    .ddl_editor(self.create_table_ddl_editor.clone())
                    .sync_status(self.create_table_sync_status.clone())
                    .on_sync_from_ddl(on_sync_ddl)
                    .indexes(self.create_table_indexes.clone())
                    .on_add_index(on_add_idx)
                    .on_remove_index(on_remove_idx)
                    .on_toggle_index_type(on_toggle_idx_type)
                    .on_toggle_index_column(on_toggle_idx_col)
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

        // Table destructive action confirm dialog overlay (Drop/Truncate)
        let table_confirm_overlay = if let Some(ref action_state) = self.table_confirm_modal {
            let on_confirm = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.execute_table_confirm_action(cx);
                    });
                }
            };
            let on_cancel = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.table_confirm_modal = None;
                        cx.notify();
                    });
                }
            };

            Some(
                ConfirmDialog::new(
                    action_state.kind,
                    action_state.table.clone(),
                    action_state.database_family,
                    action_state.sql_preview.clone(),
                )
                .language(self.settings_manager.settings().language)
                .executing(action_state.is_executing)
                .error(action_state.error.clone())
                .on_confirm(on_confirm)
                .on_cancel(on_cancel),
            )
        } else {
            None
        };

        // Database Connection Error Dialog modal overlay if active
        let conn_error_overlay = self.connection_error_modal.clone().map(|info| {
            let on_close = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.close_connection_error_modal(cx);
                    });
                }
            };
            let on_retry = {
                let handle = app_handle.clone();
                move |conn_id: &str, _: &mut Window, cx: &mut App| {
                    let cid = conn_id.to_string();
                    handle.update(cx, |this, cx| {
                        this.retry_connection_error(&cid, cx);
                    });
                }
            };
            let on_edit = {
                let handle = app_handle.clone();
                move |conn_id: &str, window: &mut Window, cx: &mut App| {
                    let cid = conn_id.to_string();
                    handle.update(cx, |this, cx| {
                        this.close_connection_error_modal(cx);
                        this.open_edit_connection_dialog(&cid, window, cx);
                    });
                }
            };
            let on_copy = {
                let handle = app_handle.clone();
                move |err: &str, _: &mut Window, cx: &mut App| {
                    let len = err.len();
                    cx.write_to_clipboard(ClipboardItem::new_string(err.to_string()));
                    handle.update(cx, |this, cx| {
                        this.connection_error_copied = true;
                        this.status_message =
                            Some(format!("Copied error details to clipboard ({len} bytes)"));
                        cx.notify();
                    });
                }
            };

            crate::ui::components::ConnectionErrorDialog::new(info)
                .language(self.settings_manager.settings().language)
                .copied(self.connection_error_copied)
                .retrying(self.connection_error_is_retrying)
                .on_close(on_close)
                .on_retry(on_retry)
                .on_edit(on_edit)
                .on_copy(on_copy)
        });

        // Data Import Wizard Modal overlay if open
        let import_overlay = if self.import_modal_open {
            let on_close = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.close_import_modal(cx);
                    });
                }
            };
            let on_browse = {
                let handle = app_handle.clone();
                move |window: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.browse_import_file(window, cx);
                    });
                }
            };
            let on_inspect = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.inspect_import_file(cx);
                    });
                }
            };
            let on_step = {
                let handle = app_handle.clone();
                move |step, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.import_step = step;
                        cx.notify();
                    });
                }
            };
            let on_sel_table = {
                let handle = app_handle.clone();
                move |tbl: String, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.select_import_table(tbl, cx);
                    });
                }
            };
            let on_sel_delim = {
                let handle = app_handle.clone();
                move |delim, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.update_import_delimiter(delim, cx);
                    });
                }
            };
            let on_sel_enc = {
                let handle = app_handle.clone();
                move |enc, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.update_import_encoding(enc, cx);
                    });
                }
            };
            let on_tog_hdr = {
                let handle = app_handle.clone();
                move |has_hdr, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.toggle_import_headers(has_hdr, cx);
                    });
                }
            };
            let on_upd_map = {
                let handle = app_handle.clone();
                move |src_idx, target_col, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.update_import_mapping(src_idx, target_col, cx);
                    });
                }
            };
            let on_sel_batch = {
                let handle = app_handle.clone();
                move |size, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.import_batch_size = size;
                        cx.notify();
                    });
                }
            };
            let on_sel_policy = {
                let handle = app_handle.clone();
                move |policy, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.import_error_policy = policy;
                        cx.notify();
                    });
                }
            };
            let on_start = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.start_import_execution(cx);
                    });
                }
            };
            let on_view_tbl = {
                let handle = app_handle.clone();
                move |tbl: String, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.view_imported_table(tbl, cx);
                    });
                }
            };

            Some(
                ImportModal::new(
                    &self.import_file_path_input,
                    self.import_step,
                    self.import_format,
                    self.import_encoding,
                    self.import_delimiter,
                    self.import_has_headers,
                    self.import_target_table.clone(),
                    self.import_available_tables.clone(),
                    self.import_table_columns.clone(),
                    self.import_mappings.clone(),
                    self.import_batch_size,
                    self.import_error_policy,
                    self.import_is_executing,
                )
                .file_path(self.import_file_path.clone())
                .csv_preview(self.import_csv_preview.clone())
                .sql_preview(self.import_sql_preview.clone())
                .progress(self.import_progress.clone())
                .result(self.import_result.clone())
                .error(self.import_error.clone())
                .language(self.settings_manager.settings().language)
                .on_close(on_close)
                .on_browse_file(on_browse)
                .on_inspect_file(on_inspect)
                .on_change_step(on_step)
                .on_select_table(on_sel_table)
                .on_select_delimiter(on_sel_delim)
                .on_select_encoding(on_sel_enc)
                .on_toggle_headers(on_tog_hdr)
                .on_update_mapping(on_upd_map)
                .on_select_batch_size(on_sel_batch)
                .on_select_error_policy(on_sel_policy)
                .on_start_import(on_start)
                .on_view_table(on_view_tbl),
            )
        } else {
            None
        };

        // Data & Schema Export Wizard Modal overlay if open
        let export_overlay = if self.export_modal_open {
            let on_close = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.close_export_modal(cx);
                    });
                }
            };
            let on_step = {
                let handle = app_handle.clone();
                move |step, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.export_step = step;
                        if step == ExportWizardStep::Step2Preview {
                            this.load_export_preview(cx);
                        }
                        cx.notify();
                    });
                }
            };
            let on_sel_table = {
                let handle = app_handle.clone();
                move |tbl: String, window: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.select_export_table(tbl, window, cx);
                    });
                }
            };
            let on_sel_fmt = {
                let handle = app_handle.clone();
                move |fmt, window: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.update_export_format(fmt, window, cx);
                    });
                }
            };
            let on_sel_scope = {
                let handle = app_handle.clone();
                move |scope, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.export_config.scope = scope;
                        this.load_export_preview(cx);
                        cx.notify();
                    });
                }
            };
            let on_sel_dest = {
                let handle = app_handle.clone();
                move |dest, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.export_destination = dest;
                        cx.notify();
                    });
                }
            };
            let on_browse = {
                let handle = app_handle.clone();
                move |window: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.browse_export_file(window, cx);
                    });
                }
            };
            let on_drop = {
                let handle = app_handle.clone();
                move |drop_tbl, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.export_config.sql_options.drop_table_if_exists = drop_tbl;
                        this.load_export_preview(cx);
                        cx.notify();
                    });
                }
            };
            let on_tx = {
                let handle = app_handle.clone();
                move |wrap_tx, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.export_config.sql_options.wrap_in_transaction = wrap_tx;
                        this.load_export_preview(cx);
                        cx.notify();
                    });
                }
            };
            let on_batch = {
                let handle = app_handle.clone();
                move |batch_sz, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.export_config.sql_options.batch_size = batch_sz;
                        this.load_export_preview(cx);
                        cx.notify();
                    });
                }
            };
            let on_hdr = {
                let handle = app_handle.clone();
                move |hdr, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.export_config.csv_options.include_headers = hdr;
                        this.load_export_preview(cx);
                        cx.notify();
                    });
                }
            };
            let on_delim = {
                let handle = app_handle.clone();
                move |delim, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.export_config.csv_options.delimiter = delim;
                        this.load_export_preview(cx);
                        cx.notify();
                    });
                }
            };
            let on_null = {
                let handle = app_handle.clone();
                move |null_rep, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.export_config.csv_options.null_representation = null_rep;
                        this.load_export_preview(cx);
                        cx.notify();
                    });
                }
            };
            let on_pretty = {
                let handle = app_handle.clone();
                move |pretty, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.export_config.json_options.pretty = pretty;
                        this.load_export_preview(cx);
                        cx.notify();
                    });
                }
            };
            let on_refresh = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.load_export_preview(cx);
                    });
                }
            };
            let on_start = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.start_export_execution(cx);
                    });
                }
            };
            let on_reveal = {
                let handle = app_handle.clone();
                move |path: String, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, _cx| {
                        this.reveal_exported_file(&path);
                    });
                }
            };

            Some(
                ExportModal::new(
                    self.export_step,
                    self.export_target_table.clone(),
                    self.export_available_tables.clone(),
                    self.export_config.family,
                    self.export_config.clone(),
                    self.export_destination,
                    &self.export_file_path_input,
                    &self.export_where_input,
                    &self.export_limit_input,
                    self.export_is_executing,
                    self.settings_manager.settings().language,
                )
                .preview(self.export_preview_content.clone(), self.export_is_loading_preview)
                .progress(self.export_progress_rows)
                .success(self.export_success_info.clone())
                .error(self.export_error.clone())
                .on_close(on_close)
                .on_select_step(on_step)
                .on_select_table(on_sel_table)
                .on_select_format(on_sel_fmt)
                .on_select_scope(on_sel_scope)
                .on_select_destination(on_sel_dest)
                .on_browse_file(on_browse)
                .on_toggle_drop_table(on_drop)
                .on_toggle_transaction(on_tx)
                .on_select_batch_size(on_batch)
                .on_toggle_headers(on_hdr)
                .on_select_delimiter(on_delim)
                .on_select_null_rep(on_null)
                .on_toggle_pretty_json(on_pretty)
                .on_refresh_preview(on_refresh)
                .on_start_export(on_start)
                .on_reveal_file(on_reveal),
            )
        } else {
            None
        };

        let mock_overlay = if self.mock_modal_open {
            let on_close = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.close_mock_modal(cx);
                    });
                }
            };
            let on_step = {
                let handle = app_handle.clone();
                move |step, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.mock_step = step;
                        if step == MockWizardStep::Step2Preview {
                            this.refresh_mock_preview(cx);
                        }
                        cx.notify();
                    });
                }
            };
            let on_sel_table = {
                let handle = app_handle.clone();
                move |tbl: String, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.select_mock_table(tbl, cx);
                    });
                }
            };
            let on_sel_count = {
                let handle = app_handle.clone();
                move |cnt: usize, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.mock_row_count = cnt;
                        cx.notify();
                    });
                }
            };
            let on_sel_batch = {
                let handle = app_handle.clone();
                move |bs: usize, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.mock_batch_size = bs;
                        cx.notify();
                    });
                }
            };
            let on_col_gen = {
                let handle = app_handle.clone();
                move |idx: usize, gen_type: MockGeneratorType, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        if let Some(col) = this.mock_columns.get_mut(idx) {
                            col.generator = gen_type;
                            cx.notify();
                        }
                    });
                }
            };
            let on_refresh = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.refresh_mock_preview(cx);
                    });
                }
            };
            let on_start = {
                let handle = app_handle.clone();
                move |_: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.start_mock_execution(cx);
                    });
                }
            };
            let on_view_grid = {
                let handle = app_handle.clone();
                move |tbl: String, _: &mut Window, cx: &mut App| {
                    handle.update(cx, |this, cx| {
                        this.close_mock_modal(cx);
                        this.active_nav = ActivityNav::Databases;
                        this.active_tab = WorkspaceTab::DataGrid;
                        if let Some(t) = this.active_tables.iter().find(|t| t.name == tbl).cloned() {
                            this.select_table(t, cx);
                        }
                        cx.notify();
                    });
                }
            };

            Some(
                MockDataModal::new(
                    self.mock_step,
                    self.mock_target_table.clone(),
                    self.mock_available_tables.clone(),
                    self.mock_columns.clone(),
                    self.mock_row_count,
                    self.mock_batch_size,
                )
                .preview(
                    self.mock_preview_headers.clone(),
                    self.mock_preview_rows.clone(),
                    self.mock_is_loading_preview,
                )
                .progress(self.mock_progress)
                .result(self.mock_result.clone())
                .error(self.mock_error.clone())
                .executing(self.mock_is_executing)
                .language(self.settings_manager.settings().language)
                .on_close(on_close)
                .on_select_step(on_step)
                .on_select_table(on_sel_table)
                .on_select_row_count(on_sel_count)
                .on_select_batch_size(on_sel_batch)
                .on_change_column_generator(on_col_gen)
                .on_refresh_preview(on_refresh)
                .on_start_seeding(on_start)
                .on_view_in_grid(on_view_grid),
            )
        } else {
            None
        };

        // Left rail Activity Bar
        let activity_bar =
            ActivityBar::new(self.active_nav, self.settings_manager.settings().language)
                .on_select_nav({
                    let handle = app_handle.clone();
                    move |nav, _, cx| {
                        handle.update(cx, |this, cx| {
                            match nav {
                                ActivityNav::Databases => {
                                    this.active_nav = ActivityNav::Databases;
                                    if this.active_tab == WorkspaceTab::QueryConsole
                                        && this.selected_table.is_some()
                                    {
                                        this.active_tab = WorkspaceTab::DataGrid;
                                    }
                                }
                                ActivityNav::Console => {
                                    this.active_nav = ActivityNav::Console;
                                    this.active_tab = WorkspaceTab::QueryConsole;
                                }
                                ActivityNav::Settings => {
                                    if this.active_nav == ActivityNav::Settings {
                                        this.active_nav =
                                            if this.active_tab == WorkspaceTab::QueryConsole {
                                                ActivityNav::Console
                                            } else {
                                                ActivityNav::Databases
                                            };
                                    } else {
                                        this.active_nav = ActivityNav::Settings;
                                    }
                                }
                            }
                            cx.notify();
                        });
                    }
                });

        // Settings View component
        let settings_view = SettingsView::new(
            self.settings_manager.settings().clone(),
            self.active_settings_tab,
        )
        .checking_update(self.is_checking_update)
        .update_status_msg(self.update_status_msg.clone())
        .update_result(self.update_check_result.clone())
        .on_open_release_url({
            let handle = app_handle.clone();
            move |url, _, cx| {
                handle.update(cx, |_, cx| {
                    cx.open_url(&url);
                });
            }
        })
        .on_select_tab({
            let handle = app_handle.clone();
            move |tab, _, cx| {
                handle.update(cx, |this, cx| {
                    this.active_settings_tab = tab;
                    cx.notify();
                });
            }
        })
        .on_change_settings({
            let handle = app_handle.clone();
            move |mutator, window, cx| {
                handle.update(cx, |this, cx| {
                    let _ = this.settings_manager.update(mutator);
                    let _ = this.settings_manager.save();
                    let hist_limit = this.settings_manager.settings().query.history_limit;
                    this.history_manager.set_max_entries(hist_limit);
                    let theme = this.settings_manager.settings().appearance.theme;
                    let theme_mode = match theme {
                        ThemePreference::Light => ThemeMode::Light,
                        _ => ThemeMode::Dark,
                    };
                    crate::ui::theme::set_active_theme_mode(theme_mode == ThemeMode::Light);
                    Theme::change(theme_mode, Some(window), cx);
                    this.sync_editor_settings(window, cx);
                    this.status_message = Some("Settings saved".to_string());
                    cx.notify();
                });
            }
        })
        .on_reset_defaults({
            let handle = app_handle.clone();
            move |window, cx| {
                handle.update(cx, |this, cx| {
                    let _ = this.settings_manager.reset_defaults();
                    let hist_limit = this.settings_manager.settings().query.history_limit;
                    this.history_manager.set_max_entries(hist_limit);
                    let theme = this.settings_manager.settings().appearance.theme;
                    let theme_mode = match theme {
                        ThemePreference::Light => ThemeMode::Light,
                        _ => ThemeMode::Dark,
                    };
                    crate::ui::theme::set_active_theme_mode(theme_mode == ThemeMode::Light);
                    Theme::change(theme_mode, Some(window), cx);
                    this.sync_editor_settings(window, cx);
                    this.status_message = Some("Settings reset to defaults".to_string());
                    cx.notify();
                });
            }
        })
        .on_check_updates({
            let handle = app_handle.clone();
            move |_, cx| {
                handle.update(cx, |this, cx| {
                    this.trigger_check_for_updates(cx);
                });
            }
        });

        // Root layout
        v_flex()
            .id("crabstudio_root")
            .key_context("CrabStudio")
            .on_action(cx.listener(|this, _: &RunQuery, window, cx| {
                this.run_query(Some(window), cx);
            }))
            .on_action(cx.listener(|this, _: &FormatSql, window, cx| {
                this.format_editor_sql(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ExplainQuery, _, cx| {
                this.run_explain(cx);
            }))
            .on_action(cx.listener(|this, _: &SaveGridChanges, _, cx| {
                if (this.active_tab == WorkspaceTab::DataGrid
                    || this.active_tab == WorkspaceTab::QueryConsole)
                    && this.grid_changeset.is_dirty()
                {
                    this.open_sql_review_modal(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &AddNewRow, window, cx| {
                if this.active_tab == WorkspaceTab::DataGrid
                    || this.active_tab == WorkspaceTab::QueryConsole
                {
                    this.add_new_grid_row(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &DuplicateGridRow, window, cx| {
                if this.active_tab == WorkspaceTab::DataGrid
                    || this.active_tab == WorkspaceTab::QueryConsole
                {
                    if let Some(coord) = this.grid_selected_cell {
                        this.duplicate_grid_row(coord, window, cx);
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &DeleteGridRow, _, cx| {
                if this.active_tab == WorkspaceTab::DataGrid
                    || this.active_tab == WorkspaceTab::QueryConsole
                {
                    if let Some(coord) = this.grid_selected_cell {
                        if coord.is_inserted {
                            this.discard_inserted_row(coord.row_idx, cx);
                        } else {
                            this.toggle_delete_grid_row(coord.row_idx, cx);
                        }
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &FocusGridFilter, window, cx| {
                if this.active_tab == WorkspaceTab::DataGrid
                    || this.active_tab == WorkspaceTab::QueryConsole
                {
                    this.grid_filter_input.update(cx, |inp, cx| {
                        inp.focus(window, cx);
                    });
                }
            }))
            .on_action(cx.listener(|this, _: &CloseDialog, _, cx| {
                if this.mock_modal_open {
                    this.close_mock_modal(cx);
                } else if this.export_modal_open {
                    this.close_export_modal(cx);
                } else if this.import_modal_open {
                    this.close_import_modal(cx);
                } else if this.connection_error_modal.is_some() {
                    this.close_connection_error_modal(cx);
                } else if this.table_confirm_modal.is_some() {
                    this.table_confirm_modal = None;
                    cx.notify();
                } else if this.create_table_modal_open {
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
            .on_action(cx.listener(|this, _: &CloseWindow, window, cx| {
                if this.mock_modal_open {
                    this.close_mock_modal(cx);
                } else if this.export_modal_open {
                    this.close_export_modal(cx);
                } else if this.import_modal_open {
                    this.close_import_modal(cx);
                } else if this.connection_error_modal.is_some() {
                    this.close_connection_error_modal(cx);
                } else if this.table_confirm_modal.is_some() {
                    this.table_confirm_modal = None;
                    cx.notify();
                } else if this.create_table_modal_open {
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
                } else if this.active_nav == ActivityNav::Settings {
                    this.active_nav = ActivityNav::Databases;
                    cx.notify();
                } else if this.active_tab == WorkspaceTab::QueryConsole && this.query_tabs.len() > 1
                {
                    this.close_active_query_tab(window, cx);
                } else {
                    window.remove_window();
                    #[cfg(not(target_os = "macos"))]
                    cx.quit();
                }
            }))
            .on_action(cx.listener(|_this, _: &Quit, _, cx| {
                cx.quit();
            }))
            .on_action(cx.listener(|this, _: &OpenImportModal, _, cx| {
                this.open_import_modal(None, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| {
                this.active_nav = ActivityNav::Settings;
                this.active_settings_tab = SettingsTab::Appearance;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NewConnection, window, cx| {
                this.open_connection_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewQueryTab, window, cx| {
                this.create_query_tab(None, None, window, cx);
            }))
            .on_action(cx.listener(|this, _: &RefreshTables, _, cx| {
                this.refresh_schema(cx);
            }))
            .on_action(cx.listener(|this, _: &SelectConsoleTab, _, cx| {
                this.active_nav = ActivityNav::Databases;
                this.active_tab = WorkspaceTab::QueryConsole;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectGridTab, _, cx| {
                this.active_nav = ActivityNav::Databases;
                this.active_tab = WorkspaceTab::DataGrid;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectSchemaTab, _, cx| {
                this.active_nav = ActivityNav::Databases;
                this.active_tab = WorkspaceTab::Schema;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectHistoryTab, _, cx| {
                this.active_nav = ActivityNav::Databases;
                this.active_tab = WorkspaceTab::History;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleActivityBar, _, cx| {
                let curr = this
                    .settings_manager
                    .settings()
                    .appearance
                    .show_activity_bar;
                let _ = this.settings_manager.update(|s| {
                    s.appearance.show_activity_bar = !curr;
                });
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleStatusBar, _, cx| {
                let curr = this.settings_manager.settings().appearance.show_status_bar;
                let _ = this.settings_manager.update(|s| {
                    s.appearance.show_status_bar = !curr;
                });
                cx.notify();
            }))
            .on_action(cx.listener(|_this, _: &MinimizeWindow, window, _cx| {
                window.minimize_window();
            }))
            .on_action(cx.listener(|_this, _: &ZoomWindow, window, _cx| {
                window.zoom_window();
            }))
            .on_action(cx.listener(|_this, _: &ToggleFullscreen, window, _cx| {
                window.toggle_fullscreen();
            }))
            .on_action(cx.listener(|this, _: &AboutZqlcrab, _, cx| {
                this.active_nav = ActivityNav::Settings;
                this.active_settings_tab = SettingsTab::About;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &CheckForUpdates, _, cx| {
                this.active_nav = ActivityNav::Settings;
                this.active_settings_tab = SettingsTab::Appearance;
                this.trigger_check_for_updates(cx);
            }))
            .on_action(cx.listener(|_this, _: &OpenDocs, _, cx| {
                cx.open_url("https://github.com/hyzwhu/zqlcrab#readme");
            }))
            .on_action(cx.listener(|_this, _: &OpenGithub, _, cx| {
                cx.open_url("https://github.com/hyzwhu/zqlcrab");
            }))
            .on_action(cx.listener(|_this, _: &ReportIssue, _, cx| {
                cx.open_url("https://github.com/hyzwhu/zqlcrab/issues");
            }))
            .size_full()
            .bg(ThemeColors::BG_APP)
            .child(title_bar)
            .child({
                let show_act_bar = self
                    .settings_manager
                    .settings()
                    .appearance
                    .show_activity_bar;
                h_flex()
                    .items_stretch()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .when(show_act_bar, |this| this.child(activity_bar))
                    .child(sidebar)
                    .child(v_flex().flex_1().h_full().min_w_0().min_h_0().child(
                        if self.active_nav == ActivityNav::Settings {
                            settings_view.into_any_element()
                        } else {
                            v_flex()
                                .size_full()
                                .child(tabs_bar)
                                .child(
                                    v_flex()
                                        .size_full()
                                        .flex_1()
                                        .min_h_0()
                                        .w_full()
                                        .child(main_content),
                                )
                                .into_any_element()
                        },
                    ))
            })
            .when(
                self.settings_manager.settings().appearance.show_status_bar,
                |this| this.child(status_bar),
            )
            .children(dialog_overlay)
            .children(sql_review_overlay)
            .children(create_table_overlay)
            .children(table_confirm_overlay)
            .children(conn_error_overlay)
            .children(import_overlay)
            .children(export_overlay)
            .children(mock_overlay)
    }
}
