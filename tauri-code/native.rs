#![allow(unexpected_cfgs)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::sync_channel;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use std::process::Command;
use tauri::{
    AppHandle, Emitter, Listener, LogicalPosition, LogicalSize, Manager, Position, Size, WebviewWindow,
};
use tauri_plugin_dialog::DialogExt;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "ios", target_os = "android"))]
use tauri_plugin_sharekit::{ShareExt, ShareFileOptions};
#[cfg(target_os = "macos")]
use tauri_nspanel::objc2_app_kit::{NSWindowCollectionBehavior, NSWindowStyleMask};
#[cfg(target_os = "macos")]
use tauri_nspanel::{ManagerExt as PanelManagerExt, Panel, WebviewWindowExt};
#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use tauri::menu::{Menu, MenuItemBuilder, PredefinedMenuItem};
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use tauri::tray::TrayIconBuilder;
// #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
// use tauri_plugin_window_state::{WindowExt, StateFlags};
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
use tauri::Runtime;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use active_win_pos_rs::get_active_window;

const MAIN_WINDOW: &str = "main";
const OVERLAY_WINDOW: &str = "overlay";
const TRAY_ID: &str = "main-tray";

const TRAY_EVENT: &str = "timeflow-tray";
const OVERLAY_TO_MAIN_EVENT: &str = "timeflow-overlay-main";
const MAIN_TO_OVERLAY_EVENT: &str = "timeflow-overlay-window";

#[cfg(target_os = "macos")]
const OVERLAY_CORNER_RADIUS: f64 = 16.0;

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(OverlayPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true,
        }
    })

    panel!(FullscreenPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true,
        }
    })

    panel_event!(OverlayPanelEventHandler {
        window_did_move(notification: &tauri_nspanel::objc2_foundation::NSNotification) -> (),
        window_did_resize(notification: &tauri_nspanel::objc2_foundation::NSNotification) -> (),
    })
}


