#![allow(unexpected_cfgs)]
// tauri_nspanel's panel_event! macro requires `-> ()` in its event signatures.
#![allow(clippy::unused_unit)]

use tauri::{AppHandle, Manager, WebviewWindow};

#[cfg(target_os = "macos")]
use tauri_nspanel::objc2_app_kit::{
    NSAutoresizingMaskOptions, NSTrackingArea, NSTrackingAreaOptions,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
#[cfg(target_os = "macos")]
use tauri_nspanel::{ManagerExt as PanelManagerExt, WebviewWindowExt};

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use tauri::menu::{CheckMenuItem, Menu, MenuItemBuilder, PredefinedMenuItem};
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use tauri::tray::TrayIconBuilder;

const TRAY_ID: &str = "breaktime-tray";

/// Handle to the tray's "2× HUD Size" checkbox so its check state can track
/// changes made from the HUD dropdown (and vice versa).
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub struct TrayState(pub std::sync::Mutex<Option<CheckMenuItem<tauri::Wry>>>);

/// Flip `settings.hud2x` on disk and mirror it to the frontend through the
/// same settings-changed envelope the webviews use (sender "rust").
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn flip_hud_2x<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<bool> {
    use tauri::Emitter;

    let mut v = crate::settings::get_setting_value(app, "settings")?;
    let current = v.get("hud2x").and_then(|b| b.as_bool()).unwrap_or(false);
    let next = !current;
    if let Some(obj) = v.as_object_mut() {
        obj.insert("hud2x".to_string(), serde_json::json!(next));
    }
    crate::settings::set_setting_value(app, "settings".to_string(), v.clone()).ok()?;
    let _ = app.emit(
        "breaktime:settings-changed",
        serde_json::json!({ "sender": "rust", "data": v }),
    );
    Some(next)
}

/// Keep the tray checkbox in sync with an externally supplied hud2x value.
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn set_tray_hud_2x_checked<R: tauri::Runtime>(app: &tauri::AppHandle<R>, checked: bool) {
    use tauri::Manager;
    if let Some(item) = app
        .try_state::<TrayState>()
        .and_then(|s| s.0.lock().ok().and_then(|g| g.clone()))
    {
        let _ = item.set_checked(checked);
    }
}

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(OverlayPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true,
            hides_on_deactivate: false,
        }
    })

    panel_event!(OverlayPanelEventHandler {
        window_did_move(notification: &tauri_nspanel::objc2_foundation::NSNotification) -> (),
        window_did_resize(notification: &tauri_nspanel::objc2_foundation::NSNotification) -> (),
    })
}

