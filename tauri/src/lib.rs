#![allow(unexpected_cfgs)]

use tauri::Manager;

pub mod native;
pub mod settings;
pub mod sound;
pub mod window_state;

use window_state::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_dialog::init());
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_idlemonitor::init());

    builder
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            let _ = native::setup_tray(app);
            native::setup_event_listeners(app);

            let mut state = load_window_state(app.handle());

            let main_win = app.get_webview_window("main");
            let overlay_win = app.get_webview_window("overlay");

            // Manage the state cache BEFORE installing the panel delegate:
            // the macOS window_did_move callback mutates the cache, and a
            // set_position during setup can fire it synchronously.
            manage_window_state(app, state.clone());

            if let Some(ref _overlay) = overlay_win {
                #[cfg(target_os = "macos")]
                {
                    let _ = native::configure_overlay_panel(_overlay);
                    setup_macos_overlay_spaces(_overlay);
                }
            }

            if state.mode == "normal" {
                if let Some(_overlay) = overlay_win {
                    let _ = native::hide_overlay_panel(app.handle());
                }
                if let Some(main) = main_win {
                    let target = &state.normal;
                    let _ = main.set_size(tauri::Size::Logical(tauri::LogicalSize::new(target.width, target.height)));
                    let (clamped_x, clamped_y) =
                        clamp_main_position(app.handle(), target.x, target.y, target.width, target.height);
                    state.normal.x = Some(clamped_x);
                    state.normal.y = Some(clamped_y);
                    let _ = main.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                        clamped_x as i32,
                        clamped_y as i32,
                    )));
                    let _ = main.set_always_on_top(false);
                    let _ = main.show();
                    let _ = main.set_focus();
                }
            } else {
                // Default mode: minimalist overlay
                if let Some(main) = main_win {
                    let _ = main.hide();
                }
                if let Some(overlay) = overlay_win {
                    // Size for the persisted HUD scale (2× mode) so the
                    // doubled eye isn't clipped before the frontend loads.
                    let (idle_w, idle_h) = idle_hud_logical_size(app.handle());
                    state.minimalist.width = idle_w;
                    state.minimalist.height = idle_h;
                    let _ = overlay.set_size(tauri::Size::Logical(tauri::LogicalSize::new(idle_w, idle_h)));

                    let (clamped_x, clamped_y) = clamp_overlay_position(app.handle(), state.minimalist.x, state.minimalist.y);
                    state.minimalist.x = Some(clamped_x);
                    state.minimalist.y = Some(clamped_y);
                    let _ = overlay.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                        clamped_x as i32,
                        clamped_y as i32,
                    )));
                    let _ = overlay.set_always_on_top(true);
                    let _ = native::show_overlay_panel(app.handle());
                }
            }

            // Sync the post-positioning state (clamped coordinates) into the
            // managed cache; drag/resize events mutate it from here on.
            if app.try_state::<window_state::WindowStateCache>().is_some() {
                mutate_window_state_now(app.handle(), |cached| *cached = state.clone());
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    if window.label() == "main" {
                        api.prevent_close();
                        let _ = crate::window_state::enter_overlay_mode(window.app_handle().clone());
                    } else if window.label() == "overlay" {
                        api.prevent_close();
                        let _ = crate::native::hide_overlay_panel(window.app_handle());
                    }
                }
                tauri::WindowEvent::Moved(phys_pos) => {
                    // Store physical coordinates directly — mixed-DPI logical
                    // conversion is unreliable (see clamp_rect docs).
                    let app = window.app_handle();
                    let (x, y) = (phys_pos.x as f64, phys_pos.y as f64);

                    mutate_window_state(app, |state| {
                        if window.label() == "overlay" {
                            state.minimalist.x = Some(x);
                            state.minimalist.y = Some(y);
                        } else if window.label() == "main" {
                            state.normal.x = Some(x);
                            state.normal.y = Some(y);
                        }
                    });
                }
                tauri::WindowEvent::Resized(phys_size) => {
                    let app = window.app_handle();
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let logical_w = phys_size.width as f64 / scale;
                    let logical_h = phys_size.height as f64 / scale;

                    if window.label() == "main" && logical_w > 400.0 && logical_h > 300.0 {
                        mutate_window_state(app, |state| {
                            state.normal.width = logical_w;
                            state.normal.height = logical_h;
                        });
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            window_state::exit_app,
            window_state::enter_overlay_mode,
            window_state::exit_overlay_mode,
            window_state::set_overlay_size,
            window_state::native_overlay_start_drag,
            native::native_overlay_set_opacity,
            settings::get_setting,
            settings::set_setting,
            sound::import_custom_sound,
            sound::get_custom_sound_data,
            sound::cleanup_custom_sounds,
        ])
        .build(tauri::generate_context!())
        .expect("error while running breaktime application")
        .run(|app_handle, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { has_visible_windows, .. } = event {
                if !has_visible_windows {
                    let state = cached_window_state(app_handle);
                    if state.mode == "normal" {
                        if let Some(main) = app_handle.get_webview_window("main") {
                            let _ = main.show();
                            let _ = main.set_focus();
                        }
                    } else {
                        let _ = native::show_overlay_panel(app_handle);
                    }
                }
            }

            // Persist any window-state changes that were throttled out.
            if let tauri::RunEvent::Exit = event {
                flush_window_state(app_handle);
            }
        });
}
