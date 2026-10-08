#![allow(unexpected_cfgs)]

use tauri::{Manager, WebviewWindow};

#[cfg(target_os = "macos")]
use tauri_nspanel::objc2_app_kit::{NSWindowCollectionBehavior, NSWindowStyleMask};
#[cfg(target_os = "macos")]
use tauri_nspanel::WebviewWindowExt;
#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use tauri::menu::{Menu, MenuItemBuilder, PredefinedMenuItem};
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use tauri::tray::TrayIconBuilder;

const TRAY_ID: &str = "breaktime-tray";

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(OverlayPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true,
        }
    })
}

#[cfg(target_os = "macos")]
pub fn configure_overlay_panel(window: &WebviewWindow) -> Result<(), String> {
    apply_vibrancy(window, NSVisualEffectMaterial::HudWindow, None, None)
        .map_err(|error| error.to_string())?;
    let panel = window.to_panel::<OverlayPanel>().map_err(|error| error.to_string())?;
    panel.set_level(4);
    panel.set_style_mask(
        NSWindowStyleMask::HUDWindow | NSWindowStyleMask::NonactivatingPanel,
    );
    panel.set_collection_behavior(
        NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::CanJoinAllSpaces,
    );
    panel.set_becomes_key_only_if_needed(true);
    panel.set_floating_panel(true);
    panel.set_has_shadow(false);
    panel.set_transparent(true);
    panel.set_corner_radius(14.0);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn configure_overlay_panel(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

pub fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        use tauri::Emitter;

        let toggle_overlay = MenuItemBuilder::with_id("toggle-overlay", "Show Overlay").build(app)?;
        let open_settings = MenuItemBuilder::with_id("open-settings", "Settings & Stats").build(app)?;
        let sep1 = PredefinedMenuItem::separator(app)?;
        let eye_now = MenuItemBuilder::with_id("eye-now", "👀 Take Eye Break Now").build(app)?;
        let stretch_now = MenuItemBuilder::with_id("stretch-now", "🧍 Take Stretch Break Now").build(app)?;
        let toggle_timers = MenuItemBuilder::with_id("toggle-timers", "⏸ Pause Timers").build(app)?;
        let sep2 = PredefinedMenuItem::separator(app)?;
        let quit = MenuItemBuilder::with_id("quit", "Quit Breaktime").build(app)?;

        let menu = Menu::with_items(
            app,
            &[
                &toggle_overlay,
                &open_settings,
                &sep1,
                &eye_now,
                &stretch_now,
                &toggle_timers,
                &sep2,
                &quit,
            ],
        )?;

        let mut tray = TrayIconBuilder::with_id(TRAY_ID)
            .menu(&menu)
            .tooltip("Breaktime - Screen & Exercise Reminders");

        if let Some(icon) = app.default_window_icon() {
            tray = tray.icon(icon.clone());
        }

        let tray = tray.on_menu_event(|app, event| match event.id().as_ref() {
            "toggle-overlay" => {
                if let Some(overlay) = app.get_webview_window("overlay") {
                    let _ = overlay.show();
                    let _ = overlay.set_focus();
                }
            }
            "open-settings" => {
                let _ = app.emit("open-settings", ());
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                }
            }
            "eye-now" => {
                let _ = app.emit("trigger-break", "eye");
            }
            "stretch-now" => {
                let _ = app.emit("trigger-break", "stretch");
            }
            "toggle-timers" => {
                let _ = app.emit("toggle-timers", ());
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
    use tauri::{Emitter, Listener};

    let app_handle = app.handle().clone();
    app.listen("open-settings", move |_| {
        if let Some(main) = app_handle.get_webview_window("main") {
            let _ = main.show();
            let _ = main.set_focus();
        }
    });

    let app_handle = app.handle().clone();
    app.listen("trigger-break", move |event| {
        if let Some(overlay) = app_handle.get_webview_window("overlay") {
            let _ = overlay.show();
            let _ = overlay.set_focus();
        }
        let _ = app_handle.emit("break-triggered", event.payload());
    });

    let app_handle = app.handle().clone();
    app.listen("toggle-timers", move |_| {
        let _ = app_handle.emit("timers-toggled", ());
    });
}