#[derive(Debug, Default, Deserialize)]
pub struct WindowOptions {
    width: Option<f64>,
    height: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TrayPomoInfo {
    status: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayTimerInfo {
    start_at: Option<i64>,
    paused_at: Option<i64>,
    elapsed_time: Option<String>,
    pomo_timer: Option<TrayPomoInfo>,
}

#[derive(Debug, Serialize)]
pub struct MediaDebugInfo {
    media_root: String,
    root_exists: bool,
    entries: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeActiveWindowInfo {
    app: String,
    title: String,
    process_path: String,
    process_id: String,
    window_id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub fn setup(app: &mut tauri::App<tauri::Wry>) -> tauri::Result<()> {
    setup_tray(app)?;
    let _ = ensure_window(app.handle(), "fullscreen");
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn setup_tray(app: &mut tauri::App<tauri::Wry>) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id("show", "Show ARC").build(app)?;
    let toggle_overlay = MenuItemBuilder::with_id("toggle-overlay", "Toggle Overlay").build(app)?;
    let show_files = MenuItemBuilder::with_id("show-files", "Show Files").build(app)?;
    let toggle_timer =
        MenuItemBuilder::with_id("timer-toggle", "Pause / Resume Timer").build(app)?;
    let stop_timer = MenuItemBuilder::with_id("timer-stop", "Stop Timer").build(app)?;
    let start_pomo = MenuItemBuilder::with_id("pomo-start", "Start Pomo").build(app)?;
    let stop_pomo = MenuItemBuilder::with_id("pomo-stop", "Stop Pomo").build(app)?;
    #[cfg(any(debug_assertions, feature = "devtools"))]
    let toggle_devtools =
        MenuItemBuilder::with_id("toggle-devtools", "Toggle Web Inspector").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit ARC").build(app)?;
    let separator = PredefinedMenuItem::separator(app)?;

    let menu = {
        #[cfg(any(debug_assertions, feature = "devtools"))]
        {
            Menu::with_items(
                app,
                &[
                    &show,
                    &toggle_overlay,
                    &show_files,
                    &separator,
                    &toggle_timer,
                    &stop_timer,
                    &start_pomo,
                    &stop_pomo,
                    &toggle_devtools,
                    &separator,
                    &quit,
                ],
            )?
        }
        #[cfg(not(any(debug_assertions, feature = "devtools")))]
        {
            Menu::with_items(
                app,
                &[
                    &show,
                    &toggle_overlay,
                    &show_files,
                    &separator,
                    &toggle_timer,
                    &stop_timer,
                    &start_pomo,
                    &stop_pomo,
                    &separator,
                    &quit,
                ],
            )?
        }
    };
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("ARC - Time Focus");

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.on_menu_event(|app, event| match event.id().as_ref() {
        "show" => {
            let _ = show_main_window(app);
        }
        "toggle-overlay" => {
            let _ = toggle_overlay_window(app);
        }
        "show-files" => {
            let _ = show_media_files(app);
        }
        "timer-toggle" => {
            let _ = emit_to_main(app, TRAY_EVENT, "timerToggle");
        }
        "timer-stop" => {
            let _ = emit_to_main(app, TRAY_EVENT, "timerStop");
        }
        "pomo-start" => {
            let _ = emit_to_main(app, TRAY_EVENT, "pomoStart");
        }
        "pomo-stop" => {
            let _ = emit_to_main(app, TRAY_EVENT, "pomoStop");
        }
        #[cfg(any(debug_assertions, feature = "devtools"))]
        "toggle-devtools" => {
            let _ = toggle_main_devtools(app);
        }
        "quit" => app.exit(0),
        _ => {}
    })
    .build(app.handle())?;

    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn setup<R: Runtime>(_app: &mut tauri::App<R>) -> tauri::Result<()> {
    Ok(())
}

fn ensure_window(app: &AppHandle, label: &str) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(label) {
        eprintln!(
            "[OVERLAY-DBG] ensure_window(label='{}') → reusing existing window",
            label
        );
        return Ok(window);
    }

    eprintln!(
        "[OVERLAY-DBG] ensure_window(label='{}') → window NOT found, creating new HWND+WebView2",
        label
    );

    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == label)
        .ok_or_else(|| format!("Missing window config: {label}"))?;

    let window = tauri::WebviewWindowBuilder::from_config(app, config)
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?;
    configure_window(label, &window)?;

    // Wire up position/size persistence and restore saved state for this window.
    // This handles the overlay window being created lazily (after startup).
    // Skip for fullscreen window as it doesn't need state persistence.
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    if label != "fullscreen" {
        crate::window_state::watch_window(app, label);
        crate::window_state::restore_window(app, label);
    }

    Ok(window)
}

fn configure_window(label: &str, window: &WebviewWindow) -> Result<(), String> {
    let _ = label;
    let _ = window;
    #[cfg(target_os = "macos")]
    if label == OVERLAY_WINDOW {
        configure_overlay_panel(window)?;
    }

    #[cfg(target_os = "windows")]
    if label == OVERLAY_WINDOW {
        configure_overlay_window_win32(window)?;
    }

    #[cfg(target_os = "macos")]
    if label == "fullscreen" {
        // Configure the panel during window creation (like overlay)
        configure_fullscreen_panel_basic(window)?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn configure_fullscreen_panel_basic(window: &WebviewWindow) -> Result<(), String> {
    use tauri_nspanel::PanelLevel;

    // Just convert to panel and set basic properties
    // Screen-specific configuration happens in native_fullscreen_show
    let panel = window.to_panel::<FullscreenPanel>().map_err(|error| error.to_string())?;

    // Set basic panel properties
    panel.set_level(PanelLevel::ScreenSaver.value());
    panel.set_style_mask(NSWindowStyleMask::NonactivatingPanel);
    panel.set_collection_behavior(
        NSWindowCollectionBehavior::FullScreenAuxiliary | NSWindowCollectionBehavior::CanJoinAllSpaces
    );

    // Prevent panel from hiding when app deactivates
    unsafe {
        let _: () = tauri_nspanel::objc2::msg_send![panel.as_panel(), setHidesOnDeactivate: false];
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn configure_fullscreen_panel(window: &WebviewWindow, screen_index: Option<i32>, opacity: Option<f64>) -> Result<(), String> {

    // Panel is already converted in configure_window, just get it
    let panel = window.to_panel::<FullscreenPanel>().map_err(|error| error.to_string())?;

    // Get all available monitors
    let monitors = match window.available_monitors() {
        Ok(m) => m,
        Err(_) => vec![],
    };

    // Check if we should span all screens (screenIndex = -1)
    if screen_index == Some(-1) && !monitors.is_empty() {
        // Calculate bounding rectangle of all monitors
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for monitor in &monitors {
            let pos = monitor.position();
            let size = monitor.size();

            min_x = min_x.min(pos.x);
            min_y = min_y.min(pos.y);
            max_x = max_x.max(pos.x + size.width as i32);
            max_y = max_y.max(pos.y + size.height as i32);
        }

        let width = (max_x - min_x) as f64;
        let height = (max_y - min_y) as f64;
        let scale_factor = window.scale_factor().unwrap_or(1.0);

        let logical_size = tauri::LogicalSize::new(width / scale_factor, height / scale_factor);
        let logical_pos = tauri::LogicalPosition::new(min_x as f64 / scale_factor, min_y as f64 / scale_factor);

        let _ = window.set_size(tauri::Size::Logical(logical_size));
        let _ = window.set_position(tauri::Position::Logical(logical_pos));
    } else {
        // Use specific screen or current monitor
        let target_monitor = if let Some(idx) = screen_index {
            // Use specific screen index (0-based)
            if idx >= 0 && (idx as usize) < monitors.len() {
                Some(monitors[idx as usize].clone())
            } else if idx as usize == monitors.len() {
                // Index equals length, use the last monitor
                monitors.last().cloned()
            } else {
                // Invalid index, fall back to current monitor
                window.current_monitor().ok().flatten()
            }
        } else {
            // No specific screen requested - use current monitor
            window.current_monitor().ok().flatten()
        };

        // Set panel size and position for single monitor
        if let Some(monitor) = target_monitor {
            let size = monitor.size();
            let position = monitor.position();
            let scale_factor = monitor.scale_factor();
            let logical_size = size.to_logical::<f64>(scale_factor);
            let logical_pos = position.to_logical::<f64>(scale_factor);
            let _ = window.set_size(tauri::Size::Logical(logical_size));
            let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(logical_pos.x, logical_pos.y)));
        } else {
            // Fallback: cover current monitor
            if let Ok(Some(monitor)) = window.current_monitor() {
                let size = monitor.size();
                let scale_factor = monitor.scale_factor();
                let logical_size = size.to_logical::<f64>(scale_factor);
                let _ = window.set_size(tauri::Size::Logical(logical_size));
                let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(0.0, 0.0)));
            }
        }
    }

    // Set opacity if provided
    if let Some(alpha) = opacity {
        unsafe {
            let _: () = tauri_nspanel::objc2::msg_send![panel.as_panel(), setAlphaValue: alpha];
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn configure_overlay_panel(window: &WebviewWindow) -> Result<(), String> {
    use tauri_nspanel::objc2::rc::Retained;
    use tauri_nspanel::objc2_app_kit::{NSAutoresizingMaskOptions, NSView, NSTrackingArea, NSTrackingAreaOptions};
    use tauri_nspanel::objc2_foundation::NSRect;
    use tauri_nspanel::objc2::ClassType;

    apply_vibrancy(window, NSVisualEffectMaterial::HudWindow, None, None)
        .map_err(|error| error.to_string())?;
    let panel = window.to_panel::<OverlayPanel>().map_err(|error| error.to_string())?;
    panel.set_level(4);
    // HUD style meant to be translucent but on Ventura don't see it (maybe later OS)
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
    panel.set_corner_radius(OVERLAY_CORNER_RADIUS);

    // 1. Manually add tracking area with panel (window) as the owner & InVisibleRect to track resizing
    unsafe {
        let content_view: Retained<NSView> = tauri_nspanel::objc2::msg_send![panel.as_panel(), contentView];
        let bounds: NSRect = tauri_nspanel::objc2::msg_send![&content_view, bounds];
        
        let tracking_options = NSTrackingAreaOptions::ActiveAlways
            | NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::MouseMoved
            | NSTrackingAreaOptions::InVisibleRect; // Automatically resizes with window!
            
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

    // 2. Create custom panel event handler for mouse tracking events (macOS only)
    let handler = OverlayPanelEventHandler::new();
    let app_clone = window.app_handle().clone();
    let app_clone_moved = app_clone.clone();

    use std::cell::Cell;
    use std::time::{Duration, Instant};

    let last_emitted_time = Cell::new(None::<Instant>);
    let last_emitted_pos = Cell::new(None::<(f64, f64)>);

    handler.on_mouse_moved(move |event| {
        let location = event.locationInWindow();
        let now = Instant::now();

        let should_emit = match (last_emitted_time.get(), last_emitted_pos.get()) {
            (Some(last_time), Some(last_pos)) => {
                let elapsed = now.duration_since(last_time);
                let dx = (location.x - last_pos.0).abs();
                let dy = (location.y - last_pos.1).abs();

                elapsed >= Duration::from_millis(100) && (dx + dy) >= 6.0
            }
            _ => true, // Emit the very first event immediately
        };

        if should_emit {
            last_emitted_time.set(Some(now));
            last_emitted_pos.set(Some((location.x, location.y)));
            if let Some(overlay_window) = app_clone_moved.get_webview_window(OVERLAY_WINDOW) {
                let _ = overlay_window.emit("overlay-mouse", (location.x as i32, location.y as i32));
            }
        }
    });

    let app_clone_entered = app_clone.clone();
    handler.on_mouse_entered(move |_event| {
        println!("Rust: Mouse Entered Overlay Panel!");
        if let Some(overlay_window) = app_clone_entered.get_webview_window(OVERLAY_WINDOW) {
            let _ = overlay_window.emit("overlay-entered", ());
        }
    });
    let app_clone_exited = app_clone.clone();
    handler.on_mouse_exited(move |_event| {
        println!("Rust: Mouse Exited Overlay Panel!");
        if let Some(overlay_window) = app_clone_exited.get_webview_window(OVERLAY_WINDOW) {
            let _ = overlay_window.emit("overlay-exited", ());
        }
    });
    // 3. Persist layout dynamically when panel is moved/resized (macOS delegate events).
    // Note: Since tauri-nspanel replaces Tauri's default delegate, standard Tauri Moved/Resized
    // events do not fire on macOS. We hook these natively via AppKit delegates here.
    // Windows/Linux fall back to the standard tauri::WindowEvent listeners in window_state::watch_window.
    let app_clone_did_move = app_clone.clone();
    let seq_move = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    handler.window_did_move(move |_notification| {
        let current_seq = seq_move.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let app_clone = app_clone_did_move.clone();
        let seq_clone = seq_move.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if seq_clone.load(std::sync::atomic::Ordering::Relaxed) == current_seq {
                crate::window_state::persist_window(&app_clone, OVERLAY_WINDOW);
            }
        });
    });

    let app_clone_did_resize = app_clone.clone();
    let seq_resize = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    handler.window_did_resize(move |_notification| {
        let current_seq = seq_resize.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let app_clone = app_clone_did_resize.clone();
        let seq_clone = seq_resize.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if seq_clone.load(std::sync::atomic::Ordering::Relaxed) == current_seq {
                crate::window_state::persist_window(&app_clone, OVERLAY_WINDOW);
            }
        });
    });

    panel.set_event_handler(Some(handler.as_ref()));

    Ok(())
}



#[cfg(target_os = "macos")]
fn overlay_panel(app: &AppHandle) -> Result<Arc<dyn Panel>, String> {
    app.get_webview_panel(OVERLAY_WINDOW)
        .map_err(|_| format!("Missing panel config: {OVERLAY_WINDOW}"))
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn apply_window_options(window: &WebviewWindow, options: WindowOptions) -> Result<(), String> {
    if options.width.is_some() || options.height.is_some() {
        let width = options.width.unwrap_or(200.0);
        let height = options.height.unwrap_or(70.0);
        window
            .set_size(Size::Logical(LogicalSize::new(width, height)))
            .map_err(|error| error.to_string())?;
    }

    if options.x.is_some() || options.y.is_some() {
        let x = options.x.unwrap_or(100.0);
        let y = options.y.unwrap_or(100.0);
        window
            .set_position(Position::Logical(LogicalPosition::new(x, y)))
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn apply_window_options(_window: &WebviewWindow, _options: WindowOptions) -> Result<(), String> {
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn show_and_focus_window(window: &WebviewWindow) -> Result<(), String> {
    window.show().map_err(|error| error.to_string())?;
    
    if let Ok(physical_size) = window.inner_size() {
        let scale_factor = window.scale_factor().unwrap_or(1.0);
        let logical_width = (physical_size.width as f64) / scale_factor;
        let logical_height = (physical_size.height as f64) / scale_factor;
        let physical_pos = window.inner_position().ok();
        let logical_x = physical_pos.map(|p| p.x as f64 / scale_factor).unwrap_or(0.0);
        let logical_y = physical_pos.map(|p| p.y as f64 / scale_factor).unwrap_or(0.0);
        
        eprintln!(
            "[WINDOW-DEBUG] Label: '{}' | ScaleFactor: {} | PhysicalSize: {}x{} | LogicalSize: {:.1}x{:.1} | LogicalPos: {:.1}x{:.1}",
            window.label(),
            scale_factor,
            physical_size.width,
            physical_size.height,
            logical_width,
            logical_height,
            logical_x,
            logical_y
        );
    }

    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn show_and_focus_window(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

/// Show the overlay window. Windows mirrors the macOS NSPanel contract and
/// never activates (see `show_window_no_activate`); Linux keeps the previous
/// show-and-focus behaviour.
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn show_overlay_window(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        show_window_no_activate(window)
    }
    #[cfg(target_os = "linux")]
    {
        show_and_focus_window(window)
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn hide_window(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = window.window_handle() {
            if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                let hwnd = win32_handle.hwnd.get() as *mut std::ffi::c_void;
                unsafe {
                    SetWindowPos(
                        hwnd,
                        HWND_NOTOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_HIDEWINDOW,
                    );
                }
            }
        }
    }

    window.hide().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn hide_window(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn resize_window(window: &WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    window
        .set_size(Size::Logical(LogicalSize::new(width, height)))
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn resize_window(_window: &WebviewWindow, _width: f64, _height: f64) -> Result<(), String> {
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn start_drag_window(window: &WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn start_drag_window(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

fn show_main_window(app: &AppHandle) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        show_and_focus_window(&window)?;
        return Ok(true);
    }

    Ok(false)
}

#[cfg(any(debug_assertions, feature = "devtools"))]
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn toggle_main_devtools(app: &AppHandle) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        show_and_focus_window(&window)?;
        if window.is_devtools_open() {
            window.close_devtools();
        } else {
            window.open_devtools();
        }
        return Ok(true);
    }

    Ok(false)
}

#[cfg(any(debug_assertions, feature = "devtools"))]
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn toggle_main_devtools(_app: &AppHandle) -> Result<bool, String> {
    Ok(false)
}

fn emit_to_main<T: serde::Serialize + Clone>(
    app: &AppHandle,
    event_name: &str,
    payload: T,
) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        window
            .emit(event_name, payload)
            .map_err(|error| error.to_string())?;
        return Ok(true);
    }

    Ok(false)
}

fn toggle_overlay_window(app: &AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        let _window = ensure_window(app, OVERLAY_WINDOW)?;
        let panel = overlay_panel(app)?;
        if panel.is_visible() {
            panel.hide();
            crate::window_state::set_window_visibility_state(app, OVERLAY_WINDOW, false);
        } else {
            panel.show();
            crate::window_state::set_window_visibility_state(app, OVERLAY_WINDOW, true);
        }
        return Ok(true);
    }

    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            return Ok(false);
        }

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            let window = ensure_window(app, OVERLAY_WINDOW)?;
            if window.is_visible().unwrap_or(false) {
                hide_window(&window)?;
                crate::window_state::set_window_visibility_state(app, OVERLAY_WINDOW, false);
            } else {
                show_overlay_window(&window)?;
                crate::window_state::set_window_visibility_state(app, OVERLAY_WINDOW, true);
            }
            return Ok(true);
        }
    }
}

fn media_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| error.to_string())
        .map(|path| path.join("media"))
}

fn download_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().download_dir().map_err(|error| error.to_string())
}

fn temp_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().temp_dir().map_err(|error| error.to_string())
}

