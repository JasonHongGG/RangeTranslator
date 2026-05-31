use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Context;
use serde_json::json;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::{
    app::events::{emit_debug, emit_snapshot, emit_translation},
    capture::virtual_desktop_bounds,
    models::SelectionRect,
    state::SharedState,
};

const SELECTOR_INIT_SCRIPT: &str = r#"window.__RANGE_TRANSLATOR_VIEW__ = 'selector';"#;
static APP_SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

fn set_window_capture_protection(
    window: &WebviewWindow,
    content_protected: bool,
) -> Result<(), String> {
    window
        .set_content_protected(content_protected)
        .map_err(|error| error.to_string())
}

pub fn set_capture_protection(app: &AppHandle, content_protected: bool) -> Result<(), String> {
    for label in ["panel", "selector", "overlay"] {
        if let Some(window) = app.get_webview_window(label) {
            set_window_capture_protection(&window, content_protected)?;
        }
    }

    Ok(())
}

pub fn is_shutting_down() -> bool {
    APP_SHUTTING_DOWN.load(Ordering::SeqCst)
}

pub async fn open_selector_window(app: &AppHandle, state: SharedState) -> Result<(), String> {
    use tokio::sync::oneshot;

    let bounds = virtual_desktop_bounds().map_err(|error| error.to_string())?;
    let content_protected = !state.snapshot().debug_screenshot_mode;
    let debug_window_mode = cfg!(debug_assertions);
    let (window_x, window_y, window_width, window_height) =
        (bounds.x, bounds.y, bounds.width, bounds.height);
    let selector_bounds = SelectionRect {
        x: window_x,
        y: window_y,
        width: window_width,
        height: window_height,
    };

    emit_debug(
        app,
        "selector-backend",
        "open_selector_window invoked",
        json!({
            "desktopBounds": {
                "x": bounds.x,
                "y": bounds.y,
                "width": bounds.width,
                "height": bounds.height,
            },
            "windowBounds": {
                "x": window_x,
                "y": window_y,
                "width": window_width,
                "height": window_height,
            },
            "existingWindow": app.get_webview_window("selector").is_some(),
            "debugWindowMode": debug_window_mode,
        }),
    );

    let (tx, rx) = oneshot::channel();
    let app_handle = app.clone();
    app.run_on_main_thread(move || {
        let result: Result<(), String> = (|| {
            emit_debug(
                &app_handle,
                "selector-backend",
                "selector main-thread entry",
                json!({
                    "existingWindow": app_handle.get_webview_window("selector").is_some(),
                    "debugWindowMode": debug_window_mode,
                }),
            );

            if let Some(window) = app_handle.get_webview_window("selector") {
                set_window_capture_protection(&window, content_protected)?;
                window
                    .set_position(Position::Physical(PhysicalPosition::new(window_x, window_y)))
                    .map_err(|error| error.to_string())?;
                window
                    .set_size(Size::Physical(PhysicalSize::new(window_width, window_height)))
                    .map_err(|error| error.to_string())?;
                window
                    .set_always_on_top(true)
                    .map_err(|error| error.to_string())?;
                window
                    .set_ignore_cursor_events(false)
                    .map_err(|error| error.to_string())?;
                emit_debug(
                    &app_handle,
                    "selector-backend",
                    "reused selector window prepared",
                    json!({
                        "alwaysOnTop": true,
                        "ignoreCursorEvents": false,
                        "position": { "x": window_x, "y": window_y },
                        "size": { "width": window_width, "height": window_height },
                    }),
                );
                window.show().map_err(|error| error.to_string())?;
                window.set_focus().map_err(|error| error.to_string())?;

                emit_debug(
                    &app_handle,
                    "selector-backend",
                    "reused selector window",
                    json!({
                        "alwaysOnTop": true,
                        "ignoreCursorEvents": false,
                    }),
                );
                return Ok(());
            }

            let builder = WebviewWindowBuilder::new(
                &app_handle,
                "selector",
                WebviewUrl::App("index.html".into()),
            )
            .title(if debug_window_mode {
                "RangeTranslator selector [debug]"
            } else {
                "RangeTranslator selector"
            })
            .initialization_script(SELECTOR_INIT_SCRIPT)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .visible(false)
            .focused(true)
            .position(0.0, 0.0)
            .inner_size(120.0, 80.0);

            let window = match builder.build() {
                Ok(window) => window,
                Err(error) => {
                    let message = error.to_string();
                    emit_debug(
                        &app_handle,
                        "selector-backend",
                        "selector window build failed",
                        json!({
                            "error": message,
                            "position": { "x": window_x, "y": window_y },
                            "size": { "width": window_width, "height": window_height },
                            "debugWindowMode": debug_window_mode,
                        }),
                    );
                    return Err(message);
                }
            };

            emit_debug(
                &app_handle,
                "selector-backend",
                "selector window built",
                json!({
                    "position": { "x": window_x, "y": window_y },
                    "size": { "width": window_width, "height": window_height },
                    "debugWindowMode": debug_window_mode,
                }),
            );

            set_window_capture_protection(&window, content_protected)?;
            window
                .set_position(Position::Physical(PhysicalPosition::new(window_x, window_y)))
                .map_err(|error| error.to_string())?;
            window
                .set_size(Size::Physical(PhysicalSize::new(window_width, window_height)))
                .map_err(|error| error.to_string())?;
            window
                .set_ignore_cursor_events(false)
                .map_err(|error| error.to_string())?;
            window.show().map_err(|error| error.to_string())?;
            window.set_focus().map_err(|error| error.to_string())?;

            emit_debug(
                &app_handle,
                "selector-backend",
                "created selector window",
                json!({
                    "url": "index.html",
                    "transparent": true,
                    "alwaysOnTop": true,
                    "skipTaskbar": true,
                    "shadow": false,
                }),
            );

            Ok(())
        })();

        let _ = tx.send(result);
    })
    .map_err(|error| error.to_string())?;

    rx.await
        .map_err(|_| "selector main-thread callback dropped".to_string())??;

    let snapshot = state.activate_selector(selector_bounds.clone());
    emit_debug(
        app,
        "selector-backend",
        "selector mode activated",
        json!({
            "status": snapshot.status,
            "selection": snapshot.selection,
            "selectorBounds": selector_bounds,
        }),
    );
    emit_snapshot(app, &snapshot);
    Ok(())
}

