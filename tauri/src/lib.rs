#![allow(unexpected_cfgs)]

use tauri::Manager;

pub mod native;
pub mod settings;
pub mod window_state;

pub use window_state::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    let builder = builder.plugin(tauri_plugin_dialog::init());
    let builder = builder.plugin(tauri_plugin_opener::init());
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_idlemonitor::init());

    builder
        .setup(|app| {
            let _ = native::setup_tray(app);
            native::setup_event_listeners(app);

            let mut state = {
                let path = get_config_path(&app.handle());
                if path.exists() {
                    std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|c| serde_json::from_str::<AppWindowState>(&c).ok())
                        .unwrap_or_default()
                } else {
                    AppWindowState::default()
                }
            };

            let main_win = app.get_webview_window("main");
            let overlay_win = app.get_webview_window("overlay");

            if let Some(ref overlay) = overlay_win {
                #[cfg(target_os = "macos")]
                {
                    let _ = native::configure_overlay_panel(overlay);
                    setup_macos_overlay_spaces(overlay);
                }
            }

            if state.mode == "normal" {
                if let Some(overlay) = overlay_win {
                    let _ = overlay.hide();
                }
                if let Some(main) = main_win {
                    let target = &state.normal;
                    let _ = main.set_size(tauri::Size::Logical(tauri::LogicalSize::new(target.width, target.height)));
                    let (clamped_x, clamped_y) = clamp_main_position(&app.handle(), target.x, target.y);
                    state.normal.x = Some(clamped_x);
                    state.normal.y = Some(clamped_y);
                    let _ = main.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(clamped_x, clamped_y)));
                    let _ = main.show();
                }
            } else {
                // Default mode: minimalist overlay
                if let Some(main) = main_win {
                    let _ = main.hide();
                }
                if let Some(overlay) = overlay_win {
                    state.minimalist.width = IDLE_HUD_WIDTH;
                    state.minimalist.height = IDLE_HUD_HEIGHT;
                    let _ = overlay.set_size(tauri::Size::Logical(tauri::LogicalSize::new(IDLE_HUD_WIDTH, IDLE_HUD_HEIGHT)));

                    let (clamped_x, clamped_y) = clamp_overlay_position(&app.handle(), state.minimalist.x, state.minimalist.y);
                    state.minimalist.x = Some(clamped_x);
                    state.minimalist.y = Some(clamped_y);
                    let _ = overlay.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(clamped_x, clamped_y)));
                    let _ = overlay.set_always_on_top(true);
                    let _ = overlay.show();
                    let _ = overlay.set_focus();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::Moved(phys_pos) => {
                    let app = window.app_handle();
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let logical_x = phys_pos.x as f64 / scale;
                    let logical_y = phys_pos.y as f64 / scale;

                    if let Ok(mut state) = get_window_state(app.clone()) {
                        if window.label() == "overlay" {
                            state.minimalist.x = Some(logical_x);
                            state.minimalist.y = Some(logical_y);
                        } else if window.label() == "main" {
                            state.normal.x = Some(logical_x);
                            state.normal.y = Some(logical_y);
                        }
                        let _ = save_window_state(app.clone(), state);
                    }
                }
                tauri::WindowEvent::Resized(phys_size) => {
                    let app = window.app_handle();
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let logical_w = phys_size.width as f64 / scale;
                    let logical_h = phys_size.height as f64 / scale;

                    if window.label() == "main" && logical_w > 400.0 && logical_h > 300.0 {
                        if let Ok(mut state) = get_window_state(app.clone()) {
                            state.normal.width = logical_w;
                            state.normal.height = logical_h;
                            let _ = save_window_state(app.clone(), state);
                        }
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            window_state::get_window_state,
            window_state::save_window_state,
            window_state::exit_app,
            window_state::enter_overlay_mode,
            window_state::exit_overlay_mode,
            window_state::sync_overlay_bounds,
            window_state::set_window_always_on_top,
            window_state::set_overlay_size,
            settings::get_setting,
            settings::set_setting,
        ])
        .build(tauri::generate_context!())
        .expect("error while running breaktime application")
        .run(|app_handle, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { has_visible_windows, .. } = event {
                if !has_visible_windows {
                    if let Some(overlay) = app_handle.get_webview_window("overlay") {
                        let _ = overlay.show();
                        let _ = overlay.set_focus();
                    } else if let Some(main) = app_handle.get_webview_window("main") {
                        let _ = main.show();
                        let _ = main.set_focus();
                    }
                }
            }
        });
}