fn show_media_files(app: &AppHandle) -> Result<String, String> {
    let path = media_root(app)?;
    fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    open_directory(&path)?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn open_directory(path: &Path) -> Result<(), String> {
    let mut command = if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        command.arg(path);
        command
    } else if cfg!(target_os = "windows") {
        let mut command = Command::new("explorer");
        command.arg(path);
        command
    } else {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };

    let status = command.status().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Failed to open media folder (exit code {:?})",
            status.code()
        ))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn open_directory(_path: &Path) -> Result<(), String> {
    Err("Opening the media folder is not supported on this platform".to_string())
}

fn sanitize_relative_path(relative_path: &str) -> PathBuf {
    let mut safe = PathBuf::new();
    for component in PathBuf::from(relative_path).components() {
        if let Component::Normal(part) = component {
            safe.push(part);
        }
    }
    safe
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn resolve_media_path(app: &AppHandle, path: &str) -> Result<PathBuf, String> {
    let requested = PathBuf::from(path);
    if requested.is_absolute() {
        let resolved = normalize_path(&requested);
        let media = normalize_path(&media_root(app)?);
        let temp = normalize_path(&temp_root(app)?);
        let cache_root = app.path().app_cache_dir().map_err(|error| error.to_string())?;
        let cache = normalize_path(&cache_root);

        if resolved.starts_with(&media) || resolved.starts_with(&temp) || resolved.starts_with(&cache) {
            Ok(resolved)
        } else {
            Err(format!(
                "native_media_path: absolute path is outside allowed roots. requested='{}' media_root='{}' temp_root='{}' cache_root='{}'",
                requested.display(),
                media.display(),
                temp.display(),
                cache.display()
            ))
        }
    } else {
        Ok(media_root(app)?.join(sanitize_relative_path(path)))
    }
}

fn sanitize_file_name(file_name: &str) -> String {
    let candidate = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Document.pdf");

    let cleaned: String = candidate
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect();

    if cleaned.is_empty() {
        "Document.pdf".to_string()
    } else {
        cleaned
    }
}

fn collect_media_entries(root: &Path, current: &Path, entries: &mut Vec<String>) -> Result<(), String> {
    let read_dir = match fs::read_dir(current) {
        Ok(read_dir) => read_dir,
        Err(error) => {
            entries.push(format!(
                "[error reading {}: {}]",
                current
                    .strip_prefix(root)
                    .unwrap_or(current)
                    .display(),
                error
            ));
            return Ok(());
        }
    };

    for dir_entry in read_dir {
        let dir_entry = dir_entry.map_err(|error| error.to_string())?;
        let path = dir_entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let metadata = dir_entry.metadata().map_err(|error| error.to_string())?;

        if metadata.is_dir() {
            entries.push(format!("{}/", relative.display()));
            collect_media_entries(root, &path, entries)?;
        } else {
            entries.push(format!("{} ({} bytes)", relative.display(), metadata.len()));
        }
    }

    Ok(())
}

fn media_debug_info(app: &AppHandle) -> Result<MediaDebugInfo, String> {
    let root_path = media_root(app)?;
    let root_exists = root_path.exists();
    let mut entries = Vec::new();

    if root_exists {
        collect_media_entries(&root_path, &root_path, &mut entries)?;
        entries.sort();
    }

    Ok(MediaDebugInfo {
        media_root: root_path.to_string_lossy().into_owned(),
        root_exists,
        entries,
    })
}

// fn next_available_path(directory: &Path, file_name: &str) -> PathBuf {
//     let base_path = directory.join(file_name);
//     if !base_path.exists() {
//         return base_path;
//     }

//     let file_path = Path::new(file_name);
//     let stem = file_path
//         .file_stem()
//         .and_then(|value| value.to_str())
//         .unwrap_or("Document");
//     let extension = file_path.extension().and_then(|value| value.to_str());

//     for index in 2.. {
//         let candidate_name = match extension {
//             Some(ext) if !ext.is_empty() => format!("{stem} ({index}).{ext}"),
//             _ => format!("{stem} ({index})"),
//         };
//         let candidate_path = directory.join(candidate_name);
//         if !candidate_path.exists() {
//             return candidate_path;
//         }
//     }

//     unreachable!("download filename collision loop should always return")
// }

#[tauri::command]
pub fn native_main_show(app: AppHandle) -> Result<bool, String> {
    show_main_window(&app)
}

#[tauri::command]
pub fn native_overlay_show(app: AppHandle, options: Option<WindowOptions>) -> Result<bool, String> {
    eprintln!(
        "[OVERLAY-DBG] native_overlay_show() called from JS — creating/showing overlay window"
    );
    #[cfg(target_os = "macos")]
    {
        let window = ensure_window(&app, OVERLAY_WINDOW)?;
        if let Some(options) = options {
            apply_window_options(&window, options)?;
        }
        overlay_panel(&app)?.show();
        crate::window_state::set_window_visibility_state(&app, OVERLAY_WINDOW, true);
        return Ok(true);
    }

    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            return Ok(false);
        }

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            let window = ensure_window(&app, OVERLAY_WINDOW)?;
            if let Some(options) = options {
                apply_window_options(&window, options)?;
            }
            show_overlay_window(&window)?;
            crate::window_state::set_window_visibility_state(&app, OVERLAY_WINDOW, true);
            return Ok(true);
        }
    }
}

