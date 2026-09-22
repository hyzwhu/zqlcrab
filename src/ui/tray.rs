#![allow(unsafe_op_in_unsafe_fn)]
//! System status bar tray menu and inter-thread action dispatcher.
//!
//! Provides a macOS status bar menu reflecting active database metrics,
//! quick connection switching, new connection shortcuts, and preferences.

use std::sync::{Mutex, OnceLock, RwLock};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs, deprecated, unused_imports)]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs, deprecated, unused_imports)]
use cocoa::foundation::{NSData, NSPoint, NSRect, NSSize, NSString};
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs, deprecated, unused_imports)]
use objc::declare::ClassDecl;
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs, deprecated, unused_imports)]
use objc::runtime::{BOOL, Class, NO, Object, Sel, YES};
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs, deprecated, unused_imports)]
use objc::{class, msg_send, sel, sel_impl};

/// Actions originating from the system status bar tray menu.
#[derive(Debug, Clone)]
pub enum TrayAction {
    /// Connect to a saved connection profile by ID.
    Connect(String),
    /// Open the New Connection dialog.
    NewConnection,
    /// Open the Settings / Preferences workspace.
    OpenSettings,
}

/// Active connection metrics displayed in the status bar tray.
#[derive(Debug, Clone, Default)]
pub struct TrayStatus {
    pub active_conn_id: Option<String>,
    pub active_name: Option<String>,
    pub db_name: Option<String>,
    pub ping_ms: Option<u64>,
    pub memory_mb: Option<f64>,
}

/// Query current process physical footprint memory (matching macOS Activity Monitor Memory column)
#[cfg(target_os = "macos")]
pub fn get_process_memory_mb() -> Option<f64> {
    #[repr(C)]
    struct TaskVmInfo {
        virtual_size: u64,
        region_count: i32,
        page_size: i32,
        resident_size: u64,
        resident_size_peak: u64,
        device: u64,
        device_peak: u64,
        internal: u64,
        internal_peak: u64,
        external: u64,
        external_peak: u64,
        reusable: u64,
        reusable_peak: u64,
        purgeable_volatile_pmap: u64,
        purgeable_volatile_resident: u64,
        purgeable_volatile_virtual: u64,
        compressed: u64,
        compressed_peak: u64,
        compressed_lifetime: u64,
        phys_footprint: u64,
        min_address: u64,
        max_address: u64,
        ledger_tag_offset: i64,
        ledger_tag_credit: i64,
        ledger_tag_debit: i64,
        ledger_tag_limit: i64,
        ledger_tag_balance: i64,
        unused: [u64; 2],
    }

    unsafe extern "C" {
        fn mach_task_self() -> u32;
        fn task_info(
            target_task: u32,
            flavor: i32,
            task_info_out: *mut TaskVmInfo,
            task_info_outCnt: *mut u32,
        ) -> i32;
    }

    const TASK_VM_INFO: i32 = 22;

    unsafe {
        let mut info = std::mem::MaybeUninit::<TaskVmInfo>::uninit();
        let mut count = (std::mem::size_of::<TaskVmInfo>() / std::mem::size_of::<u32>()) as u32;
        let kret = task_info(
            mach_task_self(),
            TASK_VM_INFO,
            info.as_mut_ptr(),
            &mut count,
        );
        if kret == 0 {
            let info = info.assume_init();
            Some(info.phys_footprint as f64 / (1024.0 * 1024.0))
        } else {
            None
        }
    }
}

/// Cross-platform memory query fallback
#[cfg(not(target_os = "macos"))]
pub fn get_process_memory_mb() -> Option<f64> {
    None
}

static TRAY_STATUS: RwLock<Option<TrayStatus>> = RwLock::new(None);
static TRAY_SENDER: OnceLock<UnboundedSender<TrayAction>> = OnceLock::new();
static TRAY_RECEIVER: Mutex<Option<UnboundedReceiver<TrayAction>>> = Mutex::new(None);

/// Update the active status displayed in the tray menu.
pub fn update_tray_status(status: Option<TrayStatus>) {
    if let Ok(mut guard) = TRAY_STATUS.write() {
        *guard = status;
    }
}

