//! Schema structure viewer displaying column definitions, constraints, indexes, and DDL,
//! with interactive online ALTER TABLE schema evolution mode.

use crate::db::types::{ColumnInfo, DatabaseFamily, IndexInfo};
use crate::ui::theme::ThemeColors;
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow},
    tooltip::Tooltip,
};
use gpui_kit::gpui::{
    App, ElementId, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, prelude::*, px,
};
use std::rc::Rc;

pub type ColumnActionCallback = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

/// State representation of an individual column row in SchemaViewer's structure editor.
#[derive(Clone)]
pub struct SchemaEditColumnState {
    pub original_name: Option<String>,
    pub name: Entity<InputState>,
    pub data_type: Entity<InputState>,
    pub is_primary_key: bool,
    pub is_nullable: bool,
    pub is_auto_increment: bool,
    pub default_val: Entity<InputState>,
    pub comment: Entity<InputState>,
    pub is_deleted: bool,
}

#[derive(IntoElement)]
pub struct SchemaViewer {
    table_name: Option<String>,
    columns: Vec<ColumnInfo>,
    indexes: Vec<IndexInfo>,
    ddl: Option<String>,
    family: DatabaseFamily,
    // Online schema evolution edit mode states
    is_editing: bool,
    edit_columns: Vec<SchemaEditColumnState>,
    pending_alterations_count: usize,
    // Callbacks
    on_quick_query: Option<Rc<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    on_create_table: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_start_edit: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_cancel_edit: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_add_column: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_toggle_delete_column: Option<ColumnActionCallback>,
    on_toggle_nullable: Option<ColumnActionCallback>,
    on_toggle_pk: Option<ColumnActionCallback>,
    on_toggle_auto_increment: Option<ColumnActionCallback>,
    on_review_alterations: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl SchemaViewer {
    pub fn new(
        table_name: Option<String>,
        columns: Vec<ColumnInfo>,
        indexes: Vec<IndexInfo>,
        ddl: Option<String>,
    ) -> Self {
        Self {
            table_name,
            columns,
            indexes,
            ddl,
            family: DatabaseFamily::Sqlite,
            is_editing: false,
            edit_columns: Vec::new(),
            pending_alterations_count: 0,
            on_quick_query: None,
            on_create_table: None,
            on_start_edit: None,
            on_cancel_edit: None,
            on_add_column: None,
            on_toggle_delete_column: None,
            on_toggle_nullable: None,
            on_toggle_pk: None,
            on_toggle_auto_increment: None,
            on_review_alterations: None,
        }
    }

    pub fn family(mut self, family: DatabaseFamily) -> Self {
        self.family = family;
        self
    }

    pub fn is_editing(mut self, editing: bool) -> Self {
        self.is_editing = editing;
        self
    }

    pub fn edit_columns(mut self, cols: Vec<SchemaEditColumnState>) -> Self {
        self.edit_columns = cols;
        self
    }

    pub fn pending_alterations_count(mut self, count: usize) -> Self {
        self.pending_alterations_count = count;
        self
    }

    pub fn on_start_edit<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_start_edit = Some(Rc::new(handler));
        self
    }

