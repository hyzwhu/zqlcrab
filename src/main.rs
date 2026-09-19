//! Desktop relational database client application entry point.

pub mod db;
pub mod ui;

use gpui_kit::AppContext;
use gpui_kit::component::{Root, Theme, ThemeMode, TitleBar};
use gpui_kit::gpui::{Bounds, KeyBinding, WindowBounds, px, size};
use ui::app::{
    AddNewRow, CloseDialog, CrabStudioApp, DeleteGridRow, DuplicateGridRow, ExplainQuery,
    FormatSql, RunQuery, SaveGridChanges,
};

fn main() {
    // Initialize background Tokio runtime and set ambient context on main thread
    let _tokio_guard = db::tokio_runtime().enter();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);

            cx.bind_keys([
                KeyBinding::new("cmd-enter", RunQuery, Some("CrabStudio")),
                KeyBinding::new("ctrl-enter", RunQuery, Some("CrabStudio")),
                KeyBinding::new("alt-shift-f", FormatSql, Some("CrabStudio")),
                KeyBinding::new("cmd-shift-e", ExplainQuery, Some("CrabStudio")),
                KeyBinding::new("ctrl-shift-e", ExplainQuery, Some("CrabStudio")),
                KeyBinding::new("cmd-s", SaveGridChanges, Some("CrabStudio")),
                KeyBinding::new("ctrl-s", SaveGridChanges, Some("CrabStudio")),
                KeyBinding::new("cmd-n", AddNewRow, Some("CrabStudio")),
                KeyBinding::new("ctrl-n", AddNewRow, Some("CrabStudio")),
                KeyBinding::new("cmd-d", DuplicateGridRow, Some("CrabStudio")),
                KeyBinding::new("ctrl-d", DuplicateGridRow, Some("CrabStudio")),
                KeyBinding::new("cmd-backspace", DeleteGridRow, Some("CrabStudio")),
                KeyBinding::new("ctrl-backspace", DeleteGridRow, Some("CrabStudio")),
                KeyBinding::new("escape", CloseDialog, Some("CrabStudio")),
            ]);

            let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
            let mut window_options = TitleBar::window_options();
            window_options.window_bounds = Some(WindowBounds::Windowed(bounds));
            if let Some(titlebar) = window_options.titlebar.as_mut() {
                titlebar.title = Some("CrabStudio".into());
            }

            cx.spawn(async move |cx| {
                cx.open_window(window_options, |window, cx| {
                    let view = cx.new(|cx| CrabStudioApp::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("Failed to open CrabStudio window");
            })
            .detach();
        });
}