#[tauri::command]
pub fn native_overlay_hide(app: AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    if let Ok(panel) = overlay_panel(&app) {
        panel.hide();
        crate::window_state::set_window_visibility_state(&app, OVERLAY_WINDOW, false);
        return Ok(true);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        return Ok(false);
    }

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    if let Some(window) = app.get_webview_window(OVERLAY_WINDOW) {
        hide_window(&window)?;
        crate::window_state::set_window_visibility_state(&app, OVERLAY_WINDOW, false);
        return Ok(true);
    }

    Ok(false)
}

#[tauri::command]
pub fn native_overlay_toggle(app: AppHandle) -> Result<bool, String> {
    toggle_overlay_window(&app)
}

#[tauri::command]
pub fn native_overlay_set_opacity(app: AppHandle, alpha: f64) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(panel) = overlay_panel(&app) {
            unsafe {
                let _: () = tauri_nspanel::objc2::msg_send![panel.as_panel(), setAlphaValue: alpha];
            }
            return Ok(());
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(window) = app.get_webview_window(OVERLAY_WINDOW) {
            set_window_opacity(&window, alpha)?;
            return Ok(());
        }
    }

    #[cfg(target_os = "linux")]
    {
        let _ = alpha;
        if let Some(_window) = app.get_webview_window(OVERLAY_WINDOW) {
            return Ok(());
        }
    }

    Err("Failed to find overlay window/panel".to_string())
}

#[tauri::command]
pub fn native_overlay_resize(app: AppHandle, width: f64, height: f64) -> Result<bool, String> {
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        return Ok(false);
    }

    let window = ensure_window(&app, OVERLAY_WINDOW)?;
    resize_window(&window, width, height)?;
    Ok(true)
}

#[tauri::command]
pub fn native_overlay_start_drag(app: AppHandle) -> Result<bool, String> {
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        return Ok(false);
    }

    let window = ensure_window(&app, OVERLAY_WINDOW)?;
    start_drag_window(&window)?;
    Ok(true)
}

#[tauri::command]
pub fn native_overlay_send(app: AppHandle, data: Value) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window(OVERLAY_WINDOW) {
        window
            .emit(MAIN_TO_OVERLAY_EVENT, data)
            .map_err(|error| error.to_string())?;
        return Ok(true);
    }

    Ok(false)
}

#[tauri::command]
pub fn native_overlay_emit(app: AppHandle, data: Value) -> Result<bool, String> {
    emit_to_main(&app, OVERLAY_TO_MAIN_EVENT, data)
}

