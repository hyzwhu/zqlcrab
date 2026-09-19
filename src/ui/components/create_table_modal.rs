//! Create Table modal dialog for visual schema design and DDL execution.

use crate::db::sql_gen::TableIndexType;
use crate::db::types::DatabaseFamily;
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    Disableable as _, Icon, Sizable as _,
};
use gpui_kit::gpui::{
    div, px, rgba, Anchor, App, ElementId, Entity, FontWeight, IntoElement, ParentElement as _,
    RenderOnce, Styled, Window, prelude::*,
};
use std::rc::Rc;

pub type ActionCallback = Rc<dyn Fn(&mut Window, &mut App) + 'static>;
pub type IndexActionCallback = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;
pub type IndexStringCallback = Rc<dyn Fn(usize, String, &mut Window, &mut App) + 'static>;
pub type StringCallback = Rc<dyn Fn(String, &mut Window, &mut App) + 'static>;

/// State representation of an individual column row in the Create Table designer.
#[derive(Clone)]
pub struct CreateTableColumnState {
    pub name: Entity<InputState>,
    pub data_type: Entity<InputState>,
    pub is_primary_key: bool,
    pub is_nullable: bool,
    pub is_auto_increment: bool,
    pub default_val: Entity<InputState>,
    pub comment: Entity<InputState>,
}

/// State representation of an individual index row in the Create Table designer.
#[derive(Clone)]
pub struct CreateTableIndexState {
    pub name: Entity<InputState>,
    pub index_type: TableIndexType,
    pub columns: Entity<InputState>,
}

#[derive(IntoElement)]
pub struct CreateTableModal {
    database_family: DatabaseFamily,
    database_name: String,
    table_name_input: Entity<InputState>,
    schema_input: Entity<InputState>,
    comment_input: Entity<InputState>,
    columns: Vec<CreateTableColumnState>,
    indexes: Vec<CreateTableIndexState>,
    preview_sql: String,
    validation_error: Option<String>,
    error_message: Option<String>,
    is_executing: bool,
    copied: bool,

    on_add_column: Option<ActionCallback>,
    on_remove_column: Option<IndexActionCallback>,
    on_toggle_pk: Option<IndexActionCallback>,
    on_toggle_nullable: Option<IndexActionCallback>,
    on_toggle_auto_inc: Option<IndexActionCallback>,
    on_quick_type: Option<IndexStringCallback>,
    on_copy_sql: Option<StringCallback>,
    on_open_in_console: Option<StringCallback>,
    on_execute: Option<ActionCallback>,
    on_cancel: Option<ActionCallback>,
    on_add_index: Option<ActionCallback>,
    on_remove_index: Option<IndexActionCallback>,
    on_toggle_index_type: Option<IndexActionCallback>,
    on_toggle_index_column: Option<IndexStringCallback>,
}

impl CreateTableModal {
    pub fn new(
        database_family: DatabaseFamily,
        database_name: String,
        table_name_input: &Entity<InputState>,
        schema_input: &Entity<InputState>,
        comment_input: &Entity<InputState>,
        columns: Vec<CreateTableColumnState>,
        preview_sql: String,
    ) -> Self {
        Self {
            database_family,
            database_name,
            table_name_input: table_name_input.clone(),
            schema_input: schema_input.clone(),
            comment_input: comment_input.clone(),
            columns,
            indexes: Vec::new(),
            preview_sql,
            validation_error: None,
            error_message: None,
            is_executing: false,
            copied: false,
            on_add_column: None,
            on_remove_column: None,
            on_toggle_pk: None,
            on_toggle_nullable: None,
            on_toggle_auto_inc: None,
            on_quick_type: None,
            on_copy_sql: None,
            on_open_in_console: None,
            on_execute: None,
            on_cancel: None,
            on_add_index: None,
            on_remove_index: None,
            on_toggle_index_type: None,
            on_toggle_index_column: None,
        }
    }

    pub fn indexes(mut self, indexes: Vec<CreateTableIndexState>) -> Self {
        self.indexes = indexes;
        self
    }