/// Retrieve the current active status displayed in the tray menu.
pub fn get_tray_status() -> Option<TrayStatus> {
    if let Ok(guard) = TRAY_STATUS.read() {
        guard.clone()
    } else {
        None
    }
}

fn get_or_init_tray_sender() -> &'static UnboundedSender<TrayAction> {
    TRAY_SENDER.get_or_init(|| {
        let (tx, rx) = unbounded_channel();
        if let Ok(mut guard) = TRAY_RECEIVER.lock() {
            *guard = Some(rx);
        }
        tx
    })
}

/// Take the tray action receiver to listen for events in the main app loop.
pub fn take_tray_receiver() -> Option<UnboundedReceiver<TrayAction>> {
    let _ = get_or_init_tray_sender();
    if let Ok(mut guard) = TRAY_RECEIVER.lock() {
        guard.take()
    } else {
        None
    }
}

/// Send a tray action into the main application channel.
pub fn send_tray_action(action: TrayAction) {
    let sender = get_or_init_tray_sender();
    let _ = sender.send(action);
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs, deprecated)]
unsafe fn activate_and_show_window() {
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

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs, deprecated)]
unsafe fn populate_menu(menu: id, target: id) {
    let _: () = msg_send![menu, removeAllItems];
    let _: () = msg_send![menu, setAutoenablesItems: NO];

    let add_item = |title: &str, action: Option<Sel>, enabled: bool| -> id {
        if let Some(item_cls) = Class::get("NSMenuItem") {
            let alloc_item: id = msg_send![item_cls, alloc];
            let t = NSString::alloc(nil).init_str(title);
            let k = NSString::alloc(nil).init_str("");
            let act = action.unwrap_or_else(|| Sel::from_ptr(std::ptr::null()));
            let item: id = msg_send![alloc_item, initWithTitle: t action: act keyEquivalent: k];
            if action.is_some() {
                let _: () = msg_send![item, setTarget: target];
            } else {
                let _: () = msg_send![item, setTarget: nil];
            }
            if !enabled {
                let _: () = msg_send![item, setEnabled: NO];
            }
            let _: () = msg_send![menu, addItem: item];
            // Release caller-allocated objects so menu has sole ownership (MRC rules)
            let _: () = msg_send![t, release];
            let _: () = msg_send![k, release];
            let _: () = msg_send![item, release];
            item
        } else {
            nil
        }
    };

    let add_separator = || {
        if let Some(item_cls) = Class::get("NSMenuItem") {
            let sep: id = msg_send![item_cls, separatorItem];
            let _: () = msg_send![menu, addItem: sep];
        }
    };

    // Header item: "zqlcrab" (clicks activate app)
    add_item("zqlcrab", Some(sel!(showWindow:)), true);

    add_separator();

    // Database status metrics block (aligned at the colon ':' with standard macOS menu typography)
    let current_status = get_tray_status();
    let (active_val, db_val, ping_val) = match current_status.as_ref() {
        Some(s) if s.active_name.is_some() => {
            let active = s.active_name.as_deref().unwrap_or("--").to_string();
            let db = s.db_name.as_deref().unwrap_or("--").to_string();
            let ping = match s.ping_ms {
                Some(ms) => format!("{ms}ms"),
                None => "--".to_string(),
            };
            (active, db, ping)
        }
        _ => ("--".to_string(), "--".to_string(), "--".to_string()),
    };

    let mem_mb =
        get_process_memory_mb().or_else(|| current_status.as_ref().and_then(|s| s.memory_mb));
    let mem_val = match mem_mb {
        Some(mb) if mb >= 1024.0 => format!("{:.2} GB", mb / 1024.0),
        Some(mb) => format!("{mb:.1} MB"),
        None => "--".to_string(),
    };

    let metrics = [
        ("Active:", active_val),
        ("DB:", db_val),
        ("Ping:", ping_val),
        ("Memory:", mem_val),
    ];

    // Render metrics aligned at the colon ':' using custom row views
    let view_cls = Class::get("NSView");
    let tf_cls = Class::get("NSTextField");
    let font_cls = Class::get("NSFont");
    let color_cls = Class::get("NSColor");
    let item_cls = Class::get("NSMenuItem");

    if let (Some(v_cls), Some(t_cls), Some(f_cls), Some(c_cls), Some(i_cls)) =
        (view_cls, tf_cls, font_cls, color_cls, item_cls)
    {
        let font: id = msg_send![f_cls, menuFontOfSize: 13.0f64];
        let sec_color: id = msg_send![c_cls, secondaryLabelColor];
        let label_color: id = msg_send![c_cls, labelColor];

        for (key, val) in metrics {
            let alloc_item: id = msg_send![i_cls, alloc];
            let blank_title = NSString::alloc(nil).init_str("");
            let blank_key = NSString::alloc(nil).init_str("");
            let item: id = msg_send![alloc_item, initWithTitle: blank_title action: nil keyEquivalent: blank_key];
            let _: () = msg_send![item, setEnabled: NO];

            let row_frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, 19.0));
            let alloc_row: id = msg_send![v_cls, alloc];
            let row_view: id = msg_send![alloc_row, initWithFrame: row_frame];

            // Key label (right-aligned, ending at x = 72.0)
            let key_ns = NSString::alloc(nil).init_str(key);
            let key_tf: id = msg_send![t_cls, labelWithString: key_ns];
            let key_frame = NSRect::new(NSPoint::new(14.0, 2.0), NSSize::new(58.0, 15.0));
            let _: () = msg_send![key_tf, setFrame: key_frame];
            let _: () = msg_send![key_tf, setAlignment: 2isize]; // NSTextAlignmentRight = 2
            let _: () = msg_send![key_tf, setFont: font];
            let _: () = msg_send![key_tf, setTextColor: sec_color];
            let _: () = msg_send![row_view, addSubview: key_tf];

            // Value label (left-aligned, starting at x = 78.0)
            let val_ns = NSString::alloc(nil).init_str(&val);
            let val_tf: id = msg_send![t_cls, labelWithString: val_ns];
            let val_frame = NSRect::new(NSPoint::new(78.0, 2.0), NSSize::new(115.0, 15.0));
            let _: () = msg_send![val_tf, setFrame: val_frame];
            let _: () = msg_send![val_tf, setAlignment: 0isize]; // NSTextAlignmentLeft = 0
            let _: () = msg_send![val_tf, setFont: font];
            let _: () = msg_send![val_tf, setTextColor: label_color];
            let _: () = msg_send![row_view, addSubview: val_tf];

            let _: () = msg_send![item, setView: row_view];
            let _: () = msg_send![menu, addItem: item];

            // Release allocated objects according to MRC rules
            let _: () = msg_send![key_ns, release];
            let _: () = msg_send![val_ns, release];
            let _: () = msg_send![blank_title, release];
            let _: () = msg_send![blank_key, release];
            let _: () = msg_send![row_view, release];
            let _: () = msg_send![item, release];
        }
    } else {
        // Fallback for non-macOS or missing AppKit classes
        for (key, val) in metrics {
            add_item(&format!("{key} {val}"), None, false);
        }
    }

    add_separator();

    // "Quick Connect >" Submenu with saved connection profiles
    if let (Some(item_cls), Some(menu_cls)) = (Class::get("NSMenuItem"), Class::get("NSMenu")) {
        let alloc_item: id = msg_send![item_cls, alloc];
        let qc_title = NSString::alloc(nil).init_str("Quick Connect");
        let qc_key = NSString::alloc(nil).init_str("");
        let qc_item: id =
            msg_send![alloc_item, initWithTitle: qc_title action: nil keyEquivalent: qc_key];

        let alloc_submenu: id = msg_send![menu_cls, alloc];
        let sub_title = NSString::alloc(nil).init_str("Quick Connect");
        let submenu: id = msg_send![alloc_submenu, initWithTitle: sub_title];
        let _: () = msg_send![submenu, setAutoenablesItems: NO];

        let manager = crate::db::manager::ConnectionManager::new();
        let configs = manager.list_configs();
        let active_id = current_status
            .as_ref()
            .and_then(|s| s.active_conn_id.as_deref());

        if configs.is_empty() {
            let no_item_alloc: id = msg_send![item_cls, alloc];
            let no_title = NSString::alloc(nil).init_str("(No saved connections)");
            let empty_key = NSString::alloc(nil).init_str("");
            let no_item: id = msg_send![no_item_alloc, initWithTitle: no_title action: nil keyEquivalent: empty_key];
            let _: () = msg_send![no_item, setEnabled: NO];
            let _: () = msg_send![submenu, addItem: no_item];
            let _: () = msg_send![no_title, release];
            let _: () = msg_send![empty_key, release];
            let _: () = msg_send![no_item, release];
        } else {
            for cfg in configs {
                let item_alloc: id = msg_send![item_cls, alloc];
                let t = NSString::alloc(nil).init_str(&cfg.name);
                let k = NSString::alloc(nil).init_str("");
                let item: id = msg_send![item_alloc, initWithTitle: t action: sel!(connectProfile:) keyEquivalent: k];
                let _: () = msg_send![item, setTarget: target];
                let rep_id = NSString::alloc(nil).init_str(&cfg.id);
                let _: () = msg_send![item, setRepresentedObject: rep_id];
                if active_id == Some(&cfg.id) {
                    let _: () = msg_send![item, setState: 1isize]; // NSControlStateValueOn (checkmark)
                } else {
                    let _: () = msg_send![item, setState: 0isize]; // NSControlStateValueOff
                }
                let _: () = msg_send![submenu, addItem: item];
                let _: () = msg_send![t, release];
                let _: () = msg_send![k, release];
                let _: () = msg_send![rep_id, release];
                let _: () = msg_send![item, release];
            }
        }

        let _: () = msg_send![qc_item, setSubmenu: submenu];
        let _: () = msg_send![menu, addItem: qc_item];
        let _: () = msg_send![qc_title, release];
        let _: () = msg_send![qc_key, release];
        let _: () = msg_send![sub_title, release];
        let _: () = msg_send![submenu, release];
        let _: () = msg_send![qc_item, release];
    }

    // New Connection...
    add_item("New Connection...", Some(sel!(newConnection:)), true);

    add_separator();

    // Preferences...
    add_item("Preferences...", Some(sel!(openPreferences:)), true);

    add_separator();

    // Quit zqlcrab
    add_item("Quit zqlcrab", Some(sel!(quitApp:)), true);
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs, deprecated)]
pub fn setup_macos_status_bar() {
    extern "C" fn show_window(_this: &Object, _cmd: Sel, _sender: id) {
        unsafe {
            activate_and_show_window();
        }
    }

    extern "C" fn connect_profile(_this: &Object, _cmd: Sel, sender: id) {
        unsafe {
            activate_and_show_window();
            if !sender.is_null() {
                let rep: id = msg_send![sender, representedObject];
                if !rep.is_null() {
                    let cstr: *const std::os::raw::c_char = msg_send![rep, UTF8String];
                    if !cstr.is_null() {
                        let id_str = std::ffi::CStr::from_ptr(cstr)
                            .to_string_lossy()
                            .into_owned();
                        send_tray_action(TrayAction::Connect(id_str));
                    }
                }
            }
        }
    }

    extern "C" fn new_connection(_this: &Object, _cmd: Sel, _sender: id) {
        unsafe {
            activate_and_show_window();
            send_tray_action(TrayAction::NewConnection);
        }
    }

    extern "C" fn open_preferences(_this: &Object, _cmd: Sel, _sender: id) {
        unsafe {
            activate_and_show_window();
            send_tray_action(TrayAction::OpenSettings);
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

    extern "C" fn menu_needs_update(_this: &Object, _cmd: Sel, menu: id) {
        unsafe {
            if !menu.is_null() {
                populate_menu(menu, _this as *const Object as id);
            }
        }
    }

    extern "C" fn menu_will_open(_this: &Object, _cmd: Sel, menu: id) {
        unsafe {
            if !menu.is_null() {
                populate_menu(menu, _this as *const Object as id);
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
            let bytes = crate::ui::app::TRAY_ICON_PNG_BYTES;
            let data = NSData::dataWithBytes_length_(
                nil,
                bytes.as_ptr() as *const std::ffi::c_void,
                bytes.len() as u64,
            );
            if let Some(cls) = Class::get("NSImage") {
                let alloc_image: id = msg_send![cls, alloc];
                let image: id = msg_send![alloc_image, initWithData: data];
                if !image.is_null() {
                    // Set as template image for native macOS monochrome silhouette rendering
                    let _: () = msg_send![image, setTemplate: YES];
                    // Proportionally sized (w: 22.0pt, h: 16.0pt) to match standard status bar icons
                    let size = NSSize::new(22.0, 16.0);
                    let _: () = msg_send![image, setSize: size];
                    let _: () = msg_send![button, setImage: image];
                    let _: () = msg_send![image, release];
                }
            }
        }

        // Setup Target Class for menu item actions and menu delegate
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
                            sel!(connectProfile:),
                            connect_profile as extern "C" fn(&Object, Sel, id),
                        );
                        decl.add_method(
                            sel!(newConnection:),
                            new_connection as extern "C" fn(&Object, Sel, id),
                        );
                        decl.add_method(
                            sel!(openPreferences:),
                            open_preferences as extern "C" fn(&Object, Sel, id),
                        );
                        decl.add_method(
                            sel!(quitApp:),
                            quit_app as extern "C" fn(&Object, Sel, id),
                        );
                        decl.add_method(
                            sel!(menuNeedsUpdate:),
                            menu_needs_update as extern "C" fn(&Object, Sel, id),
                        );
                        decl.add_method(
                            sel!(menuWillOpen:),
                            menu_will_open as extern "C" fn(&Object, Sel, id),
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
        let _: id = msg_send![target, retain];

        if let Some(menu_cls) = Class::get("NSMenu") {
            let alloc_menu: id = msg_send![menu_cls, alloc];
            let menu_title = NSString::alloc(nil).init_str("zqlcrab");
            let menu: id = msg_send![alloc_menu, initWithTitle: menu_title];
            let _: () = msg_send![menu_title, release];
            let _: () = msg_send![menu, setDelegate: target];

            // Populate initial menu
            populate_menu(menu, target);

            let _: () = msg_send![status_item, setMenu: menu];
            let _: () = msg_send![menu, release];
        }
    }
}

/// Cross-platform stub for non-macOS systems.
#[cfg(not(target_os = "macos"))]
pub fn setup_macos_status_bar() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_status_get_set() {
        update_tray_status(None);
        assert!(get_tray_status().is_none());

        let status = TrayStatus {
            active_conn_id: Some("id1".to_string()),
            active_name: Some("Local MySQL".to_string()),
            db_name: Some("test_db".to_string()),
            ping_ms: Some(42),
            memory_mb: Some(58.5),
        };
        update_tray_status(Some(status.clone()));

        let current = get_tray_status().expect("status should be set");
        assert_eq!(current.active_conn_id.as_deref(), Some("id1"));
        assert_eq!(current.active_name.as_deref(), Some("Local MySQL"));
        assert_eq!(current.db_name.as_deref(), Some("test_db"));
        assert_eq!(current.ping_ms, Some(42));
        assert_eq!(current.memory_mb, Some(58.5));

        update_tray_status(None);
        assert!(get_tray_status().is_none());
    }

    #[test]
    fn test_process_memory_mb_query() {
        #[cfg(target_os = "macos")]
        {
            let mem = get_process_memory_mb();
            assert!(mem.is_some());
            let mb = mem.unwrap();
            assert!(
                mb > 0.0,
                "Process memory should be greater than 0MB, got {mb}"
            );
        }
    }

    #[test]
    fn test_tray_action_variants() {
        let action1 = TrayAction::Connect("prod-db".to_string());
        let action2 = TrayAction::NewConnection;
        let action3 = TrayAction::OpenSettings;

        assert_eq!(format!("{action1:?}"), "Connect(\"prod-db\")");
        assert_eq!(format!("{action2:?}"), "NewConnection");
        assert_eq!(format!("{action3:?}"), "OpenSettings");
    }
}
