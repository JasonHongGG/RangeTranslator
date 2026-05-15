#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod benchmark;
mod capture;
mod models;
mod sidecar;
mod state;

use serde_json::json;
use tauri::Manager;

use app::{commands, events::emit_debug, windows};
use state::SharedState;

const DEFAULT_ENDPOINT: &str =
    "https://lacresha-posological-steven.ngrok-free.dev";
const DEFAULT_MODEL: &str = "discovering";

fn main() {
    let state = SharedState::new(DEFAULT_ENDPOINT.to_string(), DEFAULT_MODEL.to_string());

    tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            if let Some(window) = app.get_webview_window("panel") {
                window.set_content_protected(true)?;
                window.set_always_on_top(true)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "selector" {
                match event {
                    tauri::WindowEvent::Focused(focused) => emit_debug(
                        &window.app_handle(),
                        "selector-window",
                        "focus changed",
                        json!({
                            "focused": focused,
                        }),
                    ),
                    tauri::WindowEvent::Moved(position) => emit_debug(
                        &window.app_handle(),
                        "selector-window",
                        "window moved",
                        json!({
                            "x": position.x,
                            "y": position.y,
                        }),
                    ),
                    tauri::WindowEvent::Resized(size) => emit_debug(
                        &window.app_handle(),
                        "selector-window",
                        "window resized",
                        json!({
                            "width": size.width,
                            "height": size.height,
                        }),
                    ),
                    tauri::WindowEvent::CloseRequested { .. } => emit_debug(
                        &window.app_handle(),
                        "selector-window",
                        "close requested",
                        json!(null),
                    ),
                    tauri::WindowEvent::Destroyed => emit_debug(
                        &window.app_handle(),
                        "selector-window",
                        "destroyed",
                        json!(null),
                    ),
                    _ => {}
                }
            }

            if window.label() == "panel" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if !windows::is_shutting_down() {
                        emit_debug(
                            &window.app_handle(),
                            "panel-window",
                            "panel close requested",
                            json!(null),
                        );

                        let state = window.app_handle().state::<SharedState>();
                        windows::request_shutdown(&window.app_handle(), state.inner_clone());
                        windows::schedule_app_exit(&window.app_handle());
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::clear_selection,
            commands::close_selector_window,
            commands::get_latest_translation,
            commands::get_runtime_capabilities,
            commands::get_runtime_snapshot,
            commands::open_selector_window,
            commands::panel_close,
            commands::panel_minimize,
            commands::run_prompt_benchmark,
            commands::toggle_panel_pin,
            commands::start_pipeline,
            commands::stop_pipeline,
            commands::submit_selection,
            commands::toggle_debug_screenshot_mode,
            commands::toggle_copy_mode,
            commands::update_overlay_selection,
        ])
        .run(tauri::generate_context!())
        .expect("tauri application failed")
}
