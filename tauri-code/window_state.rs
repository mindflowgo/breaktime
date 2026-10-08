//! Custom window state persistence — stores position/size per window label.
//!
//! Unlike `tauri-plugin-window-state`, this implementation:
//! - Does NOT clamp positions to primary-monitor bounds (multi-monitor safe)
//! - Stores coordinates in LOGICAL pixels to handle Retina/HiDPI correctly
//!   on both macOS (2x) and Windows (125%, 150% DPI) without double-scaling
//! - Restores window visibility
//!
//! State is saved to `window_positions.json` inside the app data directory.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Manager};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Stored in LOGICAL pixels (device-independent points).
/// On a 2x Retina display a 733-point wide window → physical 1466 px.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Whether the window was visible at save time.
    pub visible: bool,
}

/// Map from window label → geometry, stored as JSON.
type StateMap = HashMap<String, WindowGeometry>;

// ---------------------------------------------------------------------------
// File helpers
// ---------------------------------------------------------------------------

fn load_state_map(app: &AppHandle) -> StateMap {
    if let Some(val) = crate::settings::get_setting_value(app, "window_positions") {
        if let Ok(map) = serde_json::from_value(val) {
            return map;
        }
    }
    HashMap::new()
}

fn save_state_map(app: &AppHandle, map: &StateMap) {
    if let Ok(val) = serde_json::to_value(map) {
        let _ = crate::settings::set_setting_value(app, "window_positions".to_string(), val);
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Show a window being restored from saved state.
///
/// The overlay mirrors the macOS NSPanel contract and must never activate:
/// on Windows `window.show()` is `ShowWindow(SW_SHOW)`, which activates, so
/// route it through the no-activate `SetWindowPos` path instead.
fn show_restored(window: &tauri::WebviewWindow, label: &str) {
    let _ = label; // only used on Windows; avoids unused-variable warning elsewhere
    #[cfg(target_os = "windows")]
    if label == "overlay" {
        let _ = crate::native::show_window_no_activate(window);
        return;
    }
    let _ = window.show();
    let _ = window.set_focus();
}

/// Restore a window's position, size, and visibility from the persisted state.
///
/// Coordinates are applied as LOGICAL pixels so Retina/HiDPI scale factors
/// are not double-applied. No clamping to monitor bounds is performed, so
/// positions on secondary monitors (negative x/y) are preserved correctly.
///
/// If no saved state exists for this label this is a no-op (first launch uses
/// the defaults from tauri.conf.json).
pub fn restore_window(app: &AppHandle, label: &str) {
    let map = load_state_map(app);
    let Some(geo) = map.get(label) else {
        eprintln!(
            "[WINDOW-STATE] No saved state for '{}', showing with tauri.conf defaults.",
            label
        );
        // First launch: window starts hidden (visible: false in tauri.conf.json).
        // Show it now at the default position.
        if let Some(window) = app.get_webview_window(label) {
            show_restored(&window, label);
        }
        return;
    };

    let Some(window) = app.get_webview_window(label) else {
        eprintln!(
            "[WINDOW-STATE] Window '{}' not found during restore.",
            label
        );
        return;
    };

    eprintln!(
        "[WINDOW-STATE] Restoring '{}': {}x{} logical at ({}, {}) visible={}",
        label, geo.width, geo.height, geo.x, geo.y, geo.visible
    );

    // Use LOGICAL units — Tauri converts to physical internally using the
    // correct scale factor for the target monitor.
    use tauri::{LogicalPosition, LogicalSize, Position, Size};
    let _ = window.set_size(Size::Logical(LogicalSize::new(geo.width, geo.height)));

    // Verify if the saved position intersects with any currently available monitor.
    // If not (e.g. secondary monitor was disconnected), we skip set_position
    // so it falls back to the default visible position on the primary monitor.
    let mut is_visible_on_any_monitor = true;
    if let Ok(monitors) = window.available_monitors() {
        if !monitors.is_empty() {
            is_visible_on_any_monitor = false;
            let scale = window.scale_factor().unwrap_or(1.0);
            let phys_x = (geo.x * scale) as i32;
            let phys_y = (geo.y * scale) as i32;
            
            for m in monitors {
                let m_pos = m.position();
                let m_size = m.size();
                
                // Allow a safe 20px overlap margin
                let margin = 20;
                let inside_x = phys_x >= (m_pos.x - margin) && phys_x < (m_pos.x + m_size.width as i32 + margin);
                let inside_y = phys_y >= (m_pos.y - margin) && phys_y < (m_pos.y + m_size.height as i32 + margin);
                
                if inside_x && inside_y {
                    is_visible_on_any_monitor = true;
                    break;
                }
            }
        }
    }

    if is_visible_on_any_monitor {
        let _ = window.set_position(Position::Logical(LogicalPosition::new(geo.x, geo.y)));
    } else {
        eprintln!(
            "[WINDOW-STATE] Saved position ({}, {}) for '{}' is off-screen. Resetting to default.",
            geo.x, geo.y, label
        );
    }

    // Restore visibility. The main window may start hidden in tauri.conf.json
    // (to avoid a flash before the correct position is set).
    if geo.visible {
        show_restored(&window, label);
    }
}

/// Persist a window's current position, size, and visibility.
///
/// Coordinates are converted from physical to LOGICAL pixels so they round-trip
/// correctly on Retina/HiDPI displays. Silently no-ops if the window doesn't
/// exist or returns a zero-size (e.g. during window creation).
pub fn persist_window(app: &AppHandle, label: &str) {
    let Some(window) = app.get_webview_window(label) else { return };

    // scale_factor() gives us the DPI ratio (2.0 on Retina, 1.5 on 150% Windows)
    let scale = window.scale_factor().unwrap_or(1.0);

    let Ok(phys_pos) = window.outer_position() else { return };
    let Ok(phys_size) = window.outer_size() else { return };

    // Guard: skip saving a zero-size state that can occur during window creation.
    if phys_size.width == 0 || phys_size.height == 0 {
        return;
    }

    let visible = window.is_visible().unwrap_or(true);

    // Convert physical → logical
    let geo = WindowGeometry {
        x: phys_pos.x as f64 / scale,
        y: phys_pos.y as f64 / scale,
        width: phys_size.width as f64 / scale,
        height: phys_size.height as f64 / scale,
        visible,
    };

    eprintln!(
        "[WINDOW-STATE] Saving '{}': {}x{} logical at ({}, {}) scale={} visible={}",
        label, geo.width, geo.height, geo.x, geo.y, scale, geo.visible
    );

    let mut map = load_state_map(app);
    map.insert(label.to_string(), geo);
    save_state_map(app, &map);
}

// ---------------------------------------------------------------------------
// Event wiring helper
// ---------------------------------------------------------------------------

/// Attach move/resize/close/cursor listeners to a window so state is automatically
/// persisted AND so the overlay window emits mouse-tracking events that mirror the
/// macOS NSPanel `NSTrackingArea` contract.
///
/// On macOS, `tauri-nspanel` replaces the default window delegate with its own, so
/// `CursorMoved` / `CursorLeft` never reach this callback there — that's intentional.
/// The macOS path emits the same `overlay-entered` / `overlay-exited` / `overlay-mouse`
/// events from `native::configure_overlay_panel`. Windows + Linux go through this
/// Tauri's cross-platform window-event implementation (Win32 `WM_MOUSEMOVE` /
/// `WM_MOUSELEAVE` on Windows, X11/Wayland motion + leave on Linux).
///
/// `CursorMoved` positions are normalised to the macOS contract before emission:
/// LOGICAL pixels with a BOTTOM-LEFT origin (Cocoa `locationInWindow`). Raw Tauri
/// positions are physical pixels with a top-left origin, which would otherwise
/// break the frontend's hit-test math on HiDPI displays (125%/150% scaling).
///
/// Call once per window after creation.
pub fn watch_window(app: &AppHandle, label: &str) {
    let Some(window) = app.get_webview_window(label) else {
        eprintln!(
            "[OVERLAY-DBG] watch_window() called → label='{}' window NOT FOUND (will be re-attached when ensure_window creates it).",
            label
        );
        return;
    };
    eprintln!(
        "[OVERLAY-DBG] watch_window() called → label='{}' window FOUND, attaching on_window_event listener.",
        label
    );
    let label = label.to_string();
    let app_handle = app.clone();
    let seq = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));

    window.on_window_event(move |event| {
        match event {
            tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) => {
                // Increment sequence to invalidate any previously scheduled saves
                let current_seq = seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                let app_clone = app_handle.clone();
                let lbl = label.clone();
                let seq_clone = seq.clone();

                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    // Only save if no newer window events have incremented the counter
                    if seq_clone.load(std::sync::atomic::Ordering::Relaxed) == current_seq {
                        persist_window(&app_clone, &lbl);
                    }
                });
            }
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed => {
                persist_window(&app_handle, &label);
            }
            _ => {}
        }
    });
}

pub fn is_window_visible_in_saved_state(app: &AppHandle, label: &str) -> bool {
    let map = load_state_map(app);
    if let Some(geo) = map.get(label) {
        geo.visible
    } else {
        false
    }
}

pub fn set_window_visibility_state(app: &AppHandle, label: &str, visible: bool) {
    let mut map = load_state_map(app);
    if let Some(geo) = map.get_mut(label) {
        geo.visible = visible;
    } else {
        let (mut x, mut y, mut width, mut height) = (0.0, 0.0, 200.0, 70.0);
        if let Some(window) = app.get_webview_window(label) {
            let scale = window.scale_factor().unwrap_or(1.0);
            if let Ok(phys_pos) = window.outer_position() {
                x = phys_pos.x as f64 / scale;
                y = phys_pos.y as f64 / scale;
            }
            if let Ok(phys_size) = window.outer_size() {
                width = phys_size.width as f64 / scale;
                height = phys_size.height as f64 / scale;
            }
        }
        let geo = WindowGeometry {
            x,
            y,
            width,
            height,
            visible,
        };
        map.insert(label.to_string(), geo);
    }
    save_state_map(app, &map);
}
