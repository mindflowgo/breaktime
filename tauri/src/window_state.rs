use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// Compact HUD size in logical points. Must stay in sync with
/// IDLE_HUD_WIDTH / IDLE_HUD_HEIGHT in app/src/lib/services/timerManager.svelte.ts.
pub const IDLE_HUD_WIDTH: f64 = 104.0;
pub const IDLE_HUD_HEIGHT: f64 = 48.0;
/// Idle HUD size in 2× mode (hand-tuned tighter than a straight 2×; must
/// stay in sync with IDLE_HUD_2X_WIDTH / IDLE_HUD_2X_HEIGHT in timerManager).
pub const IDLE_HUD_2X_WIDTH: f64 = 149.0;
pub const IDLE_HUD_2X_HEIGHT: f64 = 69.0;

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

/// A monitor rectangle in physical pixels plus its scale factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy)]
struct MonitorInfo {
    rect: MonitorRect,
    scale: f64,
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

fn monitors_with_scale<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Vec<MonitorInfo> {
    app.available_monitors()
        .map(|monitors| {
            monitors
                .iter()
                .map(|mon| MonitorInfo {
                    rect: MonitorRect {
                        x: mon.position().x,
                        y: mon.position().y,
                        width: mon.size().width as i32,
                        height: mon.size().height as i32,
                    },
                    scale: mon.scale_factor(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn rects_of(monitors: &[MonitorInfo]) -> Vec<MonitorRect> {
    monitors.iter().map(|m| m.rect).collect()
}

/// Scale factor of the monitor containing the given physical point (fallback:
/// first monitor, then 1.0) — the correct scale for mixed-DPI layouts.
fn scale_at_point(monitors: &[MonitorInfo], x: i32, y: i32) -> f64 {
    for m in monitors {
        if x >= m.rect.x && x < m.rect.x + m.rect.width && y >= m.rect.y && y < m.rect.y + m.rect.height
        {
            return m.scale;
        }
    }
    monitors.first().map(|m| m.scale).unwrap_or(1.0)
}

fn default_overlay_rect(monitors: &[MonitorInfo]) -> (i32, i32) {
    monitors
        .first()
        .map(|m| (m.rect.x + 60, m.rect.y + 100))
        .unwrap_or((60, 100))
}

fn default_main_rect(monitors: &[MonitorInfo]) -> (i32, i32) {
    monitors
        .first()
        .map(|m| (m.rect.x + 100, m.rect.y + 100))
        .unwrap_or((100, 100))
}

/// Clamp an overlay window position (physical px). The HUD's physical size is
/// derived from the scale of the monitor the HUD actually sits on, times the
/// persisted HUD scale (2× mode).
pub fn clamp_overlay_position<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    x: Option<f64>,
    y: Option<f64>,
) -> (f64, f64) {
    let monitors = monitors_with_scale(app);
    let rects = rects_of(&monitors);
    let k = hud_scale(app);
    if let (Some(x), Some(y)) = (x, y) {
        let scale = scale_at_point(&monitors, x as i32, y as i32) * k;
        let (w, h) = (
            (IDLE_HUD_WIDTH * scale) as i32,
            (IDLE_HUD_HEIGHT * scale) as i32,
        );
        if let Some((cx, cy)) = clamp_rect(x as i32, y as i32, w, h, &rects) {
            return (cx as f64, cy as f64);
        }
    }
    let (dx, dy) = default_overlay_rect(&monitors);
    (dx as f64, dy as f64)
}

/// Clamp the main (settings) window position (physical px). The footprint
/// uses the window's *stored* size scaled by the containing monitor.
pub fn clamp_main_position<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    x: Option<f64>,
    y: Option<f64>,
    logical_w: f64,
    logical_h: f64,
) -> (f64, f64) {
    let monitors = monitors_with_scale(app);
    let rects = rects_of(&monitors);
    if let (Some(x), Some(y)) = (x, y) {
        let scale = scale_at_point(&monitors, x as i32, y as i32);
        let (w, h) = ((logical_w * scale) as i32, (logical_h * scale) as i32);
        if let Some((cx, cy)) = clamp_rect(x as i32, y as i32, w, h, &rects) {
            return (cx as f64, cy as f64);
        }
    }
    let (dx, dy) = default_main_rect(&monitors);
    (dx as f64, dy as f64)
}

/// Load the persisted window state from `.settings.json` (key `windowState`,
/// same get/set handler as settings and stats).
pub fn load_window_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> AppWindowState {
    crate::settings::get_setting_value(app, "windowState")
        .and_then(|v| serde_json::from_value::<AppWindowState>(v).ok())
        .unwrap_or_default()
}

/// HUD scale from the persisted `settings.hud2x` flag (frontend "2× Size"
/// menu toggle). Rust sizes the overlay window at startup / on mode switches
/// and must match the scale the frontend renders at, or the doubled eye gets
/// clipped by a 1× window.
pub fn hud_scale<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> f64 {
    let hud2x = crate::settings::get_setting_value(app, "settings")
        .and_then(|v| v.get("hud2x").and_then(|f| f.as_bool()));
    if hud2x == Some(true) {
        2.0
    } else {
        1.0
    }
}

/// Logical idle HUD size at the persisted scale.
pub fn idle_hud_logical_size<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> (f64, f64) {
    if hud_scale(app) >= 2.0 {
        (IDLE_HUD_2X_WIDTH, IDLE_HUD_2X_HEIGHT)
    } else {
        (IDLE_HUD_WIDTH, IDLE_HUD_HEIGHT)
    }
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
//
// Lock discipline: never hold the MutexGuard across window roundtrips
// (outer_position/set_position/…). On macOS the panel delegate fires
// window_did_move synchronously inside set_position, and off-main-thread
// window getters block on the event loop — holding the guard through either
// can deadlock. Mutators follow read → act (unlocked) → write-back.

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

/// Read a clone of the cached state (used by the dock-reopen handler).
pub fn cached_window_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> AppWindowState {
    match app.try_state::<WindowStateCache>() {
        Some(cache) => cache
            .0
            .lock()
            .map(|guard| guard.state.clone())
            .unwrap_or_default(),
        None => AppWindowState::default(),
    }
}

/// Mutate the cached state and persist at most once per PERSIST_THROTTLE.
/// The trailing dirty state is flushed on app exit (see run() in lib.rs).
pub fn mutate_window_state<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    mutate: impl FnOnce(&mut AppWindowState),
) {
    let (snapshot, should_persist) = {
        let Some(cache) = app.try_state::<WindowStateCache>() else { return };
        let mut guard = match cache.0.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        mutate(&mut guard.state);
        guard.dirty = true;
        let should = guard.last_persist.elapsed() >= PERSIST_THROTTLE;
        if should {
            guard.last_persist = std::time::Instant::now();
        }
        (guard.state.clone(), should)
    };
    if should_persist && persist_state_snapshot(app, &snapshot).is_ok() {
        clear_dirty(app);
    }
}

/// Mutate the cached state and persist immediately (mode changes are rare).
/// Returns the persisted snapshot.
pub fn mutate_window_state_now<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    mutate: impl FnOnce(&mut AppWindowState),
) -> AppWindowState {
    let snapshot = {
        let Some(cache) = app.try_state::<WindowStateCache>() else {
            // Cache not managed yet (early startup): run the mutation on a default.
            let mut state = AppWindowState::default();
            mutate(&mut state);
            return state;
        };
        let mut guard = match cache.0.lock() {
            Ok(g) => g,
            // Poisoned lock: fall back to a fresh default so a panicking
            // callback cannot brick window management entirely.
            Err(poisoned) => poisoned.into_inner(),
        };
        mutate(&mut guard.state);
        guard.last_persist = std::time::Instant::now();
        guard.dirty = true;
        guard.state.clone()
    };
    let _ = persist_state_snapshot(app, &snapshot);
    clear_dirty(app);
    snapshot
}

/// Write a full snapshot back into the cache and persist immediately
/// (used after unlocked window operations produced the final values).
fn commit_window_state<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: AppWindowState,
) -> AppWindowState {
    let cache = app.try_state::<WindowStateCache>();
    if let Some(cache) = cache {
        if let Ok(mut guard) = cache.0.lock() {
            guard.state = state.clone();
            guard.dirty = true;
            guard.last_persist = std::time::Instant::now();
        }
    }
    let _ = persist_state_snapshot(app, &state);
    clear_dirty(app);
    state
}

fn clear_dirty<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(cache) = app.try_state::<WindowStateCache>() {
        if let Ok(mut guard) = cache.0.lock() {
            guard.dirty = false;
        }
    }
}

/// Flush any pending dirty state to disk (called on app exit).
pub fn flush_window_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let snapshot = {
        let Some(cache) = app.try_state::<WindowStateCache>() else {
            return;
        };
        let mut guard = match cache.0.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        if !guard.dirty {
            return;
        }
        guard.last_persist = std::time::Instant::now();
        guard.state.clone()
    };
    if persist_state_snapshot(app, &snapshot).is_ok() {
        clear_dirty(app);
    }
}

fn persist_state_snapshot<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &AppWindowState,
) -> Result<(), String> {
    let value = serde_json::to_value(state).map_err(|e| e.to_string())?;
    crate::settings::set_setting_value(app, "windowState".to_string(), value)
}