#[tauri::command]
pub fn native_media_write(
    app: AppHandle,
    relative_path: String,
    buffer: Vec<u8>,
) -> Result<bool, String> {
    let full_path = media_root(&app)?.join(sanitize_relative_path(&relative_path));
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(full_path, buffer).map_err(|error| error.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn native_media_read(app: AppHandle, relative_path: String) -> Result<Option<Vec<u8>>, String> {
    let root_path = media_root(&app)?;
    let full_path = resolve_media_path(&app, &relative_path)?;
    let safe_relative_path = if Path::new(&relative_path).is_absolute() {
        PathBuf::from(&relative_path)
    } else {
        sanitize_relative_path(&relative_path)
    };
    let parent_exists = full_path.parent().map(|path| path.exists()).unwrap_or(false);

    match fs::read(&full_path) {
        Ok(data) => Ok(Some(data)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "native_media_read: file not found. relative_path='{}' sanitized='{}' media_root='{}' full_path='{}' root_exists={} parent_exists={} file_exists={}",
            relative_path,
            safe_relative_path.display(),
            root_path.display(),
            full_path.display(),
            root_path.exists(),
            parent_exists,
            full_path.exists()
        )),
        Err(error) => Err(format!(
            "native_media_read: failed to read file. relative_path='{}' sanitized='{}' media_root='{}' full_path='{}' root_exists={} parent_exists={} file_exists={} error={}",
            relative_path,
            safe_relative_path.display(),
            root_path.display(),
            full_path.display(),
            root_path.exists(),
            parent_exists,
            full_path.exists(),
            error
        )),
    }
}

#[tauri::command]
pub fn native_media_delete(app: AppHandle, relative_path: String) -> Result<bool, String> {
    let full_path = resolve_media_path(&app, &relative_path)?;
    match fs::remove_file(full_path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub fn native_media_root(app: AppHandle) -> Result<String, String> {
    media_root(&app).map(|path| path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn native_media_debug(app: AppHandle) -> Result<MediaDebugInfo, String> {
    media_debug_info(&app)
}

#[tauri::command]
pub fn native_media_show_files(app: AppHandle) -> Result<String, String> {
    show_media_files(&app)
}

#[tauri::command]
pub async fn native_downloads_write(
    app: AppHandle,
    file_name: String,
    buffer: Vec<u8>,
) -> Result<String, String> {
    let safe_name = sanitize_file_name(&file_name);
    let suggested_name = Path::new(&safe_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Document");
    let mut dialog = app
        .dialog()
        .file()
        .add_filter("PDF", &["pdf"])
        .set_file_name(suggested_name);

    if let Ok(downloads_dir) = download_root(&app) {
        dialog = dialog.set_directory(downloads_dir);
    }

    let (tx, rx) = sync_channel(1);
    dialog.save_file(move |target_file| {
        let _ = tx.send(target_file);
    });

    let target_file = tauri::async_runtime::spawn_blocking(move || rx.recv())
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;

    let Some(target_file) = target_file else {
        return Ok(String::new());
    };

    let mut target_path = target_file
        .into_path()
        .map_err(|error| error.to_string())?;

    if target_path.extension().is_none() {
        target_path.set_extension("pdf");
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    fs::write(&target_path, buffer).map_err(|error| error.to_string())?;
    Ok(target_path.to_string_lossy().into_owned())
}

// ─── Media Pick ──────────────────────────────────────────────────────────────

/// Uniform result returned by all native media-pick commands.
/// JS converts `data` to a `Blob` and feeds it into the existing `addFile()` flow.
#[derive(Serialize)]
pub struct MediaPickResult {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub ext: String,
    pub name: String,
}

/// Map a lowercase file extension to its canonical MIME type.
fn mime_from_ext(ext: &str) -> String {
    match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png"          => "image/png",
        "webp"         => "image/webp",
        "gif"          => "image/gif",
        "heic" | "heif"=> "image/heic",
        "bmp"          => "image/bmp",
        "pdf"          => "application/pdf",
        "mp3"          => "audio/mpeg",
        "m4a"          => "audio/mp4",
        "aac"          => "audio/aac",
        "wav"          => "audio/wav",
        "ogg"          => "audio/ogg",
        "mp4"          => "video/mp4",
        "mov"          => "video/quicktime",
        "avi"          => "video/x-msvideo",
        _              => "application/octet-stream",
    }
    .to_string()
}

/// Open the native file / document picker and return the selected file's bytes.
/// Returns `None` if the user cancels.
/// Uses `tauri-plugin-dialog` on all platforms (already registered in lib.rs).
#[tauri::command]
pub async fn native_pick_file(
    app: AppHandle,
    allowed_extensions: Vec<String>,
) -> Result<Option<MediaPickResult>, String> {
    let ext_refs: Vec<&str> = allowed_extensions.iter().map(|s| s.as_str()).collect();

    // Build dialog with optional extension filter
    let mut dialog = app.dialog().file();
    if !ext_refs.is_empty() {
        dialog = dialog.add_filter("Files", &ext_refs);
    }

    // blocking_pick_file runs the OS dialog synchronously on the dialog thread
    let file_path = dialog.blocking_pick_file();
    let Some(file_path) = file_path else {
        return Ok(None); // cancelled
    };

    let path = file_path.into_path().map_err(|e| e.to_string())?;
    let data = fs::read(&path).map_err(|e| e.to_string())?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let mime_type = mime_from_ext(&ext);

    Ok(Some(MediaPickResult { data, mime_type, ext, name }))
}

/// Capture / pick a photo for attachment.
/// - Mobile (iOS + Android): uses `tauri-plugin-dialog` to open the system photo library picker.
///   This works on both iOS and Android without needing a camera plugin.
///   The native dialog on iOS presents PHPickerViewController (photo library + camera option).
/// - Desktop (mac/win/linux): uses `crabcamera` webcam capture → staging dir → bytes.
#[tauri::command]
pub async fn native_take_picture(app: AppHandle) -> Result<MediaPickResult, String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let name = format!("photo_{ts}.jpg");

    // Mobile (iOS + Android) — photo library picker via tauri-plugin-dialog
    // On iOS this opens PHPickerViewController; no extra camera plugin required.
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        let image_exts = ["jpg", "jpeg", "png", "webp", "gif", "heic", "heif", "bmp", "avif"];
        let file_path = app
            .dialog()
            .file()
            .add_filter("Images", &image_exts)
            .blocking_pick_file();
        let Some(file_path) = file_path else {
            return Err("cancelled".to_string());
        };
        let path = file_path.into_path().map_err(|e| e.to_string())?;
        let data = fs::read(&path).map_err(|e| e.to_string())?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg")
            .to_lowercase();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&name)
            .to_string();
        let mime_type = mime_from_ext(&ext);
        return Ok(MediaPickResult { data, mime_type, ext, name: file_name });
    }

    // Desktop — crabcamera webcam capture
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        use crabcamera::commands::capture::{capture_single_photo, save_frame_compressed};

        let staging = share_staging_dir(&app)?.join(&name);
        fs::create_dir_all(staging.parent().unwrap()).map_err(|e| e.to_string())?;
        let frame = capture_single_photo(None, None).await?;
        save_frame_compressed(frame, staging.to_string_lossy().to_string(), Some(85)).await?;
        let data = fs::read(&staging).map_err(|e| e.to_string())?;
        return Ok(MediaPickResult {
            data,
            mime_type: "image/jpeg".into(),
            ext: "jpg".into(),
            name,
        });
    }

    #[allow(unreachable_code)]
    Err("Photo capture is not supported on this platform".to_string())
}

// ─── Share ───────────────────────────────────────────────────────────────────


/// Staging directory for annotated PDF exports (only used when annotations exist).
/// Uses cache_dir which is stable for the duration of UIActivityViewController on iOS.

fn share_staging_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_cache_dir()
        .or_else(|_| app.path().temp_dir())
        .map_err(|error| error.to_string())?;
    Ok(base.join("share"))
}

/// Share a PDF via the native share sheet.
///
/// - `relative_path`: path relative to media_root (e.g. `"mFileId.pdf"`).
/// - `share_filename`: display name for the share sheet (e.g. `"My_Document.pdf"`),
///   computed by the JS side from the document title. Always used as the staged filename.
/// - `buffer`: if `Some`, an annotated copy — written to staging.
///             if `None`, the source file is copied to staging under share_filename.
#[tauri::command]
pub async fn native_downloads_share(
    app: AppHandle,
    relative_path: String,
    share_filename: String,
    buffer: Option<Vec<u8>>,
) -> Result<String, String> {
    let staging_dir = share_staging_dir(&app)?;
    fs::create_dir_all(&staging_dir).map_err(|error| error.to_string())?;

    // Always stage under the pretty share_filename — overwrite any prior export.
    let safe_name = sanitize_file_name(&share_filename);
    let target_path = staging_dir.join(&safe_name);

    if let Some(buf) = buffer {
        // Annotated copy: write the exported buffer.
        fs::write(&target_path, &buf).map_err(|error| error.to_string())?;
    } else {
        // Source file: copy to staging under the display name (zero-alloc on JS side).
        let source_path = media_root(&app)?.join(sanitize_relative_path(&relative_path));
        if !source_path.exists() {
            return Err(format!(
                "native_downloads_share: source file not found at '{}'",
                source_path.display()
            ));
        }
        fs::copy(&source_path, &target_path).map_err(|error| error.to_string())?;
    }

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "ios", target_os = "android"))]
    {
        app.share()
            .share_file(
                target_path.to_string_lossy().into_owned(),
                ShareFileOptions {
                    mime_type: Some("application/pdf".to_string()),
                    title: Some(safe_name),
                },
            )
            .map_err(|error| error.to_string())?;
        return Ok(target_path.to_string_lossy().into_owned());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "ios", target_os = "android")))]
    {
        Err("Native PDF sharing is not supported on this platform".to_string())
    }
}