#[cfg(target_os = "macos")]
pub fn configure_overlay_panel(window: &WebviewWindow) -> Result<(), String> {
    use tauri::Emitter;

    let panel = window.to_panel::<OverlayPanel>().map_err(|error| error.to_string())?;
    panel.set_level(4); // NSFloatingWindowLevel: floats above standard windows
    let _ = panel.set_style_mask(
        NSWindowStyleMask::Borderless | NSWindowStyleMask::Resizable | NSWindowStyleMask::NonactivatingPanel,
    );
    panel.set_collection_behavior(
        NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    panel.set_becomes_key_only_if_needed(true);
    panel.set_floating_panel(true);
    panel.set_hides_on_deactivate(false);
    panel.set_has_shadow(false);
    panel.set_transparent(true);
    panel.set_corner_radius(16.0);

    unsafe {
        let _: () = tauri_nspanel::objc2::msg_send![panel.as_panel(), setMovableByWindowBackground: true];

        let content_view: Retained<NSView> = tauri_nspanel::objc2::msg_send![panel.as_panel(), contentView];
        let bounds: NSRect = tauri_nspanel::objc2::msg_send![&content_view, bounds];

        let tracking_options = NSTrackingAreaOptions::ActiveAlways
            | NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::MouseMoved
            | NSTrackingAreaOptions::InVisibleRect;

        let alloc: *mut NSTrackingArea = tauri_nspanel::objc2::msg_send![
            NSTrackingArea::class(),
            alloc
        ];
        let area: *mut NSTrackingArea = tauri_nspanel::objc2::msg_send![
            alloc,
            initWithRect: bounds,
            options: tracking_options,
            owner: panel.as_panel(),
            userInfo: tauri_nspanel::objc2::ffi::nil
        ];
        let tracking_area = Retained::from_raw(area).unwrap();

        let resize_mask = NSAutoresizingMaskOptions::ViewWidthSizable
            | NSAutoresizingMaskOptions::ViewHeightSizable;
        let _: () = tauri_nspanel::objc2::msg_send![&content_view, setAutoresizingMask: resize_mask];
        let _: () = tauri_nspanel::objc2::msg_send![&content_view, addTrackingArea: &*tracking_area];
    }

    let handler = OverlayPanelEventHandler::new();
    let app_move = window.app_handle().clone();
    handler.window_did_move(move |_notification| {
        let app_handle = app_move.clone();
        if let Some(overlay) = app_handle.get_webview_window("overlay") {
            if let Ok(phys_pos) = overlay.outer_position() {
                let (x, y) = (phys_pos.x as f64, phys_pos.y as f64);
                crate::window_state::mutate_window_state(&app_handle, |state| {
                    state.minimalist.x = Some(x);
                    state.minimalist.y = Some(y);
                });
            }
        }
    });

    let app_enter = window.app_handle().clone();
    handler.on_mouse_entered(move |_event| {
        if let Some(overlay_window) = app_enter.get_webview_window("overlay") {
            let _ = overlay_window.emit("overlay-entered", ());
        }
    });

    let app_exit = window.app_handle().clone();
    handler.on_mouse_exited(move |_event| {
        if let Some(overlay_window) = app_exit.get_webview_window("overlay") {
            let _ = overlay_window.emit("overlay-exited", ());
        }
    });

    panel.set_event_handler(Some(handler.as_ref()));

    Ok(())
}

/// Windows stand-in for the macOS NSPanel setup. The overlay is a plain
/// borderless transparent Tauri window (see tauri.conf.json); this applies
/// the panel chrome macOS gets for free:
/// - set_focusable(false): clicking/dragging the HUD never steals focus from
///   the active app (the NSPanel's NonactivatingPanel behaviour). tao maps
///   this to WS_EX_NOACTIVATE in its own style computation, so it survives
///   the style rewrites tao does on every flag change.
/// - WS_EX_TOOLWINDOW: no Alt-Tab entry (IgnoresCycle). tao's skipTaskbar
///   only deletes the taskbar tab via ITaskbarList; without this the HUD
///   still cycles through Alt-Tab. Must be poked raw *after* tao's
///   flag-driven rewrites (see show_overlay_panel ordering).
#[cfg(target_os = "windows")]
pub fn configure_overlay_panel<R: tauri::Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::*;

    window.set_focusable(false).map_err(|e| e.to_string())?;

    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    // Re-wrap the raw handle so we don't depend on which `windows`
    // crate version tauri re-exports (same trick as set_opacity below).
    let h = HWND(hwnd.0 as *mut _);
    unsafe {
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        let _ = SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW.0 as isize);
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn configure_overlay_panel<R: tauri::Runtime>(_window: &WebviewWindow<R>) -> Result<(), String> {
    Ok(())
}

pub fn show_overlay_panel<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(panel) = app.get_webview_panel("overlay") {
            panel.set_level(4);
            panel.show();
            return Ok(());
        }
    }
    if let Some(overlay) = app.get_webview_window("overlay") {
        // Re-assert topmost on every show (the macOS branch re-sets the
        // panel level) and don't focus: showing the HUD must not steal
        // focus from the active app (non-activating panel behaviour).
        let _ = overlay.set_always_on_top(true);
        overlay.show().map_err(|e| e.to_string())?;
        // tao recomputes the window styles from its own flags on the calls
        // above (apply_diff), so the tool-window chrome has to be re-applied
        // *after* them or it gets wiped on every hide/show cycle.
        #[cfg(target_os = "windows")]
        {
            let _ = configure_overlay_panel(&overlay);
        }
    }
    Ok(())
}

/// Adjust the overlay panel's alpha natively (0.0 transparent – 1.0 opaque),
/// matching llm_gauge. CSS opacity on a transparent webview window is
/// unreliable (flickers / doesn't apply while compositing), but native window
/// alpha is solid: NSPanel setAlphaValue on macOS, a WS_EX_LAYERED layered
/// window with SetLayeredWindowAttributes on Windows.
#[tauri::command]
pub fn native_overlay_set_opacity(app: tauri::AppHandle, alpha: f64) -> Result<(), String> {
    let clamped = alpha.clamp(0.0, 1.0);
    #[cfg(target_os = "macos")]
    {
        if let Ok(panel) = app.get_webview_panel("overlay") {
            unsafe {
                let _: () = tauri_nspanel::objc2::msg_send![panel.as_panel(), setAlphaValue: clamped];
            }
            return Ok(());
        }
    }
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::*;

        if let Some(overlay) = app.get_webview_window("overlay") {
            if let Ok(hwnd) = overlay.hwnd() {
                // Re-wrap the raw handle so we don't depend on which `windows`
                // crate version tauri re-exports.
                let h = HWND(hwnd.0 as *mut _);
                unsafe {
                    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
                    let _ = SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_LAYERED.0 as isize);
                    let _ = SetLayeredWindowAttributes(
                        h,
                        windows::Win32::Foundation::COLORREF(0),
                        (clamped * 255.0).round() as u8,
                        LWA_ALPHA,
                    );
                }
            }
        }
        return Ok(());
    }
    #[allow(unreachable_code)]
    {
        let _ = clamped;
        Ok(())
    }
}

