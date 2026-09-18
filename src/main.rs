//! Desktop relational database client application entry point.

pub mod db;
pub mod ui;

use gpui_kit::AppContext;
use gpui_kit::component::{Root, TitleBar};
use gpui_kit::gpui::{Bounds, WindowBounds, WindowOptions, px, size};
use ui::CrabStudioApp;

fn main() {
    // Initialize background Tokio runtime and set ambient context on main thread
    let _tokio_guard = db::tokio_runtime().enter();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);

            let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
            let window_options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..TitleBar::window_options()
            };

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