#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    flush_window_state(&app);
    app.exit(0);
}

#[tauri::command]
pub fn enter_overlay_mode(app: tauri::AppHandle) -> Result<AppWindowState, String> {
    // Cheap re-entry: switching to overlay mode while already minimalist
    // (natural timer breaks, tray break-now) must not reset the HUD size or
    // position — a break may be expanding the window right after this.
    let (mut snapshot, already_min) = {
        let Some(cache) = app.try_state::<WindowStateCache>() else {
            return Ok(AppWindowState::default());
        };
        let Ok(guard) = cache.0.lock() else {
            return Ok(AppWindowState::default());
        };
        let already = guard.state.mode == "minimalist";
        (guard.state.clone(), already)
    };

    if already_min {
        // Ensure the panel is visible, nothing else.
        let _ = crate::native::show_overlay_panel(&app);
        let _ = app.emit("overlay-mode-changed", "minimalist");
        return Ok(snapshot);
    }

    // 1. Capture current main window position & size, then hide it (no lock held).
    if let Some(main_win) = app.get_webview_window("main") {
        if let (Ok(phys_pos), Ok(phys_size)) = (main_win.outer_position(), main_win.outer_size()) {
            let scale = main_win.scale_factor().unwrap_or(1.0);
            let logical_w = phys_size.width as f64 / scale;
            let logical_h = phys_size.height as f64 / scale;
            if logical_w > 400.0 && logical_h > 300.0 {
                snapshot.normal = WindowBounds {
                    width: logical_w,
                    height: logical_h,
                    x: Some(phys_pos.x as f64),
                    y: Some(phys_pos.y as f64),
                };
            }
        }
        let _ = main_win.hide();
    }

    snapshot.mode = "minimalist".to_string();

    // 2. Show overlay window at clamped minimalist position, sized for the
    //    persisted HUD scale (2× mode) so the doubled eye is never clipped.
    if let Some(overlay_win) = app.get_webview_window("overlay") {
        let (idle_w, idle_h) = idle_hud_logical_size(&app);
        snapshot.minimalist.width = idle_w;
        snapshot.minimalist.height = idle_h;
        let _ = overlay_win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
            idle_w, idle_h,
        )));

        let (clamped_x, clamped_y) =
            clamp_overlay_position(&app, snapshot.minimalist.x, snapshot.minimalist.y);
        snapshot.minimalist.x = Some(clamped_x);
        snapshot.minimalist.y = Some(clamped_y);
        let _ = overlay_win.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            clamped_x as i32,
            clamped_y as i32,
        )));

        #[cfg(target_os = "macos")]
        setup_macos_overlay_spaces(&overlay_win);

        let _ = overlay_win.set_always_on_top(true);
        let _ = crate::native::show_overlay_panel(&app);
    }

    let _ = app.emit("overlay-mode-changed", "minimalist");
    Ok(commit_window_state(&app, snapshot))
}