    pub fn on_cancel_edit<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_cancel_edit = Some(Rc::new(handler));
        self
    }

    pub fn on_add_column<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_add_column = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_delete_column<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_delete_column = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_nullable<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_nullable = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_pk<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_pk = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_auto_increment<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &mut Window, &mut App) + 'static,
    {
        self.on_toggle_auto_increment = Some(Rc::new(handler));
        self
    }

    pub fn on_review_alterations<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_review_alterations = Some(Rc::new(handler));
        self
    }

    pub fn on_create_table<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_create_table = Some(Rc::new(handler));
        self
    }

    pub fn on_quick_query<F>(mut self, handler: F) -> Self
    where
        F: Fn(String, &mut Window, &mut App) + 'static,
    {
        self.on_quick_query = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SchemaViewer {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let on_create_empty = self.on_create_table.clone();
        let Some(table_name) = self.table_name else {
            let mut create_btn = Button::new("schema_empty_create_table_btn")
                .primary()
                .small()
                .icon(IconName::Plus)
                .label("Create New Table");
            if let Some(on_create) = on_create_empty {
                create_btn = create_btn.on_click(move |_, window, cx| {
                    on_create(window, cx);
                });
            }

            return v_flex()
                .id("schema_viewer_empty")
                .size_full()
                .justify_center()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(IconName::TableProperties)
                        .size(px(32.0))
                        .text_color(ThemeColors::TEXT_FAINT),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child("No Table Selected"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child("Select a table from the sidebar to inspect its columns, keys, and DDL, or design a new table."),
                )
                .child(
                    div()
                        .pt_2()
                        .child(create_btn),
                );
        };

        // Header
        let tbl_for_sel = table_name.clone();
        let tbl_for_cnt = table_name.clone();
        let tbl_for_exp = table_name.clone();
        let on_quick_sel = self.on_quick_query.clone();
        let on_quick_cnt = self.on_quick_query.clone();
        let on_quick_exp = self.on_quick_query.clone();
        let on_create_hdr = self.on_create_table.clone();
        let on_start_edit_hdr = self.on_start_edit.clone();
        let on_cancel_edit_hdr = self.on_cancel_edit.clone();
        let on_add_col_hdr = self.on_add_column.clone();
        let on_review_hdr = self.on_review_alterations.clone();

        let mut edit_struct_btn = Button::new("schema_start_edit_btn")
            .ghost()
            .xsmall()
            .icon(IconName::Pencil)
            .tooltip("Edit table columns, types, and constraints")
            .child("Edit Structure");
        if let Some(on_start) = on_start_edit_hdr {
            edit_struct_btn = edit_struct_btn.on_click(move |_, window, cx| {
                on_start(window, cx);
            });
        }

        let mut cancel_edit_btn = Button::new("schema_cancel_edit_btn")
            .ghost()
            .xsmall()
            .icon(IconName::X)
            .tooltip("Discard all unsaved column changes")
            .child("Discard");
        if let Some(on_cancel) = on_cancel_edit_hdr {
            cancel_edit_btn = cancel_edit_btn.on_click(move |_, window, cx| {
                on_cancel(window, cx);
            });
        }

        let mut add_col_btn = Button::new("schema_add_col_btn")
            .ghost()
            .xsmall()
            .icon(IconName::Plus)
            .tooltip("Add a new column to this table")
            .child("Add Column");
        if let Some(on_add) = on_add_col_hdr {
            add_col_btn = add_col_btn.on_click(move |_, window, cx| {
                on_add(window, cx);
            });
        }

        let review_label = if self.pending_alterations_count > 0 {
            format!("Review & Alter ({})", self.pending_alterations_count)
        } else {
            "Review & Alter".to_string()
        };

        let mut review_btn = Button::new("schema_review_alterations_btn")
            .primary()
            .xsmall()
            .icon(IconName::Check)
            .tooltip("Review generated ALTER TABLE DDL and execute atomically")
            .child(review_label);
        if let Some(on_review) = on_review_hdr {
            review_btn = review_btn.on_click(move |_, window, cx| {
                on_review(window, cx);
            });
        }

        let mut new_table_btn = Button::new("schema_new_table_btn")
            .ghost()
            .xsmall()
            .icon(IconName::Plus)
            .tooltip("Create New Table")
            .child("New Table");
        if let Some(on_create) = on_create_hdr {
            new_table_btn = new_table_btn.on_click(move |_, window, cx| {
                on_create(window, cx);
            });
        }

        let header_actions = if self.is_editing {
            h_flex()
                .items_center()
                .gap_1p5()
                .child(cancel_edit_btn)
                .child(add_col_btn)
                .child(review_btn)
        } else {
            h_flex()
                .items_center()
                .gap_1p5()
                .child(edit_struct_btn)
                .child(
                    Button::new("schema_sel_btn")
                        .primary()
                        .xsmall()
                        .icon(IconName::Play)
                        .tooltip("Query first 100 rows")
                        .child("Select 100")
                        .when_some(on_quick_sel, |btn, handler| {
                            btn.on_click(move |_, window, cx| {
                                handler(
                                    format!("SELECT * FROM \"{}\" LIMIT 100;", tbl_for_sel),
                                    window,
                                    cx,
                                );
                            })
                        }),
                )
                .child(
                    Button::new("schema_count_btn")
                        .ghost()
                        .xsmall()
                        .tooltip("Count total rows")
                        .child("Count (*)")
                        .when_some(on_quick_cnt, |btn, handler| {
                            btn.on_click(move |_, window, cx| {
                                handler(
                                    format!(
                                        "SELECT COUNT(*) AS total_count FROM \"{}\";",
                                        tbl_for_cnt
                                    ),
                                    window,
                                    cx,
                                );
                            })
                        }),
                )
                .child(
                    Button::new("schema_explain_btn")
                        .ghost()
                        .xsmall()
                        .tooltip("Explain query plan")
                        .child("Explain Plan")
                        .when_some(on_quick_exp, |btn, handler| {
                            btn.on_click(move |_, window, cx| {
                                handler(
                                    format!("EXPLAIN SELECT * FROM \"{}\" LIMIT 100;", tbl_for_exp),
                                    window,
                                    cx,
                                );
                            })
                        }),
                )
                .child(new_table_btn)
        };

        let header = h_flex()
            .items_center()
            .justify_between()
            .p_3()
            .border_b_1()
            .border_color(ThemeColors::BORDER)
            .bg(ThemeColors::BG_SURFACE)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::TableProperties)
                            .size(px(18.0))
                            .text_color(ThemeColors::PRIMARY_BORDER),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_PRIMARY)
                            .child(if self.is_editing {
                                format!("Edit Table Structure: {table_name}")
                            } else {
                                format!("Table Structure: {table_name}")
                            }),
                    )
                    .when(self.is_editing, |this| {
                        this.child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(ThemeColors::PRIMARY_BG)
                                .text_color(ThemeColors::PRIMARY_LIGHT)
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .child("EDITING"),
                        )
                    }),
            )
            .child(header_actions);

        // Columns section
        let columns_table = if self.is_editing {
            let edit_header_row = TableRow::new()
                .w_full()
                .child(
                    TableHead::new()
                        .w(px(70.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child("Status"),
                )
                .child(
                    TableHead::new()
                        .w(px(180.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child("Column Name"),
                )
                .child(
                    TableHead::new()
                        .w(px(160.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child("Data Type"),
                )
                .child(
                    TableHead::new()
                        .w(px(90.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child("Nullable"),
                )
                .child(
                    TableHead::new()
                        .w(px(110.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child("Key / Auto"),
                )
                .child(
                    TableHead::new()
                        .w(px(160.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child("Default Value"),
                )
                .child(
                    TableHead::new()
                        .flex_1()
                        .min_w(px(180.0))
                        .overflow_hidden()
                        .child("Comment / Description"),
                )
                .child(
                    TableHead::new()
                        .w(px(70.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child("Action"),
                );

            let mut edit_body = TableBody::new().w_full();

            for (col_idx, col_state) in self.edit_columns.iter().enumerate() {
                let is_del = col_state.is_deleted;
                let is_new = col_state.original_name.is_none();

                let status_badge = if is_del {
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(ThemeColors::ERROR)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child("DROP")
                } else if is_new {
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(ThemeColors::SUCCESS)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(ThemeColors::TEXT_PRIMARY)
                        .child("+ ADD")
                } else {
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(ThemeColors::BG_SURFACE_ACTIVE)
                        .text_xs()
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child("KEEP")
                };

                let on_toggle_null = self.on_toggle_nullable.clone();
                let on_toggle_pk = self.on_toggle_pk.clone();
                let on_toggle_auto = self.on_toggle_auto_increment.clone();
                let on_toggle_del = self.on_toggle_delete_column.clone();

                let null_btn = Button::new(ElementId::NamedInteger(
                    "toggle_null".into(),
                    col_idx as u64,
                ))
                .ghost()
                .xsmall()
                .child(if col_state.is_nullable {
                    "YES"
                } else {
                    "NOT NULL"
                })
                .text_color(if col_state.is_nullable {
                    ThemeColors::TEXT_MUTED
                } else {
                    ThemeColors::WARNING
                })
                .when_some(on_toggle_null, move |btn, handler| {
                    btn.on_click(move |_, window, cx| {
                        handler(col_idx, window, cx);
                    })
                });

                let pk_btn =
                    Button::new(ElementId::NamedInteger("toggle_pk".into(), col_idx as u64))
                        .ghost()
                        .xsmall()
                        .child("PK")
                        .text_color(if col_state.is_primary_key {
                            ThemeColors::PRIMARY_BORDER
                        } else {
                            ThemeColors::TEXT_FAINT
                        })
                        .when_some(on_toggle_pk, move |btn, handler| {
                            btn.on_click(move |_, window, cx| {
                                handler(col_idx, window, cx);
                            })
                        });

                let auto_btn = Button::new(ElementId::NamedInteger(
                    "toggle_auto".into(),
                    col_idx as u64,
                ))
                .ghost()
                .xsmall()
                .child("AUTO")
                .text_color(if col_state.is_auto_increment {
                    ThemeColors::SUCCESS
                } else {
                    ThemeColors::TEXT_FAINT
                })
                .when_some(on_toggle_auto, move |btn, handler| {
                    btn.on_click(move |_, window, cx| {
                        handler(col_idx, window, cx);
                    })
                });

                let action_btn = if is_del {
                    Button::new(ElementId::NamedInteger(
                        "restore_col".into(),
                        col_idx as u64,
                    ))
                    .ghost()
                    .xsmall()
                    .icon(IconName::RotateCcw)
                    .tooltip("Restore Column")
                    .when_some(on_toggle_del, move |btn, handler| {
                        btn.on_click(move |_, window, cx| {
                            handler(col_idx, window, cx);
                        })
                    })
                } else {
                    Button::new(ElementId::NamedInteger("drop_col".into(), col_idx as u64))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Trash)
                        .tooltip("Mark column for DROP")
                        .text_color(ThemeColors::ERROR)
                        .when_some(on_toggle_del, move |btn, handler| {
                            btn.on_click(move |_, window, cx| {
                                handler(col_idx, window, cx);
                            })
                        })
                };

                let row = TableRow::new()
                    .w_full()
                    .items_center()
                    .child(
                        TableCell::new()
                            .w(px(70.0))
                            .flex_shrink_0()
                            .child(status_badge),
                    )
                    .child(
                        TableCell::new()
                            .w(px(180.0))
                            .flex_shrink_0()
                            .child(Input::new(&col_state.name).xsmall().w_full()),
                    )
                    .child(
                        TableCell::new()
                            .w(px(160.0))
                            .flex_shrink_0()
                            .child(Input::new(&col_state.data_type).xsmall().w_full()),
                    )
                    .child(TableCell::new().w(px(90.0)).flex_shrink_0().child(null_btn))
                    .child(
                        TableCell::new()
                            .w(px(110.0))
                            .flex_shrink_0()
                            .child(h_flex().gap_1().child(pk_btn).child(auto_btn)),
                    )
                    .child(
                        TableCell::new()
                            .w(px(160.0))
                            .flex_shrink_0()
                            .child(Input::new(&col_state.default_val).xsmall().w_full()),
                    )
                    .child(
                        TableCell::new()
                            .flex_1()
                            .min_w(px(180.0))
                            .child(Input::new(&col_state.comment).xsmall().w_full()),
                    )
                    .child(
                        TableCell::new()
                            .w(px(70.0))
                            .flex_shrink_0()
                            .child(action_btn),
                    );

                edit_body = edit_body.child(row);
            }

            Table::new()
                .small()
                .w_full()
                .min_w(px(850.0))
                .child(
                    TableHeader::new()
                        .w_full()
                        .min_w(px(850.0))
                        .child(edit_header_row),
                )
                .child(edit_body)
        } else {
            let col_header_row = TableRow::new()
                .w_full()
                .child(
                    TableHead::new()
                        .w(px(200.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child("Name"),
                )
                .child(
                    TableHead::new()
                        .flex_1()
                        .min_w(px(240.0))
                        .overflow_hidden()
                        .child("Type"),
                )
                .child(
                    TableHead::new()
                        .w(px(90.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child("Nullable"),
                )
                .child(
                    TableHead::new()
                        .w(px(100.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child("Key"),
                )
                .child(
                    TableHead::new()
                        .w(px(180.0))
                        .min_w(px(120.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child("Default"),
                );

            let mut col_body = TableBody::new().w_full();
            for (col_idx, col) in self.columns.iter().enumerate() {
                let type_str = col.data_type.clone();
                let default_str = col.default_value.clone().unwrap_or_else(|| "-".to_string());
                let default_tooltip = default_str.clone();

                let row = TableRow::new()
                    .w_full()
                    .child(
                        TableCell::new()
                            .w(px(200.0))
                            .flex_shrink_0()
                            .overflow_hidden()
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child(col.name.clone()),
                            ),
                    )
                    .child(
                        TableCell::new()
                            .flex_1()
                            .min_w(px(240.0))
                            .overflow_hidden()
                            .child(
                                div()
                                    .id(ElementId::NamedInteger("col_type".into(), col_idx as u64))
                                    .w_full()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .font_family("JetBrains Mono")
                                    .text_color(ThemeColors::PRIMARY_BORDER)
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(type_str.clone()).build(window, cx)
                                    })
                                    .child(col.data_type.clone()),
                            ),
                    )
                    .child(
                        TableCell::new()
                            .w(px(90.0))
                            .flex_shrink_0()
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if col.is_nullable {
                                        ThemeColors::TEXT_MUTED
                                    } else {
                                        ThemeColors::WARNING
                                    })
                                    .child(if col.is_nullable { "YES" } else { "NO" }),
                            ),
                    )
                    .child(
                        TableCell::new()
                            .w(px(100.0))
                            .flex_shrink_0()
                            .overflow_hidden()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .when(col.is_primary_key, |this| {
                                        this.child(
                                            div()
                                                .px_1p5()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(ThemeColors::PRIMARY)
                                                .text_xs()
                                                .text_color(ThemeColors::TEXT_PRIMARY)
                                                .child("PK"),
                                        )
                                    })
                                    .when(col.is_auto_increment, |this| {
                                        this.child(
                                            div()
                                                .px_1p5()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(ThemeColors::BG_SURFACE_ACTIVE)
                                                .text_xs()
                                                .text_color(ThemeColors::TEXT_MUTED)
                                                .child("AUTO"),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        TableCell::new()
                            .w(px(180.0))
                            .min_w(px(120.0))
                            .flex_shrink_0()
                            .overflow_hidden()
                            .child(
                                div()
                                    .id(ElementId::NamedInteger("col_def".into(), col_idx as u64))
                                    .w_full()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_FAINT)
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(default_tooltip.clone()).build(window, cx)
                                    })
                                    .child(default_str),
                            ),
                    );
                col_body = col_body.child(row);
            }

            Table::new()
                .small()
                .w_full()
                .min_w(px(810.0))
                .child(
                    TableHeader::new()
                        .w_full()
                        .min_w(px(810.0))
                        .child(col_header_row),
                )
                .child(col_body)
        };

        // Indexes section
        let indexes_section = v_flex()
            .gap_2()
            .p_3()
            .when(!self.indexes.is_empty(), |this| {
                let mut index_list = v_flex().gap_1();
                for idx in &self.indexes {
                    let pill = h_flex()
                        .items_center()
                        .gap_2()
                        .p_2()
                        .rounded_md()
                        .bg(ThemeColors::BG_SURFACE)
                        .child(
                            Icon::new(IconName::Key)
                                .size(px(14.0))
                                .text_color(ThemeColors::TEXT_MUTED),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(ThemeColors::TEXT_PRIMARY)
                                .child(idx.name.clone()),
                        )
                        .when(idx.is_primary, |sub| {
                            sub.child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(ThemeColors::PRIMARY)
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child("Primary"),
                            )
                        })
                        .when(idx.is_unique, |sub| {
                            sub.child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(ThemeColors::SUCCESS)
                                    .text_xs()
                                    .text_color(ThemeColors::TEXT_PRIMARY)
                                    .child("Unique"),
                            )
                        });
                    index_list = index_list.child(pill);
                }

                this.child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ThemeColors::TEXT_MUTED)
                        .child("INDEXES"),
                )
                .child(index_list)
            });

        // DDL section
        let ddl_section = v_flex().gap_2().p_3().when_some(self.ddl, |this, sql| {
            this.child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ThemeColors::TEXT_MUTED)
                    .child("DDL STATEMENT"),
            )
            .child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(ThemeColors::BG_SURFACE)
                    .border_1()
                    .border_color(ThemeColors::BORDER)
                    .font_family("JetBrains Mono")
                    .text_xs()
                    .text_color(ThemeColors::TEXT_PRIMARY)
                    .child(sql),
            )
        });

        v_flex()
            .id("schema_viewer_scroll")
            .size_full()
            .bg(ThemeColors::BG_APP)
            .overflow_y_scroll()
            .child(header)
            .child(
                div()
                    .id("schema_columns_container")
                    .p_3()
                    .overflow_x_scroll()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ThemeColors::TEXT_MUTED)
                            .child("COLUMNS"),
                    )
                    .child(columns_table),
            )
            .when(!self.is_editing, |this| {
                this.child(indexes_section).child(ddl_section)
            })
    }
}