#[tauri::command]
pub fn native_tray_update(app: AppHandle, timer_info: TrayTimerInfo) -> Result<bool, String> {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let tooltip = if let Some(pomo) = timer_info.pomo_timer {
                match pomo.status.as_deref() {
                    Some("focus") | Some("break") => format!(
                        "ARC - Pomo {} {}",
                        pomo.status.unwrap_or_default(),
                        pomo.text.unwrap_or_default()
                    )
                    .trim()
                    .to_string(),
                    _ if timer_info.start_at.unwrap_or(0) > 0 => format!(
                        "ARC - Timer {}",
                        timer_info
                            .elapsed_time
                            .unwrap_or_else(|| "running".to_string())
                    ),
                    _ => "ARC - Time Focus".to_string(),
                }
            } else if timer_info.start_at.unwrap_or(0) > 0 {
                let prefix = if timer_info.paused_at.unwrap_or(0) > 0 {
                    "paused"
                } else {
                    "running"
                };
                let elapsed = timer_info
                    .elapsed_time
                    .unwrap_or_else(|| "00:00".to_string());
                format!("ARC - Timer {prefix} {elapsed}")
            } else {
                "ARC - Time Focus".to_string()
            };

            tray.set_tooltip(Some(tooltip))
                .map_err(|error| error.to_string())?;
            return Ok(true);
        }
    }

    Ok(false)
}

#[tauri::command]
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn native_active_window() -> Result<Option<NativeActiveWindowInfo>, String> {
    let active = match get_active_window() {
        Ok(active) => active,
        Err(_) => return Ok(None),
    };

    Ok(Some(NativeActiveWindowInfo {
        app: active.app_name,
        title: active.title,
        process_path: active.process_path.to_string_lossy().to_string(),
        process_id: active.process_id.to_string(),
        window_id: active.window_id.to_string(),
        x: active.position.x,
        y: active.position.y,
        width: active.position.width,
        height: active.position.height,
    }))
}

#[tauri::command]
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn native_active_window() -> Result<Option<NativeActiveWindowInfo>, String> {
    Ok(None)
}

// =============================================================================
// Database commands — thin wrappers over db.rs helpers
// =============================================================================
use crate::db::DbState;
use tauri::State;
use std::time::Instant;

/// Generic SQL execution for statements that don't return rows.
#[tauri::command]
pub fn db_execute(
    state: State<DbState>,
    sql: String,
    params: Vec<Value>,
) -> Result<usize, String> {
    let total_start = Instant::now();
    let lock_start = Instant::now();
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let conn = &guard.0;
    eprintln!("[DB-IPC] db_execute - mutex_lock: {:?}", lock_start.elapsed());
    let result = crate::db::db_execute(conn, &sql, &params);
    eprintln!("[DB-IPC] db_execute - TOTAL: {:?}", total_start.elapsed());
    result
}

/// Generic SQL execution for statements that return rows (SELECT).
#[tauri::command]
pub fn db_query(
    state: State<DbState>,
    sql: String,
    params: Vec<Value>,
) -> Result<Value, String> {
    let total_start = Instant::now();
    let lock_start = Instant::now();
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let conn = &guard.0;
    eprintln!("[DB-IPC] db_query - mutex_lock: {:?}", lock_start.elapsed());
    let result = crate::db::db_query(conn, &sql, &params);
    eprintln!("[DB-IPC] db_query - TOTAL: {:?}", total_start.elapsed());
    result
}

/// Execute a batch of SQL statements.
#[tauri::command]
pub fn db_execute_batch(
    state: State<DbState>,
    sql: String,
) -> Result<(), String> {
    let total_start = Instant::now();
    let lock_start = Instant::now();
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let conn = &guard.0;
    eprintln!("[DB-IPC] db_execute_batch - mutex_lock: {:?}", lock_start.elapsed());
    let result = crate::db::db_execute_batch(conn, &sql);
    eprintln!("[DB-IPC] db_execute_batch - TOTAL: {:?}", total_start.elapsed());
    result
}

/// Execute multiple SELECT queries in a single IPC round-trip.
#[tauri::command]
pub fn db_batch_query(
    state: State<DbState>,
    queries: Value,
) -> Result<Value, String> {
    let total_start = Instant::now();
    let lock_start = Instant::now();
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let conn = &guard.0;
    eprintln!("[DB-IPC] db_batch_query - mutex_lock: {:?}", lock_start.elapsed());
    let result = crate::db::db_batch_query(conn, &queries);
    eprintln!("[DB-IPC] db_batch_query - TOTAL: {:?}", total_start.elapsed());
    result
}

/// Switch SQLite database connections and handle optional offline-to-online migrations.
#[tauri::command]
pub fn db_switch(
    state: State<DbState>,
    app: tauri::AppHandle,
    to_entity_id: String,
) -> Result<(), String> {
    let start = Instant::now();
    let db_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&db_dir).map_err(|e| e.to_string())?;
    
    let target_db_path = if to_entity_id == "arc" {
        db_dir.join("arc.db")
    } else {
        db_dir.join(format!("db_{}.db", to_entity_id))
    };
    let offline_db_path = db_dir.join("arc.db");

    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    let current_entity_id = guard.1.clone();

    // If already correct entity, return early!
    if current_entity_id == to_entity_id {
        eprintln!("[DB-IPC] Database already switched to '{}', returning early in {:?}", to_entity_id, start.elapsed());
        return Ok(());
    }

    // Migrate offline data to newly logged in entity database
    if current_entity_id == "arc" && to_entity_id != "arc" && offline_db_path.exists() && !target_db_path.exists() {
        eprintln!("[DB-IPC] Migrating offline database arc.db to db_{}.db", to_entity_id);
        
        // Reassign Mutex connection to in-memory temporarily to unlock the file
        *guard = (rusqlite::Connection::open_in_memory().map_err(|e| e.to_string())?, "temp".to_string());
        
        // Copy and delete offline db file
        std::fs::copy(&offline_db_path, &target_db_path).map_err(|e| e.to_string())?;
        std::fs::remove_file(&offline_db_path).map_err(|e| e.to_string())?;
        eprintln!("[DB-IPC] Offline database migrated successfully.");
    }

    // Now open target connection
    let conn = crate::db::open_db_for_entity(&app, &to_entity_id)?;
    *guard = (conn, to_entity_id.clone());

    // Write new active entity to settings
    let _ = crate::settings::set_setting_value(&app, "activeEntity".to_string(), serde_json::json!(to_entity_id));

    eprintln!("[DB-IPC] Database switched from '{}' to '{}' successfully in {:?}", current_entity_id, to_entity_id, start.elapsed());
    Ok(())
}

fn get_app_base_url(app: &AppHandle) -> tauri::Url {
    if let Some(main_win) = app.get_webview_window("main") {
        if let Ok(url) = main_win.url() {
            if let Ok(base) = url.join("/") {
                return base;
            }
        }
    }
    tauri::Url::parse("tauri://localhost/").unwrap()
}

