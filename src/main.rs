//! Desktop relational database client application entry point.

pub mod db;
pub mod settings;
pub mod ui;

use gpui_kit::AppContext;
use gpui_kit::component::{Root, Theme, ThemeMode, TitleBar};
use gpui_kit::gpui::{Bounds, KeyBinding, WindowBounds, px, size};
use ui::app::{
    AddNewRow, CloseDialog, CrabStudioApp, DeleteGridRow, DuplicateGridRow, ExplainQuery,
    FormatSql, RunQuery, SaveGridChanges,
};

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs, deprecated)]
fn setup_macos_app_icon() {
    use cocoa::base::id;
    use cocoa::foundation::NSData;
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let app = cocoa::appkit::NSApp();
        if !app.is_null() {
            let bytes = ui::app::LOGO_PNG_BYTES;
            let data = NSData::dataWithBytes_length_(
                cocoa::base::nil,
                bytes.as_ptr() as *const std::ffi::c_void,
                bytes.len() as u64,
            );
            if let Some(cls) = objc::runtime::Class::get("NSImage") {
                let alloc_image: id = msg_send![cls, alloc];
                let image: id = msg_send![alloc_image, initWithData: data];
                if !image.is_null() {
                    let _: () = msg_send![app, setApplicationIconImage: image];
                }
            }
        }
    }
}

fn main() {
    // Initialize background Tokio runtime and set ambient context on main thread
    let _tokio_guard = db::tokio_runtime().enter();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            #[cfg(target_os = "macos")]
            setup_macos_app_icon();

            let settings = settings::SettingsManager::new();
            let initial_theme = match settings.settings().appearance.theme {
                settings::ThemePreference::Light => ThemeMode::Light,
                _ => ThemeMode::Dark,
            };
            Theme::change(initial_theme, None, cx);
            ui::theme::set_active_theme_mode(initial_theme == ThemeMode::Light);

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
                titlebar.title = Some("zqlcrab".into());
            }

            cx.spawn(async move |cx| {
                cx.open_window(window_options, |window, cx| {
                    let view = cx.new(|cx| CrabStudioApp::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("Failed to open zqlcrab window");
            })
            .detach();
        });
}