pub fn hide_overlay_panel<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(panel) = app.get_webview_panel("overlay") {
            panel.hide();
            return Ok(());
        }
    }
    if let Some(overlay) = app.get_webview_window("overlay") {
        overlay.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn is_overlay_panel_visible<R: tauri::Runtime>(app: &AppHandle<R>) -> bool {
    #[cfg(target_os = "macos")]
    {
        if let Ok(panel) = app.get_webview_panel("overlay") {
            return panel.is_visible();
        }
    }
    if let Some(overlay) = app.get_webview_window("overlay") {
        return overlay.is_visible().unwrap_or(false);
    }
    false
}

pub fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        use tauri::Emitter;

        let toggle_overlay = MenuItemBuilder::with_id("toggle-overlay", "Toggle Visible").build(app)?;
        let open_settings = MenuItemBuilder::with_id("open-settings", "Settings & Stats").build(app)?;
        let sep1 = PredefinedMenuItem::separator(app)?;
        let eye_now = MenuItemBuilder::with_id("eye-now", "Take Eye Break Now").build(app)?;
        let stretch_now = MenuItemBuilder::with_id("stretch-now", "Take Stretch Break Now").build(app)?;
        let initial_2x = crate::settings::get_setting_value(app.handle(), "settings")
            .and_then(|v| v.get("hud2x").and_then(|b| b.as_bool()))
            .unwrap_or(false);
        let hud_2x = tauri::menu::CheckMenuItemBuilder::with_id("hud-2x", "2× Size")
            .checked(initial_2x)
            .build(app)?;
        app.manage(TrayState(std::sync::Mutex::new(Some(hud_2x.clone()))));
        // let sep2 = PredefinedMenuItem::separator(app)?;
        // let reset_eye = MenuItemBuilder::with_id("reset-eye", "👀 Reset Eye Timer").build(app)?;
        // let reset_stretch = MenuItemBuilder::with_id("reset-stretch", "🧍 Reset Stretch Timer").build(app)?;
        let sep3 = PredefinedMenuItem::separator(app)?;
        let quit = MenuItemBuilder::with_id("quit", "Quit Breaktime").build(app)?;

        let menu = Menu::with_items(
            app,
            &[
                &toggle_overlay,
                &open_settings,
                &sep1,
                &eye_now,
                &stretch_now,
                &hud_2x,
                // &sep2,
                // &reset_eye,
                // &reset_stretch,
                &sep3,
                &quit,
            ],
        )?;

        let mut tray = TrayIconBuilder::with_id(TRAY_ID)
            .menu(&menu)
            .tooltip("Breaktime - Screen & Exercise Reminders");

        // Dedicated menu-bar icon. On macOS it is a template image (black
        // pixels + alpha), so the system tints it black in light mode and
        // white in dark mode automatically.
        #[cfg(target_os = "macos")]
        {
            match tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon-template.png")) {
                Ok(icon) => tray = tray.icon(icon).icon_as_template(true),
                Err(_) => {
                    if let Some(icon) = app.default_window_icon() {
                        tray = tray.icon(icon.clone());
                    }
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        if let Some(icon) = app.default_window_icon() {
            tray = tray.icon(icon.clone());
        }

        let tray = tray.on_menu_event(|app, event| match event.id().as_ref() {
            "toggle-overlay" => {
                if is_overlay_panel_visible(app) {
                    let _ = hide_overlay_panel(app);
                } else {
                    let _ = crate::window_state::enter_overlay_mode(app.clone());
                }
            }
            "open-settings" => {
                let _ = crate::window_state::exit_overlay_mode(app.clone());
            }
            "eye-now" => {
                let _ = app.emit("trigger-break", "eye");
            }
            "stretch-now" => {
                let _ = app.emit("trigger-break", "stretch");
            }
            "hud-2x" => {
                if let Some(next) = flip_hud_2x(app) {
                    set_tray_hud_2x_checked(app, next);
                }
            }
            "reset-eye" => {
                let _ = app.emit("reset-timer", "eye");
            }
            "reset-stretch" => {
                let _ = app.emit("reset-timer", "stretch");
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        });

        tray.build(app)?;
    }
    Ok(())
}

pub fn setup_event_listeners(app: &tauri::App) {
    use tauri::Listener;

    let app_handle = app.handle().clone();
    app.listen("trigger-break", move |_| {
        let _ = crate::window_state::enter_overlay_mode(app_handle.clone());
    });

    // The HUD dropdown can also toggle 2× mode — keep the tray checkbox honest.
    let app_handle = app.handle().clone();
    app.listen("breaktime:settings-changed", move |event| {
        #[derive(serde::Deserialize)]
        struct Envelope {
            data: serde_json::Value,
        }
        let Ok(env) = serde_json::from_str::<Envelope>(event.payload()) else {
            return;
        };
        if let Some(hud2x) = env.data.get("hud2x").and_then(|b| b.as_bool()) {
            set_tray_hud_2x_checked(&app_handle, hud2x);
        }
    });
}
