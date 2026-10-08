use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

/// Compact HUD size in logical points. Must stay in sync with
/// IDLE_HUD_WIDTH / IDLE_HUD_HEIGHT in app/src/lib/types.ts.
pub const IDLE_HUD_WIDTH: f64 = 104.0;
pub const IDLE_HUD_HEIGHT: f64 = 48.0;

/// Minimum time between window-state disk writes during drags/resizes.
const PERSIST_THROTTLE: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WindowBounds {
    pub width: f64,
    pub height: f64,
    /// Physical screen coordinates (see clamp_rect for why physical).
    pub x: Option<f64>,
    pub y: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppWindowState {
    pub mode: String, // "normal" | "minimalist"
    pub normal: WindowBounds,
    pub minimalist: WindowBounds,
}

impl Default for AppWindowState {
    fn default() -> Self {
        Self {
            mode: "minimalist".to_string(),
            normal: WindowBounds {
                width: 680.0,
                height: 560.0,
                x: Some(100.0),
                y: Some(100.0),
            },
            minimalist: WindowBounds {
                width: IDLE_HUD_WIDTH,
                height: IDLE_HUD_HEIGHT,
                x: Some(60.0),
                y: Some(100.0),
            },
        }
    }
}

/// A monitor rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Clamp a window (physical origin + physical size) so it lands fully inside
/// the monitor it belongs to.
///
/// All math is done in physical pixels: converting monitor bounds to logical
/// per-monitor (as opposed to the window's) scale factors gives wrong results
/// on mixed-DPI setups, which previously made the HUD snap to defaults.
pub fn clamp_rect(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    monitors: &[MonitorRect],
) -> Option<(i32, i32)> {
    for m in monitors {
        // Accept the monitor if the window origin is inside it, allowing the
        // origin to sit slightly off the top/left edge (partially dragged off).
        if x >= m.x - w / 2 && x < m.x + m.width && y >= m.y - h / 2 && y < m.y + m.height {
            let max_x = m.x + (m.width - w).max(0);
            let max_y = m.y + (m.height - h).max(0);
            return Some((x.clamp(m.x, max_x), y.clamp(m.y, max_y)));
        }
    }
    None
}

fn physical_monitors<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Vec<MonitorRect> {
    app.available_monitors()
        .map(|monitors| {
            monitors
                .iter()
                .map(|mon| MonitorRect {
                    x: mon.position().x,
                    y: mon.position().y,
                    width: mon.size().width as i32,
                    height: mon.size().height as i32,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn default_overlay_rect(monitors: &[MonitorRect]) -> (i32, i32) {
    monitors
        .first()
        .map(|m| (m.x + 60, m.y + 100))
        .unwrap_or((60, 100))
}

fn default_main_rect(monitors: &[MonitorRect]) -> (i32, i32) {
    monitors
        .first()
        .map(|m| (m.x + 100, m.y + 100))
        .unwrap_or((100, 100))
}

/// Clamp an overlay window position (physical px) against available monitors.
pub fn clamp_overlay_position<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    x: Option<f64>,
    y: Option<f64>,
) -> (f64, f64) {
    let monitors = physical_monitors(app);
    let scale = app.primary_monitor().ok().flatten().map(|m| m.scale_factor()).unwrap_or(1.0);
    let (w, h) = (
        (IDLE_HUD_WIDTH * scale) as i32,
        (IDLE_HUD_HEIGHT * scale) as i32,
    );
    if let (Some(x), Some(y)) = (x, y) {
        if let Some((cx, cy)) = clamp_rect(x as i32, y as i32, w, h, &monitors) {
            return (cx as f64, cy as f64);
        }
    }
    let (dx, dy) = default_overlay_rect(&monitors);
    (dx as f64, dy as f64)
}

/// Clamp the main (settings) window position (physical px).
pub fn clamp_main_position<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    x: Option<f64>,
    y: Option<f64>,
) -> (f64, f64) {
    let monitors = physical_monitors(app);
    if let (Some(x), Some(y)) = (x, y) {
        if let Some((cx, cy)) = clamp_rect(x as i32, y as i32, 680, 560, &monitors) {
            return (cx as f64, cy as f64);
        }
    }
    let (dx, dy) = default_main_rect(&monitors);
    (dx as f64, dy as f64)
}

pub fn get_config_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    if let Ok(app_dir) = app.path().app_data_dir() {
        return app_dir.join(".breaktime_window.json");
    }
    PathBuf::from(".breaktime_window.json")
}

#[cfg(target_os = "macos")]
pub fn setup_macos_overlay_spaces(overlay_win: &tauri::WebviewWindow) {
    if let Ok(ns_win_ptr) = overlay_win.ns_window() {
        let ns_win = ns_win_ptr as *mut tauri_nspanel::objc2::runtime::AnyObject;
        unsafe {
            let behavior: usize = (1 << 0) | (1 << 8) | (1 << 6);
            let _: () = tauri_nspanel::objc2::msg_send![ns_win, setCollectionBehavior: behavior];
        }
    }
}

// ---------------------------------------------------------------------------
// Cached window state with throttled, atomic persistence
// ---------------------------------------------------------------------------

pub struct CachedWindowState {
    pub state: AppWindowState,
    dirty: bool,
    last_persist: std::time::Instant,
}

/// Managed Tauri state holding the authoritative in-memory window state so
/// drag/resize events don't re-read the config file for every pixel.
pub struct WindowStateCache(pub Mutex<CachedWindowState>);

pub fn manage_window_state(app: &mut tauri::App, state: AppWindowState) {
    app.manage(WindowStateCache(Mutex::new(CachedWindowState {
        state,
        dirty: false,
        last_persist: std::time::Instant::now(),
    })));
}

/// Mutate the cached state and persist at most once per PERSIST_THROTTLE.
/// The trailing dirty state is flushed on app exit (see run() in lib.rs).
pub fn mutate_window_state<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    mutate: impl FnOnce(&mut AppWindowState),
) {
    let (snapshot, should_persist) = {
        let cache = app.state::<WindowStateCache>();
        let mut guard = cache.0.lock().expect("window state mutex poisoned");
        mutate(&mut guard.state);
        guard.dirty = true;
        let should = guard.last_persist.elapsed() >= PERSIST_THROTTLE;
        if should {
            guard.last_persist = std::time::Instant::now();
        }
        (guard.state.clone(), should)
    };
    if should_persist {
        let _ = persist_state_snapshot(app, &snapshot);
    }
}

/// Mutate the cached state and persist immediately (mode changes are rare).
pub fn mutate_window_state_now<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    mutate: impl FnOnce(&mut AppWindowState),
) -> AppWindowState {
    let snapshot = {
        let cache = app.state::<WindowStateCache>();
        let mut guard = cache.0.lock().expect("window state mutex poisoned");
        mutate(&mut guard.state);
        guard.last_persist = std::time::Instant::now();
        guard.state.clone()
    };
    let _ = persist_state_snapshot(app, &snapshot);
    snapshot
}

/// Flush any pending dirty state to disk (called on app exit).
pub fn flush_window_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let snapshot = {
        let cache = app.state::<WindowStateCache>();
        let mut guard = cache.0.lock().expect("window state mutex poisoned");
        if !guard.dirty {
            return;
        }
        guard.last_persist = std::time::Instant::now();
        guard.state.clone()
    };
    let _ = persist_state_snapshot(app, &snapshot);
}