pub fn close_selector_window(app: &AppHandle, state: SharedState) -> Result<(), String> {
    emit_debug(
        app,
        "selector-backend",
        "close_selector_window invoked",
        json!({
            "hasWindow": app.get_webview_window("selector").is_some(),
        }),
    );
    let snapshot = state.cancel_selector();
    emit_debug(
        app,
        "selector-backend",
        "selector mode cancelled",
        json!({
            "status": snapshot.status,
        }),
    );
    emit_snapshot(app, &snapshot);
    hide_window(app, "selector");
    schedule_window_close(app, "selector", 30);
    Ok(())
}

pub async fn ensure_overlay_window(
    app: &AppHandle,
    selection: &SelectionRect,
    interactive: bool,
    content_protected: bool,
) -> Result<(), String> {
    use tokio::sync::oneshot;

    let (tx, rx) = oneshot::channel();
    let app_handle = app.clone();
    let selection = selection.clone();

    app.run_on_main_thread(move || {
        let result: Result<(), String> = (|| {
            if let Some(window) = app_handle.get_webview_window("overlay") {
                set_window_capture_protection(&window, content_protected)?;
                window
                    .set_visible_on_all_workspaces(true)
                    .map_err(|error| error.to_string())?;
                window
                    .set_always_on_top(true)
                    .map_err(|error| error.to_string())?;
                window
                    .set_decorations(false)
                    .map_err(|error| error.to_string())?;
                window
                    .set_skip_taskbar(true)
                    .map_err(|error| error.to_string())?;
                window.set_shadow(false).map_err(|error| error.to_string())?;
                window
                    .set_resizable(true)
                    .map_err(|error| error.to_string())?;
                window
                    .set_position(Position::Physical(PhysicalPosition::new(
                        selection.x,
                        selection.y,
                    )))
                    .map_err(|error| error.to_string())?;
                window
                    .set_size(Size::Physical(PhysicalSize::new(
                        selection.width,
                        selection.height,
                    )))
                    .map_err(|error| error.to_string())?;
                window
                    .set_ignore_cursor_events(!interactive)
                    .map_err(|error| error.to_string())?;
                window.show().map_err(|error| error.to_string())?;
                if interactive {
                    window.set_focus().map_err(|error| error.to_string())?;
                }
                return Ok(());
            }

            let window = WebviewWindowBuilder::new(
                &app_handle,
                "overlay",
                WebviewUrl::App("index.html".into()),
            )
            .title("RangeTranslator overlay")
            .initialization_script(r#"window.__RANGE_TRANSLATOR_VIEW__ = 'overlay';"#)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(true)
            .shadow(false)
            .visible(false)
            .focused(interactive)
            .position(0.0, 0.0)
            .inner_size(120.0, 80.0)
            .build()
            .map_err(|error| error.to_string())?;

            set_window_capture_protection(&window, content_protected)?;
            window
                .set_position(Position::Physical(PhysicalPosition::new(
                    selection.x,
                    selection.y,
                )))
                .map_err(|error| error.to_string())?;
            window
                .set_size(Size::Physical(PhysicalSize::new(
                    selection.width,
                    selection.height,
                )))
                .map_err(|error| error.to_string())?;
            window
                .set_ignore_cursor_events(!interactive)
                .map_err(|error| error.to_string())?;
            window.show().map_err(|error| error.to_string())?;
            if interactive {
                window.set_focus().map_err(|error| error.to_string())?;
            }
            Ok(())
        })();

        let _ = tx.send(result);
    })
    .map_err(|error| error.to_string())?;

    rx.await
        .map_err(|_| "overlay main-thread callback dropped".to_string())?
}

pub fn schedule_window_close(app: &AppHandle, label: &str, delay_ms: u64) {
    let app_handle = app.clone();
    let window_label = label.to_string();

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;

        let app_for_lookup = app_handle.clone();
        let _ = app_handle.run_on_main_thread(move || {
            if let Some(window) = app_for_lookup.get_webview_window(&window_label) {
                let _ = window.close();
            }
        });
    });
}