#[tauri::command]
pub fn native_fullscreen_show(app: AppHandle, url: String, screen_index: Option<i32>, opacity: Option<f64>) -> Result<bool, String> {
    let _ = screen_index;
    let _ = opacity;
    eprintln!("[FULLSCREEN-DBG] native_fullscreen_show command invoked with url='{}'", url);

    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let app_clone = app.clone();
        let url_clone = url.clone();
        app.run_on_main_thread(move || {
            let res = (|| -> Result<bool, String> {
                let window = ensure_window(&app_clone, "fullscreen")?;
                let _ = configure_fullscreen_panel(&window, screen_index, opacity);
                let base_url = get_app_base_url(&app_clone);
                let resolved_url = base_url.join(&url_clone).map_err(|e| e.to_string())?;
                window.navigate(resolved_url).map_err(|e| e.to_string())?;
                let panel = app_clone.get_webview_panel("fullscreen")
                    .map_err(|_| "Missing panel config: fullscreen".to_string())?;
                panel.show();
                Ok(true)
            })();
            let _ = tx.send(res);
        }).map_err(|e| e.to_string())?;
        return rx.recv().map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            return Ok(false);
        }

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            eprintln!("[FULLSCREEN-DBG] [Win/Linux] Ensuring 'fullscreen' window exists...");
            let window = ensure_window(&app, "fullscreen")?;
            eprintln!("[FULLSCREEN-DBG] 'fullscreen' window obtained successfully.");

            let base_url = get_app_base_url(&app);
            let resolved_url = base_url.join(&url).map_err(|e| {
                eprintln!("[FULLSCREEN-DBG] Failed to resolve URL: {}", e);
                e.to_string()
            })?;
            eprintln!("[FULLSCREEN-DBG] Navigating 'fullscreen' window to: {}", resolved_url);
            window.navigate(resolved_url.clone()).map_err(|e| {
                eprintln!("[FULLSCREEN-DBG] window.navigate failed: {}", e);
                e.to_string()
            })?;

            #[cfg(target_os = "windows")]
            {
                eprintln!("[FULLSCREEN-DBG] Sizing & showing fullscreen window via Win32 SetWindowPos...");
                show_fullscreen_window_win32(&window)?;
            }

            #[cfg(not(target_os = "windows"))]
            {
                let _ = window.set_always_on_top(true);
                let _ = window.set_fullscreen(true);
                show_and_focus_window(&window)?;
            }

            eprintln!("[FULLSCREEN-DBG] native_fullscreen_show succeeded!");
            Ok(true)
        }
    }
}

#[tauri::command]
pub fn native_fullscreen_hide(app: AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let app_clone = app.clone();
        app.run_on_main_thread(move || {
            let res = (|| -> Result<bool, String> {
                let mut hidden = false;
                if let Ok(panel) = app_clone.get_webview_panel("fullscreen") {
                    panel.hide();
                    hidden = true;
                }
                if let Some(window) = app_clone.get_webview_window("fullscreen") {
                    hide_window(&window)?;
                    hidden = true;
                }
                Ok(hidden)
            })();
            let _ = tx.send(res);
        }).map_err(|e| e.to_string())?;
        return rx.recv().map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        if let Some(window) = app.get_webview_window("fullscreen") {
            hide_window(&window)?;
            return Ok(true);
        }

        Ok(false)
    }
}

#[tauri::command]
pub fn native_fullscreen_set_opacity(app: AppHandle, alpha: f64) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(panel) = app.get_webview_panel("fullscreen") {
            unsafe {
                let _: () = tauri_nspanel::objc2::msg_send![panel.as_panel(), setAlphaValue: alpha];
            }
            return Ok(());
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(window) = app.get_webview_window("fullscreen") {
            set_window_opacity(&window, alpha)?;
            return Ok(());
        }
    }

    #[cfg(target_os = "linux")]
    {
        let _ = alpha;
        if let Some(_window) = app.get_webview_window("fullscreen") {
            return Ok(());
        }
    }

    Err("Failed to find fullscreen window/panel".to_string())
}

#[cfg(target_os = "windows")]
extern "system" {
    fn GetWindowLongPtrW(hWnd: *mut std::ffi::c_void, nIndex: i32) -> isize;
    fn SetWindowLongPtrW(hWnd: *mut std::ffi::c_void, nIndex: i32, dwNewLong: isize) -> isize;
    fn GetSystemMetrics(nIndex: i32) -> i32;
    fn SetWindowPos(
        hwnd: *mut std::ffi::c_void,
        hwnd_insert_after: isize,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}

#[cfg(target_os = "windows")]
const GWL_EXSTYLE: i32 = -20;
#[cfg(target_os = "windows")]
const WS_EX_NOACTIVATE: isize = 0x0800_0000;
#[cfg(target_os = "windows")]
const HWND_TOPMOST: isize = -1;
#[cfg(target_os = "windows")]
const HWND_NOTOPMOST: isize = -2;
#[cfg(target_os = "windows")]
const SWP_NOSIZE: u32 = 0x0001;
#[cfg(target_os = "windows")]
const SWP_NOMOVE: u32 = 0x0002;
#[cfg(target_os = "windows")]
const SWP_NOACTIVATE: u32 = 0x0010;
#[cfg(target_os = "windows")]
const SWP_SHOWWINDOW: u32 = 0x0040;
#[cfg(target_os = "windows")]
const SWP_HIDEWINDOW: u32 = 0x0080;

/// Sizes and displays the fullscreen overlay using the same Win32 architecture
/// as the overlay widget, covering 100% of the primary monitor while preserving
/// Webview2 backdrop opacity and topmost status.
#[cfg(target_os = "windows")]
fn show_fullscreen_window_win32(window: &WebviewWindow) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = window.window_handle().map_err(|e| e.to_string())?.as_raw();
    let RawWindowHandle::Win32(win32_handle) = handle else {
        return Err("Unsupported window handle type".to_string());
    };
    let hwnd = win32_handle.hwnd.get() as *mut std::ffi::c_void;

    let width = unsafe { GetSystemMetrics(0) };  // SM_CXSCREEN
    let height = unsafe { GetSystemMetrics(1) }; // SM_CYSCREEN

    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_NOACTIVATE);

        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            width,
            height,
            SWP_SHOWWINDOW,
        );
    }
    Ok(())
}

/// Win32 equivalent of the macOS `OverlayPanel` configuration. Adds
/// `WS_EX_NOACTIVATE` — the analogue of `NSWindowStyleMask::NonactivatingPanel`
/// — so the overlay keeps receiving mouse input but never becomes the
/// foreground window when clicked.
#[cfg(target_os = "windows")]
fn configure_overlay_window_win32(window: &WebviewWindow) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = window.window_handle().map_err(|e| e.to_string())?.as_raw();
    let RawWindowHandle::Win32(win32_handle) = handle else {
        return Err("Unsupported window handle type".to_string());
    };
    let hwnd = win32_handle.hwnd.get() as *mut std::ffi::c_void;
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_NOACTIVATE);
    }
    Ok(())
}

/// Show a topmost window without activating it — the Win32 analogue of
/// `NSPanel.orderFrontRegardless` for a non-activating panel.
///
/// `WebviewWindow::show()` maps to `ShowWindow(SW_SHOW)`, which activates the
/// window, and `set_focus()` can silently fail under Windows foreground-lock
/// rules when another app is foreground. `SetWindowPos(SWP_NOACTIVATE)` shows
/// the window and re-asserts `HWND_TOPMOST` (so it isn't left behind other
/// topmost windows) without stealing focus.
#[cfg(target_os = "windows")]
pub(crate) fn show_window_no_activate(window: &WebviewWindow) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = window.window_handle().map_err(|e| e.to_string())?.as_raw();
    let RawWindowHandle::Win32(win32_handle) = handle else {
        return Err("Unsupported window handle type".to_string());
    };
    let hwnd = win32_handle.hwnd.get() as *mut std::ffi::c_void;
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn set_window_opacity(_window: &WebviewWindow, _opacity: f64) -> Result<(), String> {
    // Note: On Windows Webview2, setting WS_EX_LAYERED via SetLayeredWindowAttributes
    // breaks DirectComposition hardware acceleration, causing Webview2 transparent windows
    // to render completely invisible. Window transparency is handled natively via Webview2
    // transparent backdrop ("transparent": true in tauri.conf.json) + CSS background.
    Ok(())
}