#[tauri::command]
pub fn exit_overlay_mode(app: tauri::AppHandle) -> Result<AppWindowState, String> {
    let (mut snapshot, already_normal) = {
        let Some(cache) = app.try_state::<WindowStateCache>() else {
            return Ok(AppWindowState::default());
        };
        let Ok(guard) = cache.0.lock() else {
            return Ok(AppWindowState::default());
        };
        let already = guard.state.mode == "normal";
        (guard.state.clone(), already)
    };

    if let Some(overlay_win) = app.get_webview_window("overlay") {
        if let Ok(phys_pos) = overlay_win.outer_position() {
            snapshot.minimalist.x = Some(phys_pos.x as f64);
            snapshot.minimalist.y = Some(phys_pos.y as f64);
        }
        let (idle_w, idle_h) = idle_hud_logical_size(&app);
        snapshot.minimalist.width = idle_w;
        snapshot.minimalist.height = idle_h;
        let _ = crate::native::hide_overlay_panel(&app);
    }

    snapshot.mode = "normal".to_string();

    if let Some(main_win) = app.get_webview_window("main") {
        let target = snapshot.normal.clone();
        let _ = main_win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
            target.width,
            target.height,
        )));
        let (clamped_x, clamped_y) =
            clamp_main_position(&app, target.x, target.y, target.width, target.height);
        snapshot.normal.x = Some(clamped_x);
        snapshot.normal.y = Some(clamped_y);
        let _ = main_win.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            clamped_x as i32,
            clamped_y as i32,
        )));
        let _ = main_win.set_always_on_top(false);
        if !already_normal {
            let _ = main_win.show();
            let _ = main_win.set_focus();
        }
    }

    let _ = app.emit("overlay-mode-changed", "normal");
    Ok(commit_window_state(&app, snapshot))
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
