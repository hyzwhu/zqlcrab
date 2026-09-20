//! Desktop relational database client application entry point.

pub mod db;
pub mod settings;
pub mod ui;

use gpui_kit::AppContext;
use gpui_kit::component::input::{Copy, Cut, Paste, SelectAll};
use gpui_kit::component::{Root, Theme, ThemeMode, TitleBar};
use gpui_kit::gpui::{
    App, Bounds, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType, WindowBounds, px, size,
};
use ui::app::{
    AboutZqlcrab, AddNewRow, CheckForUpdates, CloseDialog, CloseWindow, CrabStudioApp,
    DeleteGridRow, DuplicateGridRow, ExplainQuery, FormatSql, MinimizeWindow, NewConnection,
    NewQueryTab, OpenDocs, OpenGithub, OpenSettings, Quit, RefreshTables, ReportIssue, RunQuery,
    SaveGridChanges, SelectConsoleTab, SelectGridTab, SelectHistoryTab, SelectSchemaTab,
    ToggleActivityBar, ToggleFullscreen, ToggleStatusBar, ZoomWindow,
};

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs, deprecated, unused_imports)]
use objc::{msg_send, sel, sel_impl};

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

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs, deprecated)]
fn setup_macos_status_bar() {
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSData, NSSize, NSString};
    use objc::declare::ClassDecl;
    use objc::runtime::{BOOL, Class, Object, Sel, YES, NO};
    use objc::{class, msg_send, sel, sel_impl};

    extern "C" fn show_window(_this: &Object, _cmd: Sel, _sender: id) {
        unsafe {
            let app = cocoa::appkit::NSApp();
            if !app.is_null() {
                let _: () = msg_send![app, activateIgnoringOtherApps: YES];
                let _: () = msg_send![app, unhide: nil];
                let windows: id = msg_send![app, windows];
                let count: usize = msg_send![windows, count];
                let mut has_main_window = false;
                for i in 0..count {
                    let win: id = msg_send![windows, objectAtIndex: i];
                    if !win.is_null() {
                        let can_key: BOOL = msg_send![win, canBecomeKeyWindow];
                        if can_key == YES {
                            has_main_window = true;
                            let is_mini: BOOL = msg_send![win, isMiniaturized];
                            if is_mini == YES {
                                let _: () = msg_send![win, deminiaturize: nil];
                            }
                            let _: () = msg_send![win, makeKeyAndOrderFront: nil];
                        }
                    }
                }
                if !has_main_window {
                    let delegate: id = msg_send![app, delegate];
                    if !delegate.is_null() {
                        let _: BOOL = msg_send![
                            delegate,
                            applicationShouldHandleReopen: app
                            hasVisibleWindows: NO
                        ];
                    }
                }
            }
        }
    }

    extern "C" fn hide_window(_this: &Object, _cmd: Sel, _sender: id) {
        unsafe {
            let app = cocoa::appkit::NSApp();
            if !app.is_null() {
                let _: () = msg_send![app, hide: nil];
            }
        }
    }

    extern "C" fn quit_app(_this: &Object, _cmd: Sel, _sender: id) {
        unsafe {
            let app = cocoa::appkit::NSApp();
            if !app.is_null() {
                let _: () = msg_send![app, terminate: nil];
            }
        }
    }

    unsafe {
        let status_bar: id = msg_send![class!(NSStatusBar), systemStatusBar];
        if status_bar.is_null() {
            return;
        }

        // NSVariableStatusItemLength = -1.0
        let status_item: id = msg_send![status_bar, statusItemWithLength: -1.0f64];
        if status_item.is_null() {
            return;
        }
        let _: id = msg_send![status_item, retain];

        let button: id = msg_send![status_item, button];
        if !button.is_null() {
            let bytes = ui::app::LOGO_PNG_BYTES;
            let data = NSData::dataWithBytes_length_(
                nil,
                bytes.as_ptr() as *const std::ffi::c_void,
                bytes.len() as u64,
            );
            if let Some(cls) = Class::get("NSImage") {
                let alloc_image: id = msg_send![cls, alloc];
                let image: id = msg_send![alloc_image, initWithData: data];
                if !image.is_null() {
                    let size = NSSize::new(18.0, 18.0);
                    let _: () = msg_send![image, setSize: size];
                    let _: () = msg_send![button, setImage: image];
                }
            }
        }

        // Setup Target Class for menu item actions
        let target_cls = match Class::get("ZqlcrabTrayTarget") {
            Some(cls) => cls,
            None => {
                if let Some(super_cls) = Class::get("NSObject") {
                    if let Some(mut decl) = ClassDecl::new("ZqlcrabTrayTarget", super_cls) {
                        decl.add_method(
                            sel!(showWindow:),
                            show_window as extern "C" fn(&Object, Sel, id),
                        );
                        decl.add_method(
                            sel!(hideWindow:),
                            hide_window as extern "C" fn(&Object, Sel, id),
                        );
                        decl.add_method(
                            sel!(quitApp:),
                            quit_app as extern "C" fn(&Object, Sel, id),
                        );
                        decl.register()
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
        };

        let target: id = msg_send![target_cls, alloc];
        let target: id = msg_send![target, init];

        if let Some(menu_cls) = Class::get("NSMenu") {
            let alloc_menu: id = msg_send![menu_cls, alloc];
            let menu_title = NSString::alloc(nil).init_str("zqlcrab");
            let menu: id = msg_send![alloc_menu, initWithTitle: menu_title];

            let add_item = |title: &str, action: Sel| {
                if let Some(item_cls) = Class::get("NSMenuItem") {
                    let alloc_item: id = msg_send![item_cls, alloc];
                    let t = NSString::alloc(nil).init_str(title);
                    let k = NSString::alloc(nil).init_str("");
                    let item: id =
                        msg_send![alloc_item, initWithTitle: t action: action keyEquivalent: k];
                    let _: () = msg_send![item, setTarget: target];
                    let _: () = msg_send![menu, addItem: item];
                }
            };

            add_item("Show zqlcrab", sel!(showWindow:));
            add_item("Hide zqlcrab", sel!(hideWindow:));

            if let Some(item_cls) = Class::get("NSMenuItem") {
                let sep: id = msg_send![item_cls, separatorItem];
                let _: () = msg_send![menu, addItem: sep];
            }

            add_item("Quit zqlcrab", sel!(quitApp:));

            let _: () = msg_send![status_item, setMenu: menu];
        }
    }
}

fn open_main_window(cx: &mut App) {
    cx.activate(true);

    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
    let mut window_options = TitleBar::window_options();
    window_options.window_bounds = Some(WindowBounds::Windowed(bounds));
    if let Some(titlebar) = window_options.titlebar.as_mut() {
        titlebar.title = Some("zqlcrab".into());
    }

    let _ = cx.open_window(window_options, |window, cx| {
        let view = cx.new(|cx| CrabStudioApp::new(window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    });
}

#[allow(unexpected_cfgs, deprecated)]
fn main() {
    // Initialize background Tokio runtime and set ambient context on main thread
    let _tokio_guard = db::tokio_runtime().enter();

    let app = gpui_kit::application().with_assets(gpui_kit::assets::AllAssets);

    #[allow(clippy::redundant_closure)]
    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            open_main_window(cx);
        } else {
            cx.activate(true);
            #[cfg(target_os = "macos")]
            unsafe {
                let app = cocoa::appkit::NSApp();
                if !app.is_null() {
                    let _: () = objc::msg_send![app, activateIgnoringOtherApps: objc::runtime::YES];
                    let _: () = objc::msg_send![app, unhide: cocoa::base::nil];
                    let windows: cocoa::base::id = objc::msg_send![app, windows];
                    let count: usize = objc::msg_send![windows, count];
                    for i in 0..count {
                        let win: cocoa::base::id = objc::msg_send![windows, objectAtIndex: i];
                        if !win.is_null() {
                            let can_key: objc::runtime::BOOL = objc::msg_send![win, canBecomeKeyWindow];
                            if can_key == objc::runtime::YES {
                                let is_mini: objc::runtime::BOOL = objc::msg_send![win, isMiniaturized];
                                if is_mini == objc::runtime::YES {
                                    let _: () = objc::msg_send![win, deminiaturize: cocoa::base::nil];
                                }
                                let _: () = objc::msg_send![win, makeKeyAndOrderFront: cocoa::base::nil];
                            }
                        }
                    }
                }
            }
        }
    });

    app.run(|cx| {
            gpui_kit::init(cx);
            #[cfg(target_os = "macos")]
            {
                setup_macos_app_icon();
                setup_macos_status_bar();
            }

            let settings = settings::SettingsManager::new();
            let initial_theme = match settings.settings().appearance.theme {
                settings::ThemePreference::Light => ThemeMode::Light,
                _ => ThemeMode::Dark,
            };
            Theme::change(initial_theme, None, cx);
            ui::theme::set_active_theme_mode(initial_theme == ThemeMode::Light);

            // Global application action listeners
            cx.on_action(|_: &Quit, cx| {
                cx.quit();
            });

            // Application Keybindings (macOS Command & Cross-platform Ctrl)
            cx.bind_keys([
                // Application Lifecycle (Quit) - cmd-q and ctrl-q
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("ctrl-q", Quit, None),
                // Window & Dialog Controls
                KeyBinding::new("cmd-w", CloseWindow, None),
                KeyBinding::new("ctrl-w", CloseWindow, None),
                KeyBinding::new("escape", CloseDialog, Some("CrabStudio")),
                KeyBinding::new("cmd-m", MinimizeWindow, None),
                KeyBinding::new("ctrl-m", MinimizeWindow, None),
                KeyBinding::new("ctrl-cmd-f", ToggleFullscreen, None),
                // Preferences / Settings
                KeyBinding::new("cmd-,", OpenSettings, None),
                KeyBinding::new("ctrl-,", OpenSettings, None),
                // Navigation & Connection Management
                KeyBinding::new("cmd-shift-n", NewConnection, None),
                KeyBinding::new("ctrl-shift-n", NewConnection, None),
                KeyBinding::new("cmd-t", NewQueryTab, None),
                KeyBinding::new("ctrl-t", NewQueryTab, None),
                KeyBinding::new("cmd-r", RefreshTables, None),
                KeyBinding::new("ctrl-r", RefreshTables, None),
                // Workspace Tab Switching
                KeyBinding::new("cmd-1", SelectConsoleTab, None),
                KeyBinding::new("ctrl-1", SelectConsoleTab, None),
                KeyBinding::new("cmd-2", SelectGridTab, None),
                KeyBinding::new("ctrl-2", SelectGridTab, None),
                KeyBinding::new("cmd-3", SelectSchemaTab, None),
                KeyBinding::new("ctrl-3", SelectSchemaTab, None),
                KeyBinding::new("cmd-4", SelectHistoryTab, None),
                KeyBinding::new("ctrl-4", SelectHistoryTab, None),
                // Query Execution & Formatting
                KeyBinding::new("cmd-enter", RunQuery, Some("CrabStudio")),
                KeyBinding::new("ctrl-enter", RunQuery, Some("CrabStudio")),
                KeyBinding::new("alt-shift-f", FormatSql, Some("CrabStudio")),
                KeyBinding::new("cmd-shift-e", ExplainQuery, Some("CrabStudio")),
                KeyBinding::new("ctrl-shift-e", ExplainQuery, Some("CrabStudio")),
                // Grid Data Editing & Mutation
                KeyBinding::new("cmd-s", SaveGridChanges, Some("CrabStudio")),
                KeyBinding::new("ctrl-s", SaveGridChanges, Some("CrabStudio")),
                KeyBinding::new("cmd-n", AddNewRow, Some("CrabStudio")),
                KeyBinding::new("ctrl-n", AddNewRow, Some("CrabStudio")),
                KeyBinding::new("cmd-d", DuplicateGridRow, Some("CrabStudio")),
                KeyBinding::new("ctrl-d", DuplicateGridRow, Some("CrabStudio")),
                KeyBinding::new("cmd-backspace", DeleteGridRow, Some("CrabStudio")),
                KeyBinding::new("ctrl-backspace", DeleteGridRow, Some("CrabStudio")),
            ]);

            // Top-left native Application Menus
            cx.set_menus([
                Menu::new("zqlcrab").items([
                    MenuItem::action("About zqlcrab", AboutZqlcrab),
                    MenuItem::action("Check for Updates...", CheckForUpdates),
                    MenuItem::separator(),
                    MenuItem::action("Settings...", OpenSettings),
                    MenuItem::separator(),
                    MenuItem::os_submenu("Services", SystemMenuType::Services),
                    MenuItem::separator(),
                    MenuItem::action("Quit zqlcrab", Quit),
                ]),
                Menu::new("File").items([
                    MenuItem::action("New Connection...", NewConnection),
                    MenuItem::action("New Query Tab", NewQueryTab),
                    MenuItem::separator(),
                    MenuItem::action("Save Changes", SaveGridChanges),
                    MenuItem::separator(),
                    MenuItem::action("Close Window", CloseWindow),
                ]),
                Menu::new("Edit").items([
                    MenuItem::os_action("Cut", Cut, OsAction::Cut),
                    MenuItem::os_action("Copy", Copy, OsAction::Copy),
                    MenuItem::os_action("Paste", Paste, OsAction::Paste),
                    MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
                    MenuItem::separator(),
                    MenuItem::action("Format SQL", FormatSql),
                ]),
                Menu::new("View").items([
                    MenuItem::action("Query Console", SelectConsoleTab),
                    MenuItem::action("Data Grid", SelectGridTab),
                    MenuItem::action("Table Schema", SelectSchemaTab),
                    MenuItem::action("Query History", SelectHistoryTab),
                    MenuItem::separator(),
                    MenuItem::action("Toggle Activity Bar", ToggleActivityBar),
                    MenuItem::action("Toggle Status Bar", ToggleStatusBar),
                    MenuItem::separator(),
                    MenuItem::action("Refresh Tables", RefreshTables),
                ]),
                Menu::new("Window").items([
                    MenuItem::action("Minimize", MinimizeWindow),
                    MenuItem::action("Zoom", ZoomWindow),
                    MenuItem::action("Toggle Full Screen", ToggleFullscreen),
                    MenuItem::separator(),
                    MenuItem::action("Close Window", CloseWindow),
                ]),
                Menu::new("Help").items([
                    MenuItem::action("Documentation", OpenDocs),
                    MenuItem::action("GitHub Repository", OpenGithub),
                    MenuItem::action("Report Issue", ReportIssue),
                    MenuItem::separator(),
                    MenuItem::action("About zqlcrab", AboutZqlcrab),
                ]),
            ]);

            open_main_window(cx);
        });
}