pub fn hide_window(app: &AppHandle, label: &str) {
    let app_handle = app.clone();
    let window_label = label.to_string();

    let _ = app.run_on_main_thread(move || {
        if let Some(window) = app_handle.get_webview_window(&window_label) {
            let _ = window.hide();
        }
    });
}

pub fn schedule_app_exit(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(60)).await;
        app_handle.exit(0);
    });
}

pub fn request_shutdown(app: &AppHandle, state: SharedState) {
    if APP_SHUTTING_DOWN.swap(true, Ordering::SeqCst) {
        return;
    }

    let snapshot = state.clear_selection();
    emit_snapshot(app, &snapshot);
    emit_translation(app, &state.translation());
}

pub fn selection_or_error(state: &SharedState) -> Result<SelectionRect, String> {
    state.snapshot()
        .selection
        .clone()
        .context("Select a region before starting")
        .map_err(|error| error.to_string())
}

pub async fn open_settings_window(app: &AppHandle, state: SharedState) -> Result<(), String> {
    use tokio::sync::oneshot;

    let (tx, rx) = oneshot::channel();
    let app_handle = app.clone();
    let is_pinned = state.snapshot().panel_pinned;

    app.run_on_main_thread(move || {
        let result: Result<(), String> = (|| {
            if let Some(window) = app_handle.get_webview_window("settings") {
                window.show().map_err(|error| error.to_string())?;
                window.set_focus().map_err(|error| error.to_string())?;
                return Ok(());
            }

            let window = WebviewWindowBuilder::new(
                &app_handle,
                "settings",
                WebviewUrl::App("index.html".into()),
            )
            .title("RangeTranslator Settings")
            .initialization_script(r#"window.__RANGE_TRANSLATOR_VIEW__ = 'settings';"#)
            .decorations(false)
            .transparent(true)
            .always_on_top(is_pinned)
            .skip_taskbar(false)
            .resizable(true)
            .shadow(true)
            .visible(false)
            .focused(true)
            .inner_size(450.0, 350.0)
            .min_inner_size(400.0, 300.0)
            .center()
            .build()
            .map_err(|error| error.to_string())?;

            window.show().map_err(|error| error.to_string())?;
            window.set_focus().map_err(|error| error.to_string())?;
            Ok(())
        })();

        let _ = tx.send(result);
    })
    .map_err(|error| error.to_string())?;

    rx.await
        .map_err(|_| "settings main-thread callback dropped".to_string())?
}
