//! Bilingual localization module providing English and 简体中文 translations.

use crate::settings::AppLanguage;

/// Translates a given message key into the requested language.
pub fn t(key: &'static str, lang: AppLanguage) -> &'static str {
    let target = lang.resolve_locale();
    match target {
        AppLanguage::ZhCn => translate_zh(key),
        _ => translate_en(key),
    }
}

fn translate_en(key: &'static str) -> &'static str {
    match key {
        // Activity bar & navigation
        "nav.databases" => "Databases",
        "nav.console" => "Query Console",
        "nav.history" => "History",
        "nav.settings" => "Settings",

        // Query console actions & tabs
        "console.run" => "Run",
        "console.running" => "Executing...",
        "console.format" => "Format",
        "console.explain" => "Explain",
        "console.explaining" => "Explaining...",
        "console.clear" => "Clear",
        "console.results" => "Results",
        "console.explain_tab" => "Execution Plan",
        "console.rows" => "Rows",
        "console.time" => "Time",

        // Status & workspace
        "status.connected" => "Connected",
        "status.disconnected" => "Disconnected",
        "status.ready" => "Ready",
        "workspace.databases" => "Databases",
        "workspace.console" => "SQL Console",
        "workspace.data" => "Data",
        "workspace.schema" => "Schema",
        "workspace.history" => "History",
        "workspace.settings" => "Settings",

        // Settings header & general
        "settings.title" => "Settings",
        "settings.subtitle" => "Configure your application preferences",
        "settings.reset" => "Reset to Defaults",
        "settings.saved" => "Saved",
        "settings.confirm_reset" => "Settings have been reset to defaults",

        // Settings tabs
        "tab.appearance" => "Appearance",
        "tab.editor" => "Editor",
        "tab.query" => "Query",
        "tab.language" => "Language",
        "tab.about" => "About",

        // Appearance tab
        "appearance.theme" => "Theme",
        "appearance.theme_desc" => "Customize the visual theme and appearance of zqlcrab",
        "appearance.system" => "System",
        "appearance.system_desc" => "Follow operating system theme",
        "appearance.dark" => "Dark",
        "appearance.dark_desc" => "Obsidian Slate dark theme",
        "appearance.light" => "Light",
        "appearance.light_desc" => "Clean high-contrast light theme",
        "appearance.show_activity_bar" => "Show Activity Bar",
        "appearance.show_activity_bar_desc" => {
            "Toggle visibility of the left-hand navigation activity bar"
        }
        "appearance.show_status_bar" => "Show Status Bar",
        "appearance.show_status_bar_desc" => {
            "Toggle visibility of the bottom session and metrics status bar"
        }
        "appearance.updates" => "Check for Updates",
        "appearance.updates_desc" => "Check for new releases, patches, and feature updates",
        "appearance.current_ver" => "Current Version",
        "appearance.status_latest" => "You are running the latest version",
        "appearance.checking" => "Checking for updates...",
        "appearance.checked_just_now" => "Checked just now: Up to date!",
        "appearance.check_now" => "Check Now",
        "appearance.updates_initial" => "Click \"Check Now\" to check for new releases on GitHub",
        "appearance.new_version" => "New version available",
        "appearance.view_release" => "View Release",
        "appearance.up_to_date" => "Up to date",

        // Editor tab
        "editor.font_family" => "Font Family",
        "editor.font_family_desc" => "Font family used in SQL query editor and result tables",
        "editor.font_size" => "Font Size",
        "editor.font_size_desc" => "Font size in pixels for the SQL editor",
        "editor.tab_size" => "Tab Size",
        "editor.tab_size_desc" => "Number of spaces per indentation level",
        "editor.line_numbers" => "Show Line Numbers",
        "editor.line_numbers_desc" => "Display vertical line numbers in the editor margin",
        "editor.word_wrap" => "Word Wrap",
        "editor.word_wrap_desc" => "Wrap long query lines instead of horizontal scrolling",
        "editor.format_on_run" => "Format on Run",
        "editor.format_on_run_desc" => "Automatically format and beautify SQL prior to execution",
        "editor.bracket_matching" => "Bracket Matching",
        "editor.bracket_matching_desc" => "Highlight matching parentheses and quotes automatically",

        // Query tab
        "query.default_limit" => "Default Row Limit",
        "query.default_limit_desc" => "Maximum rows fetched for query previews",
        "query.timeout" => "Query Timeout (seconds)",
        "query.timeout_desc" => "Abort long-running queries after the specified threshold",
        "query.safe_mode" => "Safe Mode (Confirm Unbounded Mutations)",
        "query.safe_mode_desc" => {
            "Require explicit confirmation for UPDATE/DELETE without WHERE and DROP TABLE"
        }
        "query.auto_explain" => "Auto Explain Slow Queries (>500ms)",
        "query.auto_explain_desc" => {
            "Automatically fetch query execution plan when latency exceeds 500ms"
        }
        "query.history_limit" => "History Retention Limit",
        "query.history_limit_desc" => "Maximum number of executed queries preserved in history",

        // Language tab
        "language.title" => "Application Display Language",
        "language.desc" => "Select your preferred application display language",
        "language.auto" => "Follow System",
        "language.auto_desc" => "Automatically match OS system language",
        "language.en" => "English (US)",
        "language.en_desc" => "Standard English interface",
        "language.zh" => "简体中文 (Simplified Chinese)",
        "language.zh_desc" => "完整的中文本地化界面",

        // Table context menu & actions
        "table_menu.open_data" => "Open Data",
        "table_menu.view_schema" => "View Structure",
        "table_menu.query_console" => "Query in Console",
        "table_menu.count_rows" => "Count Rows",
        "table_menu.show_ddl" => "Show CREATE TABLE (DDL)",
        "table_menu.import_data" => "Import Data...",
        "table_menu.export_data" => "Export Data / Dump...",
        "table_menu.generate_mock_data" => "Generate Mock Data...",
        "table_menu.copy_name" => "Copy Table Name",
        "table_menu.copy_select" => "Copy SELECT Statement",
        "table_menu.copy_insert" => "Copy INSERT Template",
        "table_menu.truncate" => "Truncate Table...",
        "table_menu.drop" => "Drop Table...",
        "table_menu.drop_view" => "Drop View...",

        // Table deletion & truncate dialog
        "dialog.drop_table_title" => "Drop Table",
        "dialog.drop_view_title" => "Drop View",
        "dialog.truncate_table_title" => "Truncate Table",
        "dialog.drop_table_desc" => {
            "This action is permanent and cannot be undone. The table and all its data, constraints, and indexes will be permanently removed."
        }
        "dialog.drop_view_desc" => {
            "This action is permanent and cannot be undone. The view definition will be permanently removed."
        }
        "dialog.truncate_table_desc" => {
            "This will delete all records stored in this table. The table schema will remain intact, but data cannot be recovered."
        }
        "dialog.confirm_drop" => "Drop Table",
        "dialog.confirm_drop_view" => "Drop View",
        "dialog.confirm_truncate" => "Truncate Table",
        "dialog.cancel" => "Cancel",
        "dialog.sql_statement" => "SQL Statement",

        // Database connection error dialog
        "conn_error.title" => "Connection Failed",
        "conn_error.subtitle" => "Unable to establish connection to the database server.",
        "conn_error.details" => "Error Details",
        "conn_error.retry" => "Retry",
        "conn_error.edit" => "Edit Connection",
        "conn_error.close" => "Close",
        "conn_error.copy" => "Copy Error",
        "conn_error.copied" => "Copied!",

        // Export modal wizard
        "export_modal.title" => "Export Table & Dump Wizard",
        "export_modal.step_config" => "Config",
        "export_modal.step_preview" => "Preview",
        "export_modal.step_execute" => "Export",
        "export_modal.target_table" => "Target Table",
        "export_modal.select_table" => "Select table...",
        "export_modal.format" => "Export Format",
        "export_modal.scope" => "Export Scope",
        "export_modal.scope_all" => "Structure & Data",
        "export_modal.scope_data" => "Data Only",
        "export_modal.scope_structure" => "Structure Only (DDL)",
        "export_modal.where_clause" => "WHERE Clause (Optional filter)",
        "export_modal.row_limit" => "Row Limit (Optional)",
        "export_modal.destination" => "Destination",
        "export_modal.dest_file" => "File",
        "export_modal.dest_clipboard" => "Clipboard",
        "export_modal.browse" => "Browse...",
        "export_modal.preview_btn" => "Preview...",
        "export_modal.export_now_btn" => "Export Now",
        "export_modal.sql_options" => "SQL Dump Options",
        "export_modal.drop_table_toggle" => "DROP TABLE IF EXISTS",
        "export_modal.tx_toggle" => "Wrap in Transaction",
        "export_modal.batch_size" => "Batch Size",
        "export_modal.csv_options" => "Delimited File Options",
        "export_modal.headers_toggle" => "Include Header Row",
        "export_modal.delimiter" => "Delimiter",
        "export_modal.null_as" => "NULL as",
        "export_modal.json_options" => "JSON Options",
        "export_modal.pretty_json" => "Pretty Print JSON",
        "export_modal.markdown_hint" => "Export as GitHub Flavored Markdown (GFM) table syntax.",
        "export_modal.back_btn" => "Back",
        "export_modal.refresh_preview" => "Refresh Preview",
        "export_modal.btn_export" => "Start Export",
        "export_modal.preview_title" => "Output Preview",
        "export_modal.generating_preview" => "Generating preview...",
        "export_modal.no_preview_yet" => "Click Refresh Preview to generate sample output.",
        "export_modal.exporting_title" => "Exporting Data...",
        "export_modal.processed_rows" => "Processed",
        "export_modal.rows_unit" => "rows",
        "export_modal.export_failed" => "Export Failed",
        "export_modal.success_title" => "Export Completed Successfully!",
        "export_modal.copied_notice" => "Content has been copied to your system clipboard.",
        "export_modal.reveal_file" => "Reveal in File Manager",
        "export_modal.done_btn" => "Done",

        // Mock data generator modal wizard
        "mock_modal.title" => "Visual Mock Data Generator & Seeder",
        "mock_modal.subtitle" => "Generate realistic test data and batch seed into database table.",
        "mock_modal.step_config" => "Rules & Config",
        "mock_modal.step_preview" => "Live Preview",
        "mock_modal.step_execute" => "Seeding Progress",
        "mock_modal.target_table" => "Target Table",
        "mock_modal.rows_to_generate" => "Rows to Seed",
        "mock_modal.batch_size" => "Batch Size",
        "mock_modal.column_rules" => "Column Generation Rules",
        "mock_modal.col_name" => "Column Name",
        "mock_modal.col_type" => "Data Type",
        "mock_modal.col_generator" => "Generator Strategy",
        "mock_modal.col_config" => "Configuration / Parameters",
        "mock_modal.col_skip" => "Skip",
        "mock_modal.preview_btn" => "Preview Sample...",
        "mock_modal.refresh_preview" => "Regenerate Preview",
        "mock_modal.start_seeding" => "Start Seeding Now",
        "mock_modal.seeding_in_progress" => "Generating and seeding mock data in atomic transaction...",
        "mock_modal.success_title" => "Mock Data Seeded Successfully!",
        "mock_modal.success_desc" => "Inserted realistic mock rows into table with zero conflicts.",
        "mock_modal.rows_inserted" => "Total Rows Seeded",
        "mock_modal.time_elapsed" => "Elapsed Time",
        "mock_modal.view_in_grid" => "View in Data Grid",
        "mock_modal.close" => "Close",
        "mock_modal.failed" => "Seeding Failed",
        "mock_modal.back" => "Back",

        // Other tabs
        "about.desc" => "A modern, high-performance database IDE built with Rust & GPUI",

        // Fallback
        _ => key,
    }
}

fn translate_zh(key: &'static str) -> &'static str {
    match key {
        // Activity bar & navigation
        "nav.databases" => "数据库",
        "nav.console" => "查询控制台",
        "nav.history" => "执行历史",
        "nav.settings" => "设置",

        // Query console actions & tabs
        "console.run" => "执行",
        "console.running" => "执行中...",
        "console.format" => "格式化",
        "console.explain" => "执行计划",
        "console.explaining" => "分析中...",
        "console.clear" => "清空",
        "console.results" => "查询结果",
        "console.explain_tab" => "执行计划",
        "console.rows" => "行数",
        "console.time" => "耗时",

        // Status & workspace
        "status.connected" => "已连接",
        "status.disconnected" => "未连接",
        "status.ready" => "就绪",
        "workspace.databases" => "数据库",
        "workspace.console" => "SQL 控制台",
        "workspace.data" => "数据浏览",
        "workspace.schema" => "表结构",
        "workspace.history" => "执行历史",
        "workspace.settings" => "偏好设置",

        // Settings header & general
        "settings.title" => "设置",
        "settings.subtitle" => "配置您的应用程序偏好",
        "settings.reset" => "恢复默认设置",
        "settings.saved" => "已保存",
        "settings.confirm_reset" => "已恢复所有偏好设置至默认值",

        // Settings tabs
        "tab.appearance" => "外观",
        "tab.editor" => "编辑器",
        "tab.query" => "查询",
        "tab.language" => "语言",
        "tab.about" => "关于",

        // Appearance tab
        "appearance.theme" => "界面主题",
        "appearance.theme_desc" => "定制 zqlcrab 的视觉外观与色彩主题",
        "appearance.system" => "跟随系统",
        "appearance.system_desc" => "自动匹配操作系统的明暗外观",
        "appearance.dark" => "深色模式",
        "appearance.dark_desc" => "黑曜石石板深色主题 (推荐)",
        "appearance.light" => "浅色模式",
        "appearance.light_desc" => "清爽高对比度浅色主题",
        "appearance.show_activity_bar" => "显示左侧活动栏",
        "appearance.show_activity_bar_desc" => "切换左侧图标导航栏的可见性",
        "appearance.show_status_bar" => "显示底部状态栏",
        "appearance.show_status_bar_desc" => "切换底部数据库连接与度量指标状态栏",
        "appearance.updates" => "检查更新",
        "appearance.updates_desc" => "检查应用新版本、补丁及功能更新",
        "appearance.current_ver" => "当前版本",
        "appearance.status_latest" => "您正在使用最新版本",
        "appearance.checking" => "正在检查更新...",
        "appearance.checked_just_now" => "刚刚完成检查：已是最新版本！",
        "appearance.check_now" => "立即检查",
        "appearance.updates_initial" => "点击“立即检查”以获取来自 GitHub 的最新版本发布",
        "appearance.new_version" => "发现新版本",
        "appearance.view_release" => "查看发布",
        "appearance.up_to_date" => "已是最新版本",

        // Editor tab
        "editor.font_family" => "字体族",
        "editor.font_family_desc" => "用于 SQL 编辑器及结果表格的代码字体",
        "editor.font_size" => "字号大小",
        "editor.font_size_desc" => "SQL 代码编辑器的字号大小 (像素)",
        "editor.tab_size" => "制表符缩进",
        "editor.tab_size_desc" => "每个缩进层级对应的空格数量",
        "editor.line_numbers" => "显示代码行号",
        "editor.line_numbers_desc" => "在编辑器左侧边栏显示代码行号",
        "editor.word_wrap" => "自动换行",
        "editor.word_wrap_desc" => "超长 SQL 语句自动软折行显示，无需横向滚动",
        "editor.format_on_run" => "执行前自动格式化",
        "editor.format_on_run_desc" => "在执行查询前自动对 SQL 语句进行排版美化",
        "editor.bracket_matching" => "括号高亮匹配",
        "editor.bracket_matching_desc" => "自动高亮匹配成对的括号与引号",

        // Query tab
        "query.default_limit" => "默认限制行数",
        "query.default_limit_desc" => "数据浏览和默认查询的最大返回行数限制",
        "query.timeout" => "查询超时时间 (秒)",
        "query.timeout_desc" => "超出指定时间后自动中止长耗时查询",
        "query.safe_mode" => "安全模式 (破坏性变更确认)",
        "query.safe_mode_desc" => "执行无 WHERE 的 UPDATE/DELETE 或 DROP TABLE 前强制二次确认",
        "query.auto_explain" => "慢查询自动分析 (>500ms)",
        "query.auto_explain_desc" => "当查询耗时超过 500ms 时自动获取并展示执行计划",
        "query.history_limit" => "历史记录最大条数",
        "query.history_limit_desc" => "本地持久化保存的历史 SQL 查询条数上限",

        // Language tab
        "language.title" => "界面显示语言",
        "language.desc" => "选择您偏好的应用程序界面语言",
        "language.auto" => "跟随系统",
        "language.auto_desc" => "根据操作系统语言自动适配",
        "language.en" => "English (US)",
        "language.en_desc" => "Standard English interface",
        "language.zh" => "简体中文 (Simplified Chinese)",
        "language.zh_desc" => "完整的中文本地化界面",

        // Table context menu & actions
        "table_menu.open_data" => "浏览数据",
        "table_menu.view_schema" => "查看表结构",
        "table_menu.query_console" => "在控制台查询",
        "table_menu.count_rows" => "统计总行数",
        "table_menu.show_ddl" => "查看建表语句 (DDL)",
        "table_menu.import_data" => "导入数据...",
        "table_menu.export_data" => "导出数据与转储...",
        "table_menu.generate_mock_data" => "生成模拟测试数据...",
        "table_menu.copy_name" => "复制表名",
        "table_menu.copy_select" => "复制 SELECT 语句",
        "table_menu.copy_insert" => "复制 INSERT 插入模板",
        "table_menu.truncate" => "清空数据表...",
        "table_menu.drop" => "删除数据表...",
        "table_menu.drop_view" => "删除视图...",

        // Table deletion & truncate dialog
        "dialog.drop_table_title" => "删除数据表",
        "dialog.drop_view_title" => "删除视图",
        "dialog.truncate_table_title" => "清空数据表",
        "dialog.drop_table_desc" => {
            "此操作不可逆！该表及其包含的所有数据、约束和索引将被永久删除，无法恢复。"
        }
        "dialog.drop_view_desc" => "此操作不可逆！该视图的定义将被永久删除。",
        "dialog.truncate_table_desc" => {
            "此操作将清空该表中的所有数据记录。表结构将保留，但数据将无法恢复。"
        }
        "dialog.confirm_drop" => "确认删除",
        "dialog.confirm_drop_view" => "确认删除视图",
        "dialog.confirm_truncate" => "确认清空",
        "dialog.cancel" => "取消",
        "dialog.sql_statement" => "即将执行的 SQL 语句",

        // Database connection error dialog
        "conn_error.title" => "数据库连接失败",
        "conn_error.subtitle" => "无法与指定的数据库服务器建立连接，请检查网络或配置。",
        "conn_error.details" => "错误详情",
        "conn_error.retry" => "重试连接",
        "conn_error.edit" => "编辑配置",
        "conn_error.close" => "关闭",
        "conn_error.copy" => "复制错误信息",
        "conn_error.copied" => "已复制!",

        // Export modal wizard
        "export_modal.title" => "数据导出与表转储向导",
        "export_modal.step_config" => "导出配置",
        "export_modal.step_preview" => "实时预览",
        "export_modal.step_execute" => "执行导出",
        "export_modal.target_table" => "目标表",
        "export_modal.select_table" => "选择表...",
        "export_modal.format" => "导出格式",
        "export_modal.scope" => "导出内容",
        "export_modal.scope_all" => "结构与数据 (完整转储)",
        "export_modal.scope_data" => "仅数据",
        "export_modal.scope_structure" => "仅结构 (DDL)",
        "export_modal.where_clause" => "WHERE 过滤条件 (可选)",
        "export_modal.row_limit" => "行数限制 (可选)",
        "export_modal.destination" => "导出目标",
        "export_modal.dest_file" => "保存至文件",
        "export_modal.dest_clipboard" => "复制到剪贴板",
        "export_modal.browse" => "浏览...",
        "export_modal.preview_btn" => "预览数据...",
        "export_modal.export_now_btn" => "直接导出",
        "export_modal.sql_options" => "SQL 转储选项",
        "export_modal.drop_table_toggle" => "包含 DROP TABLE IF EXISTS",
        "export_modal.tx_toggle" => "包裹在事务中 (BEGIN/COMMIT)",
        "export_modal.batch_size" => "批量插入大小",
        "export_modal.csv_options" => "分隔符文件选项",
        "export_modal.headers_toggle" => "包含表头行",
        "export_modal.delimiter" => "分隔符",
        "export_modal.null_as" => "NULL 呈现为",
        "export_modal.json_options" => "JSON 选项",
        "export_modal.pretty_json" => "格式化 JSON (Pretty Print)",
        "export_modal.markdown_hint" => "导出为 GitHub Flavored Markdown (GFM) 表格语法。",
        "export_modal.back_btn" => "上一步",
        "export_modal.refresh_preview" => "刷新预览",
        "export_modal.btn_export" => "开始导出",
        "export_modal.preview_title" => "生成结果预览",
        "export_modal.generating_preview" => "正在生成预览...",
        "export_modal.no_preview_yet" => "点击刷新预览以生成样本输出。",
        "export_modal.exporting_title" => "正在导出数据...",
        "export_modal.processed_rows" => "已处理",
        "export_modal.rows_unit" => "行",
        "export_modal.export_failed" => "导出失败",
        "export_modal.success_title" => "导出成功！",
        "export_modal.copied_notice" => "内容已成功复制到系统剪贴板。",
        "export_modal.reveal_file" => "在文件管理器中显示",
        "export_modal.done_btn" => "完成",

        // Mock data generator modal wizard
        "mock_modal.title" => "可视化模拟测试数据生成器",
        "mock_modal.subtitle" => "为数据表智能生成拟真业务测试数据并批量填充至数据库",
        "mock_modal.step_config" => "规则与配置",
        "mock_modal.step_preview" => "实时预览",
        "mock_modal.step_execute" => "填充进度",
        "mock_modal.target_table" => "目标表",
        "mock_modal.rows_to_generate" => "生成总行数",
        "mock_modal.batch_size" => "每批事务大小",
        "mock_modal.column_rules" => "字段生成规则配置",
        "mock_modal.col_name" => "字段名称",
        "mock_modal.col_type" => "数据类型",
        "mock_modal.col_generator" => "生成策略",
        "mock_modal.col_config" => "参数与选项",
        "mock_modal.col_skip" => "跳过",
        "mock_modal.preview_btn" => "预览模拟数据...",
        "mock_modal.refresh_preview" => "重新生成预览",
        "mock_modal.start_seeding" => "立即开始批量填充",
        "mock_modal.seeding_in_progress" => "正在原子事务中分批生成并填充模拟数据...",
        "mock_modal.success_title" => "模拟数据填充成功！",
        "mock_modal.success_desc" => "已将拟真测试数据安全插入目标数据表，无任何约束冲突。",
        "mock_modal.rows_inserted" => "成功写入总行数",
        "mock_modal.time_elapsed" => "总耗时",
        "mock_modal.view_in_grid" => "在数据网格中查看",
        "mock_modal.close" => "关闭",
        "mock_modal.failed" => "数据填充失败",
        "mock_modal.back" => "返回上一步",

        // Other tabs
        "about.desc" => "基于 Rust 与 GPUI 打造的高性能现代化桌面数据库客户端",

        // Fallback
        _ => translate_en(key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translations() {
        assert_eq!(t("settings.title", AppLanguage::En), "Settings");
        assert_eq!(t("settings.title", AppLanguage::ZhCn), "设置");
        assert_eq!(t("appearance.check_now", AppLanguage::ZhCn), "立即检查");
        assert_eq!(t("export_modal.title", AppLanguage::ZhCn), "数据导出与表转储向导");
        assert_eq!(t("export_modal.title", AppLanguage::En), "Export Table & Dump Wizard");
        assert_eq!(t("table_menu.export_data", AppLanguage::ZhCn), "导出数据与转储...");
        assert_eq!(t("mock_modal.title", AppLanguage::ZhCn), "可视化模拟测试数据生成器");
        assert_eq!(t("mock_modal.title", AppLanguage::En), "Visual Mock Data Generator & Seeder");
        assert_eq!(t("table_menu.generate_mock_data", AppLanguage::ZhCn), "生成模拟测试数据...");
    }
}