    pub fn on_add_index<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_add_index = Some(Rc::new(handler));
        self
    }

    pub fn on_remove_index<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_remove_index = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_index_type<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_index_type = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_index_column<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, String, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_index_column = Some(Rc::new(handler));
        self
    }

    pub fn validation_error(mut self, err: Option<String>) -> Self {
        self.validation_error = err;
        self
    }

    pub fn error(mut self, err: Option<String>) -> Self {
        self.error_message = err;
        self
    }

    pub fn executing(mut self, executing: bool) -> Self {
        self.is_executing = executing;
        self
    }

    pub fn copied(mut self, copied: bool) -> Self {
        self.copied = copied;
        self
    }

    pub fn on_add_column<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_add_column = Some(Rc::new(handler));
        self
    }

    pub fn on_remove_column<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_remove_column = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_pk<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_pk = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_nullable<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_nullable = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_auto_inc<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_auto_inc = Some(Rc::new(handler));
        self
    }

    pub fn on_quick_type<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, String, &mut Window, &mut App) + 'static,
    {
        self.on_quick_type = Some(Rc::new(handler));
        self
    }

    pub fn on_copy_sql<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_copy_sql = Some(Rc::new(handler));
        self
    }

    pub fn on_open_in_console<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_open_in_console = Some(Rc::new(handler));
        self
    }

    pub fn on_execute<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_execute = Some(Rc::new(handler));
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

impl RenderOnce for CreateTableModal {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let cancel_handler = self.on_cancel.clone();
        let execute_handler = self.on_execute.clone();
        let copy_handler = self.on_copy_sql.clone();
        let console_handler = self.on_open_in_console.clone();
        let add_col_handler = self.on_add_column.clone();
        let toggle_pk_handler = self.on_toggle_pk.clone();
        let toggle_nn_handler = self.on_toggle_nullable.clone();
        let toggle_ai_handler = self.on_toggle_auto_inc.clone();
        let quick_type_handler = self.on_quick_type.clone();
        let remove_col_handler = self.on_remove_column.clone();
        let toggle_idx_col_handler = self.on_toggle_index_column.clone();

        let available_columns: Vec<(String, String)> = self
            .columns
            .iter()
            .map(|c| {
                let name = c.name.read(cx).value().trim().to_string();
                let dt = c.data_type.read(cx).value().trim().to_string();
                (name, dt)
            })
            .filter(|(name, _)| !name.is_empty())
            .collect();

        let family_str = match self.database_family {
            DatabaseFamily::Sqlite => "SQLite",
            DatabaseFamily::Postgres => "PostgreSQL",
            DatabaseFamily::MySql => "MySQL",
        };

        // Header close button
        let mut close_btn = Button::new("close_create_table_modal")
            .ghost()
            .xsmall()
            .icon(IconName::X);
        if let Some(ref on_cancel) = cancel_handler {
            let on_cancel = on_cancel.clone();
            close_btn = close_btn.on_click(move |_, window, cx| {
                on_cancel(window, cx);
            });
        }

        // Footer cancel button
        let mut cancel_btn = Button::new("cancel_create_table_btn")
            .ghost()
            .small()
            .label("Cancel");
        if let Some(ref on_cancel) = cancel_handler {
            let on_cancel = on_cancel.clone();
            cancel_btn = cancel_btn.on_click(move |_, window, cx| {
                on_cancel(window, cx);
            });
        }

        // Footer execute create table button
        let mut exec_btn = Button::new("confirm_create_table_btn")
            .primary()
            .small();
        if self.is_executing {
            exec_btn = exec_btn.icon(IconName::Loader).label("Creating Table...");
        } else {
            exec_btn = exec_btn.icon(IconName::Check).label("Create Table");
        }
        if !self.is_executing {
            if let Some(ref on_exec) = execute_handler {
                let on_exec = on_exec.clone();
                exec_btn = exec_btn.on_click(move |_, window, cx| {
                    on_exec(window, cx);
                });
            }
        }

        // Open in console button
        let sql_for_console = self.preview_sql.clone();
        let mut open_console_btn = Button::new("open_create_table_console_btn")
            .ghost()
            .small()
            .icon(IconName::Terminal)
            .label("Open in Console");
        if let Some(ref on_console) = console_handler {
            let on_console = on_console.clone();
            open_console_btn = open_console_btn.on_click(move |_, window, cx| {
                on_console(sql_for_console.clone(), window, cx);
            });
        }