fn persist_state_snapshot<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &AppWindowState,
) -> Result<(), String> {
    let path = get_config_path(app);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    crate::settings::atomic_write(&path, &json)
}

#[tauri::command]
pub fn get_window_state(app: tauri::AppHandle) -> Result<AppWindowState, String> {
    let cache = app.state::<WindowStateCache>();
    let guard = cache.0.lock().map_err(|e| e.to_string())?;
    Ok(guard.state.clone())
}

#[tauri::command]
pub fn save_window_state(app: tauri::AppHandle, state: AppWindowState) -> Result<(), String> {
    mutate_window_state_now(&app, |cached| *cached = state.clone());
    Ok(())
}

#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    flush_window_state(&app);
    app.exit(0);
}

#[tauri::command]
pub fn enter_overlay_mode(app: tauri::AppHandle) -> Result<AppWindowState, String> {
    let snapshot = mutate_window_state_now(&app, |state| {
        // 1. Capture current main window position & size, then hide it
        if let Some(main_win) = app.get_webview_window("main") {
            if let (Ok(phys_pos), Ok(phys_size)) = (main_win.outer_position(), main_win.outer_size()) {
                let scale = main_win.scale_factor().unwrap_or(1.0);
                let logical_w = phys_size.width as f64 / scale;
                let logical_h = phys_size.height as f64 / scale;
                if logical_w > 400.0 && logical_h > 300.0 {
                    state.normal = WindowBounds {
                        width: logical_w,
                        height: logical_h,
                        x: Some(phys_pos.x as f64),
                        y: Some(phys_pos.y as f64),
                    };
                }
            }
            let _ = main_win.hide();
        }

        state.mode = "minimalist".to_string();

        // 2. Show overlay window at clamped minimalist position
        if let Some(overlay_win) = app.get_webview_window("overlay") {
            state.minimalist.width = IDLE_HUD_WIDTH;
            state.minimalist.height = IDLE_HUD_HEIGHT;
            let _ = overlay_win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                IDLE_HUD_WIDTH,
                IDLE_HUD_HEIGHT,
            )));

            let (clamped_x, clamped_y) =
                clamp_overlay_position(&app, state.minimalist.x, state.minimalist.y);
            state.minimalist.x = Some(clamped_x);
            state.minimalist.y = Some(clamped_y);
            let _ = overlay_win.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                clamped_x as i32,
                clamped_y as i32,
            )));

            #[cfg(target_os = "macos")]
            setup_macos_overlay_spaces(&overlay_win);

            let _ = overlay_win.set_always_on_top(true);
            let _ = crate::native::show_overlay_panel(&app);
        }
    });
    Ok(snapshot)
}