// ═══════════════════════════════════════════════════════════════════════════════
// MCP Streamable-HTTP Bridge
//
// These two commands implement a thin TCP ↔ Tauri-event bridge so that the
// MCP server logic can live entirely in the browser JS layer (mcp-server.js).
//
// Flow:
//   1. JS calls native_mcp_listen(port)  →  Rust binds 127.0.0.1:<port>
//   2. AI client sends HTTP POST /mcp    →  Rust reads body, emits "mcp:request"
//   3. JS handles the event, calls api-calls, emits "mcp:response"
//   4. Rust receives "mcp:response"      →  writes HTTP response to client
//
// The Rust side is intentionally dumb: it only bridges raw bytes.
// ═══════════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

/// Global state: active listener shutdown sender + pending response channels.
pub struct McpStateInner {
    pub shutdown_tx: Option<std::sync::mpsc::SyncSender<()>>,
    /// requestId → channel to send the HTTP response body back to the request thread
    pub pending: HashMap<String, std::sync::mpsc::SyncSender<(u16, String)>>,
}

impl McpStateInner {
    pub fn new() -> Self {
        McpStateInner { shutdown_tx: None, pending: HashMap::new() }
    }
}

/// App-managed state type
pub struct McpBridgeState(pub Mutex<McpStateInner>);

/// Read a minimal HTTP/1.1 request from the stream.
/// Returns (method, path, body) or an error string.
fn read_http_request(stream: &mut TcpStream) -> Result<(String, String, String), String> {
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut first_line = String::new();
    reader.read_line(&mut first_line).map_err(|e| e.to_string())?;
    let parts: Vec<&str> = first_line.trim().splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Err("Malformed request line".to_string());
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();

    // Read headers
    let mut content_length: usize = 0;
    let mut _origin = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        let line = line.trim();
        if line.is_empty() { break; }
        let lower = line.to_lowercase();
        if lower.starts_with("content-length:") {
            content_length = lower["content-length:".len()..].trim().parse().unwrap_or(0);
        } else if lower.starts_with("origin:") {
            _origin = line["origin:".len()..].trim().to_string();
        }
    }

    // Read body
    let mut body = String::new();
    if content_length > 0 {
        let mut buf = vec![0u8; content_length];
        reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
        body = String::from_utf8_lossy(&buf).to_string();
    }

    Ok((method, path, body))
}

/// Write an HTTP/1.1 response.
fn write_http_response(stream: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK", 202 => "Accepted", 400 => "Bad Request",
        404 => "Not Found", 405 => "Method Not Allowed",
        500 => "Internal Server Error", _ => "OK",
    };
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type, Authorization\r\nAccess-Control-Allow-Methods: POST, GET, OPTIONS\r\nConnection: close\r\n\r\n{body}",
        status = status, reason = reason, len = body.len(), body = body
    );
    let _ = stream.write_all(resp.as_bytes());
}

#[tauri::command]
pub fn native_mcp_listen(
    app: AppHandle,
    port: u16,
    state: tauri::State<McpBridgeState>,
) -> Result<serde_json::Value, String> {
    let mut mcp = state.0.lock().map_err(|e| e.to_string())?;

    // Already running — return success without re-binding
    if mcp.shutdown_tx.is_some() {
        return Ok(serde_json::json!({ "ok": true, "port": port, "already": true }));
    }

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)
        .map_err(|e| format!("MCP bind failed on {addr}: {e}"))?;

    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::sync_channel::<()>(1);
    mcp.shutdown_tx = Some(shutdown_tx);

    let state_arc: Arc<Mutex<McpStateInner>> = Arc::new(Mutex::new(McpStateInner::new()));
    // Share state between accept loop and response listener
    let state_for_accept = Arc::clone(&state_arc);
    let state_for_response = Arc::clone(&state_arc);
    let app_clone = app.clone();

    // ── Accept loop ───────────────────────────────────────────────────────────
    thread::spawn(move || {
        listener.set_nonblocking(true).ok();
        loop {
            // Check shutdown signal
            if shutdown_rx.try_recv().is_ok() { break; }

            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    let app2 = app_clone.clone();
                    let st = Arc::clone(&state_for_accept);
                    thread::spawn(move || {
                        stream.set_nonblocking(false).ok();
                        let req = match read_http_request(&mut stream) {
                            Ok(r) => r,
                            Err(e) => {
                                write_http_response(&mut stream, 400, &format!("{{\"error\":\"{e}\"}}"));
                                return;
                            }
                        };
                        let (method, path, body) = req;

                        // CORS preflight
                        if method == "OPTIONS" {
                            write_http_response(&mut stream, 200, "");
                            return;
                        }

                        // Health check
                        if path == "/health" || path == "/v1/health" {
                            write_http_response(&mut stream, 200, "{\"status\":\"ok\"}");
                            return;
                        }

                        // Only accept POST to /mcp
                        if method != "POST" || (path != "/mcp" && path != "/v1/mcp") {
                            write_http_response(&mut stream, 404, "{\"error\":\"not found\"}");
                            return;
                        }

                        // Register a one-shot response channel
                        let request_id = format!("{}-{}", std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_nanos()).unwrap_or(0),
                            rand_u32());
                        let (resp_tx, resp_rx) = std::sync::mpsc::sync_channel::<(u16, String)>(1);
                        {
                            let mut s = st.lock().unwrap();
                            s.pending.insert(request_id.clone(), resp_tx);
                        }

                        // Emit to JS webview
                        let payload = serde_json::json!({
                            "requestId": request_id,
                            "body": body,
                        });
                        let _ = app2.emit("mcp:request", payload);

                        // Wait for JS response (10 s timeout)
                        match resp_rx.recv_timeout(std::time::Duration::from_secs(10)) {
                            Ok((status, resp_body)) => {
                                write_http_response(&mut stream, status, &resp_body);
                            }
                            Err(_) => {
                                write_http_response(&mut stream, 500,
                                    "{\"error\":\"webview response timeout\"}");
                            }
                        }
                        // Clean up
                        let mut s = st.lock().unwrap();
                        s.pending.remove(&request_id);
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No connection ready — sleep briefly
                    thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    // ── Response listener — receives "mcp:response" events from JS ────────────
    let app_for_resp = app.clone();
    thread::spawn(move || {
        // Poll via a channel: JS emits the event back to us.
        // We re-use listenNative pattern: subscribe via app handle event.
        let _handle = app_for_resp.listen("mcp:response", move |event| {
            let payload: serde_json::Value = match event.payload().parse() {
                Ok(v) => v, Err(_) => return,
            };
            let request_id = payload["requestId"].as_str().unwrap_or("").to_string();
            let body = payload["body"].as_str().unwrap_or("").to_string();
            let status = payload["status"].as_u64().unwrap_or(200) as u16;
            let mut s = state_for_response.lock().unwrap();
            if let Some(tx) = s.pending.remove(&request_id) {
                let _ = tx.send((status, body));
            }
        });
        // Keep thread alive (handle keeps subscription alive)
        loop { thread::sleep(std::time::Duration::from_secs(3600)); }
    });

    Ok(serde_json::json!({ "ok": true, "port": port }))
}

#[tauri::command]
pub fn native_mcp_stop(state: tauri::State<McpBridgeState>) -> Result<(), String> {
    let mut mcp = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(tx) = mcp.shutdown_tx.take() {
        let _ = tx.send(());
    }
    mcp.pending.clear();
    Ok(())
}

/// Simple non-crypto random u32 from system time nanoseconds.
fn rand_u32() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ (d.as_secs() as u32))
        .unwrap_or(42)
}
