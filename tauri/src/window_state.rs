use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

pub const IDLE_HUD_WIDTH: f64 = 300.0;
pub const IDLE_HUD_HEIGHT: f64 = 56.0;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WindowBounds {
    pub width: f64,
    pub height: f64,
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
                x: Some(30.0),
                y: Some(100.0),
            },
        }
    }
}

pub fn get_config_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> PathBuf {
    if let Ok(app_dir) = app.path().app_data_dir() {
        return app_dir.join(".breaktime_window.json");
    }
    PathBuf::from(".breaktime_window.json")
}

pub fn clamp_overlay_position<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    target_x: Option<f64>,
    target_y: Option<f64>,
) -> (f64, f64) {
    if let (Some(x), Some(y)) = (target_x, target_y) {
        if let Ok(monitors) = app.available_monitors() {
            for mon in &monitors {
                let scale = mon.scale_factor();
                let mon_x = mon.position().x as f64 / scale;
                let mon_y = mon.position().y as f64 / scale;
                let mon_w = mon.size().width as f64 / scale;
                let mon_h = mon.size().height as f64 / scale;

                if x >= mon_x && x < (mon_x + mon_w - 60.0) && y >= mon_y && y < (mon_y + mon_h - 40.0) {
                    return (x, y);
                }
            }
            if let Some(first_mon) = monitors.first() {
                let scale = first_mon.scale_factor();
                let mon_x = first_mon.position().x as f64 / scale;
                let mon_y = first_mon.position().y as f64 / scale;
                return (mon_x + 30.0, mon_y + 100.0);
            }
        }
    }
    (30.0, 100.0)
}

pub fn clamp_main_position<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    target_x: Option<f64>,
    target_y: Option<f64>,
) -> (f64, f64) {
    if let (Some(x), Some(y)) = (target_x, target_y) {
        if let Ok(monitors) = app.available_monitors() {
            for mon in &monitors {
                let scale = mon.scale_factor();
                let mon_x = mon.position().x as f64 / scale;
                let mon_y = mon.position().y as f64 / scale;
                let mon_w = mon.size().width as f64 / scale;
                let mon_h = mon.size().height as f64 / scale;

                if x >= mon_x && x < (mon_x + mon_w - IDLE_HUD_WIDTH) && y >= mon_y && y < (mon_y + mon_h - 150.0) {
                    return (x, y);
                }
            }
            if let Some(first_mon) = monitors.first() {
                let scale = first_mon.scale_factor();
                let mon_x = first_mon.position().x as f64 / scale;
                let mon_y = first_mon.position().y as f64 / scale;
                return (mon_x + 100.0, mon_y + 100.0);
            }
        }
    }
    (100.0, 100.0)
}

#[cfg(target_os = "macos")]
pub fn setup_macos_overlay_spaces<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    if let Ok(ns_win_ptr) = window.ns_window() {
        let ns_win = ns_win_ptr as *mut tauri_nspanel::objc2::runtime::AnyObject;
        unsafe {
            let behavior: usize = (1 << 0) | (1 << 8) | (1 << 6);
            let _: () = tauri_nspanel::objc2::msg_send![ns_win, setCollectionBehavior: behavior];
        }
    }
}

#[tauri::command]
pub fn get_window_state(app: tauri::AppHandle) -> Result<AppWindowState, String> {
    let path = get_config_path(&app);
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(state) = serde_json::from_str::<AppWindowState>(&content) {
                return Ok(state);
            }
        }
    }
    Ok(AppWindowState::default())
}

#[tauri::command]
pub fn save_window_state(app: tauri::AppHandle, state: AppWindowState) -> Result<(), String> {
    let path = get_config_path(&app);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn enter_overlay_mode(app: tauri::AppHandle) -> Result<AppWindowState, String> {
    let mut state = get_window_state(app.clone()).unwrap_or_default();

    // 1. Capture current main window position & size, then hide it
    if let Some(main_win) = app.get_webview_window("main") {
        let scale = main_win.scale_factor().unwrap_or(1.0);
        if let (Ok(phys_pos), Ok(phys_size)) = (main_win.outer_position(), main_win.outer_size()) {
            let logical_w = phys_size.width as f64 / scale;
            let logical_h = phys_size.height as f64 / scale;
            if logical_w > 400.0 {
                state.normal = WindowBounds {
                    width: logical_w,
                    height: logical_h,
                    x: Some(phys_pos.x as f64 / scale),
                    y: Some(phys_pos.y as f64 / scale),
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
        let _ = overlay_win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(IDLE_HUD_WIDTH, IDLE_HUD_HEIGHT)));

        let (clamped_x, clamped_y) = clamp_overlay_position(&app, state.minimalist.x, state.minimalist.y);
        state.minimalist.x = Some(clamped_x);
        state.minimalist.y = Some(clamped_y);
        let _ = overlay_win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(clamped_x, clamped_y)));

        #[cfg(target_os = "macos")]
        setup_macos_overlay_spaces(&overlay_win);

        let _ = overlay_win.set_always_on_top(true);
        let _ = overlay_win.show();
        let _ = overlay_win.set_focus();
    }

    save_window_state(app, state.clone())?;
    Ok(state)
}

#[tauri::command]
pub fn exit_overlay_mode(app: tauri::AppHandle) -> Result<AppWindowState, String> {
    let mut state = get_window_state(app.clone()).unwrap_or_default();

    if let Some(overlay_win) = app.get_webview_window("overlay") {
        let scale = overlay_win.scale_factor().unwrap_or(1.0);
        if let Ok(phys_pos) = overlay_win.outer_position() {
            state.minimalist.x = Some(phys_pos.x as f64 / scale);
            state.minimalist.y = Some(phys_pos.y as f64 / scale);
        }
        state.minimalist.width = IDLE_HUD_WIDTH;
        state.minimalist.height = IDLE_HUD_HEIGHT;
        let _ = overlay_win.hide();
    }

    state.mode = "normal".to_string();

    if let Some(main_win) = app.get_webview_window("main") {
        let target = &state.normal;
        let _ = main_win.set_size(tauri::Size::Logical(tauri::LogicalSize::new(target.width, target.height)));
        let (clamped_x, clamped_y) = clamp_main_position(&app, target.x, target.y);
        state.normal.x = Some(clamped_x);
        state.normal.y = Some(clamped_y);
        let _ = main_win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(clamped_x, clamped_y)));
        let _ = main_win.set_always_on_top(false);
        let _ = main_win.show();
        let _ = main_win.set_focus();
    }

    save_window_state(app, state.clone())?;
    Ok(state)
}

#[tauri::command]
pub fn sync_overlay_bounds(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    let mut state = get_window_state(app.clone()).unwrap_or_default();
    let scale = window.scale_factor().unwrap_or(1.0);
    if let (Ok(phys_pos), Ok(phys_size)) = (window.outer_position(), window.outer_size()) {
        if phys_size.width > 0 && phys_size.height > 0 {
            let bounds = WindowBounds {
                width: phys_size.width as f64 / scale,
                height: phys_size.height as f64 / scale,
                x: Some(phys_pos.x as f64 / scale),
                y: Some(phys_pos.y as f64 / scale),
            };
            if window.label() == "overlay" {
                state.minimalist.width = IDLE_HUD_WIDTH;
                state.minimalist.height = IDLE_HUD_HEIGHT;
                state.minimalist.x = bounds.x;
                state.minimalist.y = bounds.y;
            } else {
                state.normal = bounds;
            }
            save_window_state(app, state)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_window_always_on_top(window: tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    window.set_always_on_top(enabled).map_err(|e| e.to_string())
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