#[tauri::command]
pub fn exit_overlay_mode(app: tauri::AppHandle) -> Result<AppWindowState, String> {
    let snapshot = mutate_window_state_now(&app, |state| {
        if let Some(overlay_win) = app.get_webview_window("overlay") {
            if let Ok(phys_pos) = overlay_win.outer_position() {
                state.minimalist.x = Some(phys_pos.x as f64);
                state.minimalist.y = Some(phys_pos.y as f64);
            }
            state.minimalist.width = IDLE_HUD_WIDTH;
            state.minimalist.height = IDLE_HUD_HEIGHT;
            let _ = crate::native::hide_overlay_panel(&app);
        }

        state.mode = "normal".to_string();

        if let Some(main_win) = app.get_webview_window("main") {
            let target = state.normal.clone();
            let _ = main_win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                target.width,
                target.height,
            )));
            let (clamped_x, clamped_y) = clamp_main_position(&app, target.x, target.y);
            state.normal.x = Some(clamped_x);
            state.normal.y = Some(clamped_y);
            let _ = main_win.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                clamped_x as i32,
                clamped_y as i32,
            )));
            let _ = main_win.set_always_on_top(false);
            let _ = main_win.show();
            let _ = main_win.set_focus();
        }
    });
    Ok(snapshot)
}

#[tauri::command]
pub fn set_overlay_size(app: tauri::AppHandle, width: f64, height: f64) -> Result<(), String> {
    if let Some(overlay_win) = app.get_webview_window("overlay") {
        let safe_w = if width < IDLE_HUD_WIDTH { IDLE_HUD_WIDTH } else { width };
        let safe_h = if height < IDLE_HUD_HEIGHT { IDLE_HUD_HEIGHT } else { height };
        overlay_win
            .set_size(tauri::Size::Logical(tauri::LogicalSize::new(safe_w, safe_h)))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Initiate native window drag on the overlay panel (matching llm_gauge)
#[tauri::command]
pub fn native_overlay_start_drag(app: tauri::AppHandle) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window("overlay") {
        window.start_dragging().map_err(|e| e.to_string())?;
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITORS: &[MonitorRect] = &[
        MonitorRect { x: 0, y: 0, width: 2560, height: 1440 },
        MonitorRect { x: 2560, y: 180, width: 1920, height: 1080 },
    ];

    #[test]
    fn keeps_position_inside_monitor() {
        let (x, y) = clamp_rect(100, 200, 300, 56, MONITORS).unwrap();
        assert_eq!((x, y), (100, 200));
    }

    #[test]
    fn clamps_position_too_far_right() {
        // Origin inside monitor 0, but the HUD would extend past its edge
        let (x, y) = clamp_rect(2500, 100, 300, 56, MONITORS).unwrap();
        assert_eq!(x, 2560 - 300);
        assert_eq!(y, 100);
    }

    #[test]
    fn accepts_position_on_second_monitor() {
        // Mixed-DPI setups previously failed this by dividing each monitor's
        // bounds by its own scale factor before comparing.
        let (x, y) = clamp_rect(3000, 400, 300, 56, MONITORS).unwrap();
        assert_eq!((x, y), (3000, 400));
    }

    #[test]
    fn clamps_on_second_monitor() {
        let (x, _y) = clamp_rect(4300, 400, 300, 56, MONITORS).unwrap();
        assert_eq!(x, 2560 + 1920 - 300);
    }

    #[test]
    fn tolerates_slightly_offscreen_origin() {
        // Origin dragged a bit off the left edge still belongs to monitor 0
        let (x, y) = clamp_rect(-100, 100, 300, 56, MONITORS).unwrap();
        assert_eq!((x, y), (0, 100));
    }

    #[test]
    fn returns_none_when_not_on_any_monitor() {
        assert!(clamp_rect(9000, 9000, 300, 56, MONITORS).is_none());
    }
}