        // Copy SQL button
        let sql_to_copy = self.preview_sql.clone();
        let mut copy_btn = Button::new("copy_create_table_sql_btn")
            .outline()
            .xsmall()
            .icon(if self.copied {
                IconName::Check
            } else {
                IconName::Copy
            })
            .label(if self.copied { "Copied!" } else { "Copy SQL" });
        if let Some(ref on_copy) = copy_handler {
            let on_copy = on_copy.clone();
            copy_btn = copy_btn.on_click(move |_, window, cx| {
                on_copy(sql_to_copy.clone(), window, cx);
            });
        }

        // Add Column button
        let mut add_col_btn = Button::new("add_column_action_btn")
            .primary()
            .xsmall()
            .icon(IconName::Plus)
            .label("Add Column");
        if let Some(ref on_add) = add_col_handler {
            let on_add = on_add.clone();
            add_col_btn = add_col_btn.on_click(move |_, window, cx| {
                on_add(window, cx);
            });
        }

        // Recommended data type presets for this database family
        let type_presets: &'static [&'static str] = match self.database_family {
            DatabaseFamily::Sqlite => &["INTEGER", "TEXT", "REAL", "BOOLEAN", "DATETIME", "BLOB"],
            DatabaseFamily::Postgres => &[
                "SERIAL",
                "INTEGER",
                "BIGINT",
                "VARCHAR(255)",
                "TEXT",
                "BOOLEAN",
                "TIMESTAMP",
                "JSONB",
            ],
            DatabaseFamily::MySql => &[
                "INT",
                "BIGINT",
                "VARCHAR(255)",
                "TEXT",
                "TINYINT(1)",
                "DATETIME",
                "DECIMAL(10,2)",
                "JSON",
            ],
        };

        // Render column rows
        let col_count = self.columns.len();
        let mut columns_list = v_flex().w_full().gap_1p5();

        for (idx, col) in self.columns.iter().enumerate() {
            let row_idx = idx;

            // PK button
            let mut pk_btn = Button::new(("pk_btn", idx)).xsmall();
            if col.is_primary_key {
                pk_btn = pk_btn
                    .primary()
                    .label("PK")
                    .tooltip("Primary Key: ON (Click to toggle)");
            } else {
                pk_btn = pk_btn
                    .ghost()
                    .label("PK")
                    .tooltip("Primary Key: OFF (Click to toggle)");
            }
            if let Some(ref on_pk) = toggle_pk_handler {
                let on_pk = on_pk.clone();
                pk_btn = pk_btn.on_click(move |_, window, cx| {
                    on_pk(row_idx, window, cx);
                });
            }

            // Not Null button
            let mut nn_btn = Button::new(("nn_btn", idx)).xsmall();
            if !col.is_nullable {
                nn_btn = nn_btn
                    .primary()
                    .label("NN")
                    .tooltip("Not Null: Required (Click to toggle)");
            } else {
                nn_btn = nn_btn
                    .ghost()
                    .label("NULL")
                    .tooltip("Nullable: Allowed (Click to toggle)");
            }
            if let Some(ref on_nn) = toggle_nn_handler {
                let on_nn = on_nn.clone();
                nn_btn = nn_btn.on_click(move |_, window, cx| {
                    on_nn(row_idx, window, cx);
                });
            }

            // Auto Increment button
            let mut ai_btn = Button::new(("ai_btn", idx)).xsmall();
            if col.is_auto_increment {
                ai_btn = ai_btn
                    .primary()
                    .label("AI")
                    .tooltip("Auto Increment / Serial: ON (Click to toggle)");
            } else {
                ai_btn = ai_btn
                    .ghost()
                    .label("AI")
                    .tooltip("Auto Increment / Serial: OFF (Click to toggle)");
            }
            if let Some(ref on_ai) = toggle_ai_handler {
                let on_ai = on_ai.clone();
                ai_btn = ai_btn.on_click(move |_, window, cx| {
                    on_ai(row_idx, window, cx);
                });
            }

            // Delete column button
            let mut del_btn = Button::new(("del_col", idx))
                .ghost()
                .xsmall()
                .icon(IconName::Trash)
                .tooltip("Remove column");
            if col_count > 1 {
                if let Some(ref on_del) = remove_col_handler {
                    let on_del = on_del.clone();
                    del_btn = del_btn.on_click(move |_, window, cx| {
                        on_del(row_idx, window, cx);
                    });
                }
            } else {
                del_btn = del_btn.disabled(true);
            }

            let row = h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(ThemeColors::BORDER)
                .bg(ThemeColors::BG_APP)
                // Row index indicator
                .child(
                    div()
                        .w(px(20.0))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(ThemeColors::TEXT_FAINT)
                        .child(format!("{}", idx + 1)),
                )
                // Column Name
                .child(
                    div()
                        .w(px(150.0))
                        .child(Input::new(&col.name).small().w_full()),
                )
                // Data Type
                .child(
                    div()
                        .w(px(130.0))
                        .child(Input::new(&col.data_type).small().w_full()),
                )
                // PK Toggle
                .child(div().w(px(42.0)).items_center().justify_center().child(pk_btn))
                // Not Null Toggle
                .child(div().w(px(48.0)).items_center().justify_center().child(nn_btn))
                // Auto Increment Toggle
                .child(div().w(px(42.0)).items_center().justify_center().child(ai_btn))
                // Default Value Input
                .child(
                    div()
                        .w(px(130.0))
                        .child(Input::new(&col.default_val).small().w_full()),
                )
                // Column Comment Input
                .child(
                    div()
                        .flex_1()
                        .min_w(px(130.0))
                        .child(Input::new(&col.comment).small().w_full()),
                )
                // Actions
                .child(div().w(px(32.0)).items_center().justify_center().child(del_btn));

            columns_list = columns_list.child(row);
        }

        // Render index rows
        let index_count = self.indexes.len();
        let mut add_idx_btn = Button::new("add_idx_btn")
            .ghost()
            .xsmall()
            .icon(IconName::Plus)
            .label("Add Index");
        if let Some(ref on_add_idx) = self.on_add_index {
            let on_add_idx = on_add_idx.clone();
            add_idx_btn = add_idx_btn.on_click(move |_, window, cx| {
                on_add_idx(window, cx);
            });
        }

        let remove_idx_handler = self.on_remove_index.clone();
        let toggle_type_handler = self.on_toggle_index_type.clone();

        let mut indexes_list = v_flex().w_full().gap_1p5();
        if self.indexes.is_empty() {
            indexes_list = indexes_list.child(
                div()
                    .w_full()
                    .py_2()
                    .px_3()
                    .rounded_md()
                    .bg(ThemeColors::BG_APP)
                    .border_1()
                    .border_color(ThemeColors::BORDER.opacity(0.5))
                    .text_xs()
                    .text_color(ThemeColors::TEXT_FAINT)
                    .child("No indexes defined. Click '+ Add Index' to define indexes on columns."),
            );
        } else {
            for (idx, index_item) in self.indexes.iter().enumerate() {
                let idx_row = idx;
                let mut type_btn = Button::new(("idx_type_btn", idx)).xsmall();
                match index_item.index_type {
                    TableIndexType::Unique => {
                        type_btn = type_btn
                            .primary()
                            .label("UNIQUE")
                            .tooltip("Index Type: UNIQUE (Click to toggle Normal)");
                    }
                    TableIndexType::Normal => {
                        type_btn = type_btn
                            .ghost()
                            .label("INDEX")
                            .tooltip("Index Type: INDEX (Click to toggle Unique)");
                    }
                }
                if let Some(ref on_tog) = toggle_type_handler {
                    let on_tog = on_tog.clone();
                    type_btn = type_btn.on_click(move |_, window, cx| {
                        on_tog(idx_row, window, cx);
                    });
                }

                let mut del_btn = Button::new(("del_idx_btn", idx))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Trash)
                    .tooltip("Remove Index");
                if let Some(ref on_rem) = remove_idx_handler {
                    let on_rem = on_rem.clone();
                    del_btn = del_btn.on_click(move |_, window, cx| {
                        on_rem(idx_row, window, cx);
                    });
                }

                let current_cols_raw = index_item.columns.read(cx).value().to_string();
                let current_col_list: Vec<String> = current_cols_raw
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                let mut chips_row = h_flex().items_center().gap_1();
                for (col_i, (col_name, _col_type)) in available_columns.iter().take(4).enumerate() {
                    let is_checked = current_col_list.iter().any(|c| c.eq_ignore_ascii_case(col_name));
                    let chip_id = ElementId::Name(format!("idx_quick_col_{idx}_{col_i}").into());
                    let mut chip = Button::new(chip_id).xsmall();
                    if is_checked {
                        chip = chip
                            .primary()
                            .icon(IconName::Check)
                            .label(col_name.as_str())
                            .tooltip(format!("Remove '{col_name}' from index columns"));
                    } else {
                        chip = chip
                            .ghost()
                            .icon(IconName::Plus)
                            .label(col_name.as_str())
                            .tooltip(format!("Add '{col_name}' to index columns"));
                    }
                    if let Some(ref on_tog) = toggle_idx_col_handler {
                        let on_tog = on_tog.clone();
                        let c_name = col_name.clone();
                        chip = chip.on_click(move |_, window, cx| {
                            on_tog(idx_row, c_name.clone(), window, cx);
                        });
                    }
                    chips_row = chips_row.child(chip);
                }

                let avail_cols_for_menu = available_columns.clone();
                let cur_cols_for_menu = current_col_list.clone();
                let menu_toggle_handler = toggle_idx_col_handler.clone();

                let col_menu_id = ElementId::Name(format!("idx_col_menu_{idx}").into());
                let col_dropdown = Button::new(col_menu_id)
                    .outline()
                    .xsmall()
                    .dropdown_caret(true)
                    .icon(IconName::List)
                    .label("Select")
                    .tooltip("Select columns to include in this index")
                    .dropdown_menu_with_anchor(Anchor::BottomRight, move |mut menu, _window, _cx| {
                        menu = menu.max_h(px(260.0)).min_w(px(180.0)).scrollable(true);
                        if avail_cols_for_menu.is_empty() {
                            menu = menu.label("No columns defined");
                        } else {
                            menu = menu.label("Table Columns");
                            for (c_name, c_type) in &avail_cols_for_menu {
                                let is_checked = cur_cols_for_menu.iter().any(|c| c.eq_ignore_ascii_case(c_name));
                                let on_tog = menu_toggle_handler.clone();
                                let c_name_val = c_name.clone();
                                let label_text = if c_type.is_empty() {
                                    c_name.clone()
                                } else {
                                    format!("{c_name}  ({c_type})")
                                };
                                menu = menu.item(
                                    PopupMenuItem::new(label_text)
                                        .checked(is_checked)
                                        .on_click(move |_, window, cx| {
                                            if let Some(ref handler) = on_tog {
                                                handler(idx_row, c_name_val.clone(), window, cx);
                                            }
                                        }),
                                );
                            }
                        }
                        menu
                    });

                let row = h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_APP)
                    .child(
                        div()
                            .w(px(20.0))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(format!("{}", idx + 1)),
                    )
                    .child(
                        div()
                            .w(px(180.0))
                            .child(Input::new(&index_item.name).small().w_full()),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .items_center()
                            .justify_center()
                            .child(type_btn),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w(px(260.0))
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(120.0))
                                    .child(
                                        Input::new(&index_item.columns)
                                            .small()
                                            .w_full(),
                                    ),
                            )
                            .child(chips_row)
                            .child(col_dropdown),
                    )
                    .child(
                        div()
                            .w(px(32.0))
                            .items_center()
                            .justify_center()
                            .child(del_btn),
                    );

                indexes_list = indexes_list.child(row);
            }
        }

        // Live SQL Preview lines
        let preview_lines: Vec<&str> = self.preview_sql.lines().collect();
        let code_lines = v_flex()
            .w_full()
            .gap_0p5()
            .children(preview_lines.into_iter().enumerate().map(|(idx, line)| {
                let color = if line.starts_with("CREATE TABLE") || line.starts_with("COMMENT ON") {
                    ThemeColors::PRIMARY_LIGHT
                } else if line.contains("PRIMARY KEY") || line.contains("SERIAL") || line.contains("AUTO_INCREMENT") {
                    ThemeColors::SUCCESS
                } else if line.contains("NOT NULL") || line.contains("DEFAULT") {
                    ThemeColors::WARNING
                } else {
                    ThemeColors::TEXT_PRIMARY
                };

                h_flex()
                    .min_w_full()
                    .w_auto()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .w(px(24.0))
                            .flex_none()
                            .text_right()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child(format!("{}", idx + 1)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(color)
                            .whitespace_nowrap()
                            .child(line.to_string()),
                    )
            }));

        // Error banner if any
        let err_banner = self.error_message.as_ref().or(self.validation_error.as_ref()).map(|err| {
            h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .p_2p5()
                .rounded_md()
                .bg(rgba(0xEF444415))
                .border_1()
                .border_color(rgba(0xEF444440))
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .size(px(14.0))
                        .text_color(ThemeColors::ERROR),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ThemeColors::ERROR)
                        .child(err.clone()),
                )
        });

        // Modal card
        let modal = v_flex()
            .w(px(940.0))
            .max_w(px(1080.0))
            .max_h(px(760.0))
            .bg(ThemeColors::BG_SURFACE)
            .border_1()
            .border_color(ThemeColors::BORDER)
            .rounded_lg()
            .shadow_xl()
            .overflow_hidden()
            // Modal Header
            .child(
                h_flex()
                    .w_full()
                    .p_3()
                    .px_4()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_APP)
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .p_1p5()
                                    .rounded_md()
                                    .bg(ThemeColors::PRIMARY_BG)
                                    .child(
                                        Icon::new(IconName::Table)
                                            .size(px(18.0))
                                            .text_color(ThemeColors::PRIMARY_LIGHT),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .child("Create New Table"),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_full()
                                                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                                    .border_1()
                                                    .border_color(ThemeColors::BORDER)
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(ThemeColors::TEXT_MUTED)
                                                    .child(format!("{} · {}", self.database_name, family_str)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(ThemeColors::TEXT_MUTED)
                                            .child("Define table name, schema, columns, and constraints with live dialect DDL preview"),
                                    ),
                            ),
                    )
                    .child(close_btn),
            )
            // Modal Body
            .child(
                v_flex()
                    .id("create_table_body_scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .gap_3()
                    // Error banner
                    .children(err_banner)
                    // Section 1: Table & Schema basic properties
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_3()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .child("Table Name"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(ThemeColors::ERROR)
                                                    .child("*"),
                                            ),
                                    )
                                    .child(
                                        Input::new(&self.table_name_input)
                                            .id("new_tbl_name_input")
                                            .small()
                                            .w_full(),
                                    ),
                            )
                            .when(self.database_family != DatabaseFamily::Sqlite, |this| {
                                this.child(
                                    v_flex()
                                        .w(px(200.0))
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(ThemeColors::TEXT_PRIMARY)
                                                .child("Schema / Database"),
                                        )
                                        .child(
                                            Input::new(&self.schema_input)
                                                .id("new_tbl_schema_input")
                                                .small()
                                                .w_full(),
                                        ),
                                )
                            })
                            .child(
                                v_flex()
                                    .w(px(240.0))
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(ThemeColors::TEXT_PRIMARY)
                                            .child("Description / Comment"),
                                    )
                                    .child(
                                        Input::new(&self.comment_input)
                                            .id("new_tbl_comment_input")
                                            .small()
                                            .w_full(),
                                    ),
                            ),
                    )
                    // Section 2: Columns builder
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1p5()
                            // Columns header bar
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
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .child("Columns"),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_full()
                                                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                                    .border_1()
                                                    .border_color(ThemeColors::BORDER)
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(ThemeColors::TEXT_MUTED)
                                                    .child(format!("{col_count}")),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(ThemeColors::TEXT_FAINT)
                                                            .child("Presets:"),
                                                    )
                                                    .children(type_presets.iter().take(4).enumerate().map(|(p_idx, t)| {
                                                        let mut p_btn = Button::new(("preset_btn", p_idx))
                                                            .ghost()
                                                            .xsmall()
                                                            .label(*t);
                                                        if let Some(ref on_qt) = quick_type_handler {
                                                            let on_qt = on_qt.clone();
                                                            let t_str = t.to_string();
                                                            let target_idx = col_count.saturating_sub(1);
                                                            p_btn = p_btn.on_click(move |_, window, cx| {
                                                                on_qt(target_idx, t_str.clone(), window, cx);
                                                            });
                                                        }
                                                        p_btn
                                                    })),
                                            )
                                            .child(add_col_btn),
                                    ),
                            )
                            // Columns list headers
                            .child(
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .py_1()
                                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                    .border_1()
                                    .border_color(ThemeColors::BORDER)
                                    .rounded_t_md()
                                    .child(div().w(px(20.0)).text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("#"))
                                    .child(div().w(px(150.0)).text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("NAME"))
                                    .child(div().w(px(130.0)).text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("TYPE"))
                                    .child(div().w(px(42.0)).items_center().justify_center().child(div().text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("PK")))
                                    .child(div().w(px(48.0)).items_center().justify_center().child(div().text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("NULL")))
                                    .child(div().w(px(42.0)).items_center().justify_center().child(div().text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("AUTO")))
                                    .child(div().w(px(130.0)).text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("DEFAULT VALUE"))
                                    .child(div().flex_1().min_w(px(130.0)).text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("COMMENT"))
                                    .child(div().w(px(32.0)).items_center().justify_center().child(div().text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("DEL"))),
                            )
                            .child(columns_list),
                    )
                    // Section 3: Indexes builder
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1p5()
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .child("Indexes"),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_full()
                                                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                                    .border_1()
                                                    .border_color(ThemeColors::BORDER)
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(ThemeColors::TEXT_MUTED)
                                                    .child(format!("{index_count}")),
                                            ),
                                    )
                                    .child(add_idx_btn),
                            )
                            // Indexes list headers
                            .child(
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .py_1()
                                    .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                    .border_1()
                                    .border_color(ThemeColors::BORDER)
                                    .rounded_t_md()
                                    .child(div().w(px(20.0)).text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("#"))
                                    .child(div().w(px(180.0)).text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("INDEX NAME"))
                                    .child(div().w(px(80.0)).items_center().justify_center().child(div().text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("TYPE")))
                                    .child(div().flex_1().min_w(px(260.0)).text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("COLUMNS (select or type comma-separated)"))
                                    .child(div().w(px(32.0)).items_center().justify_center().child(div().text_xs().font_weight(FontWeight::BOLD).text_color(ThemeColors::TEXT_MUTED).child("DEL"))),
                            )
                            .child(indexes_list),
                    )
                    // Section 4: Live SQL Preview
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1()
                            .child(
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_1p5()
                                            .child(
                                                Icon::new(IconName::Code)
                                                    .size(px(13.0))
                                                    .text_color(ThemeColors::PRIMARY_LIGHT),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                                    .child("Generated SQL Preview"),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(ThemeColors::PRIMARY_BG)
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(ThemeColors::PRIMARY_LIGHT)
                                                    .child(family_str),
                                            ),
                                    )
                                    .child(copy_btn),
                            )
                            .child(
                                div()
                                    .id("create_table_sql_scroll")
                                    .w_full()
                                    .max_h(px(160.0))
                                    .overflow_x_scroll()
                                    .overflow_y_scroll()
                                    .p_3()
                                    .rounded_md()
                                    .bg(rgba(0x0B1120FF))
                                    .border_1()
                                    .border_color(ThemeColors::BORDER)
                                    .child(
                                        div()
                                            .min_w_full()
                                            .w_auto()
                                            .child(code_lines),
                                    ),
                            ),
                    ),
            )
            // Modal Footer
            .child(
                h_flex()
                    .w_full()
                    .p_3()
                    .px_4()
                    .justify_between()
                    .items_center()
                    .border_t_1()
                    .border_color(ThemeColors::BORDER)
                    .bg(ThemeColors::BG_SURFACE)
                    .child(
                        div()
                            .text_xs()
                            .text_color(ThemeColors::TEXT_FAINT)
                            .child("Press Esc to dismiss · Use 'Open in Console' to customize constraints"),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(cancel_btn)
                            .child(open_console_btn)
                            .child(exec_btn),
                    ),
            );

        // Backdrop overlay
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x000000B0))
            .justify_center()
            .items_center()
            .flex()
            .child(modal)
    }
}
