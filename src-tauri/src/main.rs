#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod models;
mod ollama;
mod ocr;
mod runtime;

use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use capture::{capture_region, estimate_colors, virtual_desktop_bounds, FrameSignature};
use serde::Serialize;
use serde_json::{Value, json};
use models::{
    OverlayBlock, PipelineSettings, RuntimeStatus, SelectionRect, TextAlign,
    TranslationPayload,
};
use ocr::recognize_capture;
use runtime::SharedState;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size,
    State, WebviewUrl, WebviewWindowBuilder,
};

const SNAPSHOT_EVENT: &str = "runtime-snapshot";
const TRANSLATION_EVENT: &str = "translation-update";
const DEBUG_EVENT: &str = "selector-debug";
const DEFAULT_ENDPOINT: &str =
    "https://lacresha-posological-steven.ngrok-free.dev";
const DEFAULT_MODEL: &str = "discovering";
const SELECTOR_INIT_SCRIPT: &str = r#"window.__RANGE_TRANSLATOR_VIEW__ = 'selector';"#;
static APP_SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DebugPayload {
    scope: String,
    message: String,
    detail: Value,
    timestamp: String,
}

#[tauri::command]
fn get_runtime_snapshot(state: State<'_, SharedState>) -> models::RuntimeSnapshot {
    state.snapshot()
}

#[tauri::command]
fn get_latest_translation(state: State<'_, SharedState>) -> TranslationPayload {
    state.translation()
}

#[tauri::command]
fn panel_minimize(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("panel")
        .context("panel window missing")
        .map_err(error_to_string)?;
    window.minimize().map_err(error_to_string)
}

#[tauri::command]
fn toggle_panel_pin(
    app: AppHandle,
    state: State<'_, SharedState>,
    enabled: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("panel")
        .context("panel window missing")
        .map_err(error_to_string)?;

    window.set_always_on_top(enabled).map_err(error_to_string)?;

    let snapshot = state.set_panel_pinned(enabled);
    emit_debug(
        &app,
        "panel-backend",
        "panel pin toggled",
        json!({
            "enabled": enabled,
        }),
    );
    emit_snapshot(&app, &snapshot);
    Ok(())
}

#[tauri::command]
fn panel_close(app: AppHandle, state: State<'_, SharedState>) -> Result<(), String> {
    emit_debug(
        &app,
        "panel-backend",
        "panel_close invoked",
        json!({
            "hasSelection": state.snapshot().selection.is_some(),
            "running": state.snapshot().running,
        }),
    );
    request_shutdown(&app, state.inner_clone());
    schedule_app_exit(&app);
    Ok(())
}

#[tauri::command]
async fn open_selector_window(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    use tokio::sync::oneshot;

    let bounds = virtual_desktop_bounds().map_err(error_to_string)?;
    let debug_window_mode = cfg!(debug_assertions);
    let shared = state.inner_clone();
    let (window_x, window_y, window_width, window_height) =
        (bounds.x, bounds.y, bounds.width, bounds.height);
    let selector_bounds = SelectionRect {
        x: window_x,
        y: window_y,
        width: window_width,
        height: window_height,
    };

    emit_debug(
        &app,
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
                window
                    .set_position(Position::Physical(PhysicalPosition::new(window_x, window_y)))
                    .map_err(error_to_string)?;
                window
                    .set_size(Size::Physical(PhysicalSize::new(window_width, window_height)))
                    .map_err(error_to_string)?;
                window
                    .set_always_on_top(true)
                    .map_err(error_to_string)?;
                window
                    .set_ignore_cursor_events(false)
                    .map_err(error_to_string)?;
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
                window.show().map_err(error_to_string)?;
                window.set_focus().map_err(error_to_string)?;

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
                    let message = error_to_string(error);
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

            window
                .set_position(Position::Physical(PhysicalPosition::new(window_x, window_y)))
                .map_err(error_to_string)?;
            window
                .set_size(Size::Physical(PhysicalSize::new(window_width, window_height)))
                .map_err(error_to_string)?;

            if let Err(error) = window.set_ignore_cursor_events(false) {
                let message = error_to_string(error);
                emit_debug(
                    &app_handle,
                    "selector-backend",
                    "selector ignore cursor update failed",
                    json!({ "error": message }),
                );
                return Err(message);
            }
            if let Err(error) = window.show() {
                let message = error_to_string(error);
                emit_debug(
                    &app_handle,
                    "selector-backend",
                    "selector show failed",
                    json!({ "error": message }),
                );
                return Err(message);
            }
            if let Err(error) = window.set_focus() {
                let message = error_to_string(error);
                emit_debug(
                    &app_handle,
                    "selector-backend",
                    "selector focus failed",
                    json!({ "error": message }),
                );
                return Err(message);
            }

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
    .map_err(error_to_string)?;

    rx.await
        .map_err(|_| "selector main-thread callback dropped".to_string())??;

    let snapshot = shared.activate_selector(selector_bounds.clone());
    emit_debug(
        &app,
        "selector-backend",
        "selector mode activated",
        json!({
            "status": snapshot.status,
            "selection": snapshot.selection,
            "selectorBounds": selector_bounds,
        }),
    );
    emit_snapshot(&app, &snapshot);
    Ok(())
}

#[tauri::command]
fn close_selector_window(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    emit_debug(
        &app,
        "selector-backend",
        "close_selector_window invoked",
        json!({
            "hasWindow": app.get_webview_window("selector").is_some(),
        }),
    );
    let snapshot = state.cancel_selector();
    emit_debug(
        &app,
        "selector-backend",
        "selector mode cancelled",
        json!({
            "status": snapshot.status,
        }),
    );
    emit_snapshot(&app, &snapshot);
    schedule_window_close(&app, "selector", 30);
    Ok(())
}

#[tauri::command]
async fn submit_selection(
    app: AppHandle,
    state: State<'_, SharedState>,
    selection: SelectionRect,
) -> Result<(), String> {
    emit_debug(
        &app,
        "selector-backend",
        "submit_selection invoked",
        json!({
            "selection": selection,
        }),
    );
    let snapshot_before = state.snapshot();
    let settings = PipelineSettings {
        source_language: snapshot_before.source_language,
        target_language: snapshot_before.target_language,
    };

    ensure_overlay_window(&app, &selection, snapshot_before.copy_mode).await?;

    let snapshot = state.set_selection(selection);
    emit_debug(
        &app,
        "selector-backend",
        "selection committed",
        json!({
            "selection": snapshot.selection,
            "status": snapshot.status,
        }),
    );
    emit_snapshot(&app, &snapshot);
    emit_translation(&app, &state.translation());
    begin_pipeline(&app, state.inner_clone(), settings);
    schedule_window_close(&app, "selector", 30);
    Ok(())
}

#[tauri::command]
fn clear_selection(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    emit_debug(
        &app,
        "selector-backend",
        "clear_selection invoked",
        json!({
            "hasSelection": state.snapshot().selection.is_some(),
        }),
    );
    if let Some(window) = app.get_webview_window("overlay") {
        window.close().map_err(error_to_string)?;
    }
    if let Some(window) = app.get_webview_window("selector") {
        window.close().map_err(error_to_string)?;
    }

    let snapshot = state.clear_selection();
    emit_snapshot(&app, &snapshot);
    emit_translation(&app, &state.translation());
    Ok(())
}

#[tauri::command]
async fn start_pipeline(
    app: AppHandle,
    state: State<'_, SharedState>,
    settings: PipelineSettings,
) -> Result<(), String> {
    let selection = state
        .snapshot()
        .selection
        .clone()
        .context("Select a region before starting")
        .map_err(error_to_string)?;

    ensure_overlay_window(&app, &selection, state.snapshot().copy_mode).await?;
    begin_pipeline(&app, state.inner_clone(), settings);

    Ok(())
}

#[tauri::command]
fn update_overlay_selection(
    app: AppHandle,
    state: State<'_, SharedState>,
    selection: SelectionRect,
) -> Result<(), String> {
    if !state.snapshot().copy_mode {
        emit_debug(
            &app,
            "overlay-backend",
            "ignored overlay selection update while passive",
            json!({
                "selection": selection,
            }),
        );
        return Ok(());
    }

    emit_debug(
        &app,
        "overlay-backend",
        "update_overlay_selection invoked",
        json!({
            "selection": selection,
        }),
    );

    let snapshot = state.set_selection(selection);
    emit_snapshot(&app, &snapshot);
    emit_translation(&app, &state.translation());

    Ok(())
}

#[tauri::command]
fn stop_pipeline(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let snapshot = state.stop_pipeline();
    if let Some(window) = app.get_webview_window("overlay") {
        window
            .set_ignore_cursor_events(true)
            .map_err(error_to_string)?;
    }
    emit_snapshot(&app, &snapshot);
    Ok(())
}

#[tauri::command]
fn toggle_copy_mode(
    app: AppHandle,
    state: State<'_, SharedState>,
    enabled: bool,
) -> Result<(), String> {
    let snapshot = state.set_copy_mode(enabled);
    if let Some(window) = app.get_webview_window("overlay") {
        window
            .set_ignore_cursor_events(!enabled)
            .map_err(error_to_string)?;
        if enabled {
            window.set_focus().map_err(error_to_string)?;
        }
    }
    emit_snapshot(&app, &snapshot);
    Ok(())
}

async fn pipeline_loop(
    app: AppHandle,
    state: SharedState,
    token: u64,
) -> Result<()> {
    let mut last_signature: Option<FrameSignature> = None;
    let mut detected_source_hint: Option<String> = None;

    loop {
        if !state.is_token_active(token) {
            break;
        }

        let snapshot = state.snapshot();
        let selection = if let Some(selection) = snapshot.selection.clone() {
            selection
        } else {
            let snapshot = state.stop_pipeline();
            emit_snapshot(&app, &snapshot);
            break;
        };

        emit_snapshot(&app, &state.set_status(RuntimeStatus::Capturing, "Sampling"));
        let frame = capture_region(&selection)?;
        let signature = FrameSignature::from_image(&frame.image);
        if let Some(previous) = last_signature.as_ref() {
            if !signature.is_meaningfully_different(previous) {
                tokio::time::sleep(Duration::from_millis(180)).await;
                continue;
            }
        }
        last_signature = Some(signature);

        emit_snapshot(&app, &state.set_status(RuntimeStatus::Recognizing, "OCR"));
        let recognized = recognize_capture(
            &frame,
            &snapshot.source_language,
            detected_source_hint.as_deref(),
        )?;
        if snapshot.source_language == "auto" {
            detected_source_hint = Some(recognized.language.clone());
        }

        emit_snapshot(&app, &state.set_status(RuntimeStatus::Translating, "Ollama"));
        let texts = recognized
            .lines
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>();

        let translation = ollama::translate_texts(
            &snapshot.endpoint,
            &snapshot.model,
            &recognized.language,
            &snapshot.target_language,
            &texts,
        )
        .await
        .unwrap_or_else(|_| ollama::TranslationResponse {
            model: snapshot.model.clone(),
            detected_source: recognized.language.clone(),
            translations: texts.clone(),
        });

        let model_snapshot = state.set_model(translation.model.clone());
        emit_snapshot(&app, &model_snapshot);

        let blocks = recognized
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let (foreground, background) = estimate_colors(&frame.image, &line.rect);
                OverlayBlock {
                    id: format!("block-{index}"),
                    source_text: line.text.clone(),
                    translated_text: translation
                        .translations
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| line.text.clone()),
                    x: line.rect.x,
                    y: line.rect.y,
                    width: line.rect.width.max(48),
                    height: line.rect.height.max(24),
                    font_size: (line.rect.height as f32 * 0.72).clamp(14.0, 42.0),
                    confidence: line.confidence,
                    foreground,
                    background,
                    align: TextAlign::Left,
                }
            })
            .collect::<Vec<_>>();

        let payload = TranslationPayload {
            selection: Some(selection.clone()),
            source_language: snapshot.source_language.clone(),
            target_language: snapshot.target_language.clone(),
            detected_source: Some(translation.detected_source),
            captured_at: Some(runtime::timestamp()),
            unchanged: false,
            blocks,
        };

        let snapshot = state.set_translation(payload.clone());
        emit_snapshot(&app, &snapshot);
        emit_translation(&app, &payload);

        tokio::time::sleep(Duration::from_millis(180)).await;
    }

    Ok(())
}

async fn ensure_overlay_window(
    app: &AppHandle,
    selection: &SelectionRect,
    interactive: bool,
) -> Result<(), String> {
    use tokio::sync::oneshot;

    let (tx, rx) = oneshot::channel();
    let app_handle = app.clone();
    let selection = selection.clone();

    app.run_on_main_thread(move || {
        let result: Result<(), String> = (|| {
            if let Some(window) = app_handle.get_webview_window("overlay") {
                window
                    .set_visible_on_all_workspaces(true)
                    .map_err(error_to_string)?;
                window.set_always_on_top(true).map_err(error_to_string)?;
                window.set_decorations(false).map_err(error_to_string)?;
                window.set_skip_taskbar(true).map_err(error_to_string)?;
                window.set_shadow(false).map_err(error_to_string)?;
                window.set_resizable(true).map_err(error_to_string)?;
                window
                    .set_position(Position::Physical(PhysicalPosition::new(
                        selection.x,
                        selection.y,
                    )))
                    .map_err(error_to_string)?;
                window
                    .set_size(Size::Physical(PhysicalSize::new(
                        selection.width,
                        selection.height,
                    )))
                    .map_err(error_to_string)?;
                window
                    .set_ignore_cursor_events(!interactive)
                    .map_err(error_to_string)?;
                window.show().map_err(error_to_string)?;
                if interactive {
                    window.set_focus().map_err(error_to_string)?;
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
            .map_err(error_to_string)?;

            window
                .set_position(Position::Physical(PhysicalPosition::new(
                    selection.x,
                    selection.y,
                )))
                .map_err(error_to_string)?;
            window
                .set_size(Size::Physical(PhysicalSize::new(
                    selection.width,
                    selection.height,
                )))
                .map_err(error_to_string)?;

            window
                .set_ignore_cursor_events(!interactive)
                .map_err(error_to_string)?;
            window.show().map_err(error_to_string)?;
            if interactive {
                window.set_focus().map_err(error_to_string)?;
            }
            Ok(())
        })();

        let _ = tx.send(result);
    })
    .map_err(error_to_string)?;

    rx.await
        .map_err(|_| "overlay main-thread callback dropped".to_string())?
}

fn begin_pipeline(app: &AppHandle, state: SharedState, settings: PipelineSettings) {
    let (token, snapshot) = state.start_pipeline(settings);
    emit_snapshot(app, &snapshot);

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = pipeline_loop(app_handle.clone(), state.clone(), token).await {
            let snapshot = state.set_error(error.to_string());
            emit_snapshot(&app_handle, &snapshot);
        }
    });
}

fn schedule_window_close(app: &AppHandle, label: &str, delay_ms: u64) {
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

fn schedule_app_exit(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(60)).await;
        app_handle.exit(0);
    });
}

fn request_shutdown(app: &AppHandle, state: SharedState) {
    if APP_SHUTTING_DOWN.swap(true, Ordering::SeqCst) {
        return;
    }

    let snapshot = state.clear_selection();
    emit_snapshot(app, &snapshot);
    emit_translation(app, &state.translation());
}

fn emit_snapshot(app: &AppHandle, snapshot: &models::RuntimeSnapshot) {
    let _ = app.emit(SNAPSHOT_EVENT, snapshot.clone());
}

fn emit_translation(app: &AppHandle, payload: &TranslationPayload) {
    let _ = app.emit(TRANSLATION_EVENT, payload.clone());
}

fn emit_debug(app: &AppHandle, scope: &str, message: &str, detail: Value) {
    let payload = DebugPayload {
        scope: scope.to_string(),
        message: message.to_string(),
        detail,
        timestamp: runtime::timestamp(),
    };
    let _ = app.emit(DEBUG_EVENT, payload.clone());
    println!(
        "[RangeTranslator:{}] {} {}",
        payload.scope, payload.message, payload.detail
    );
}

fn error_to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn main() {
    let state = SharedState::new(
        DEFAULT_ENDPOINT.to_string(),
        DEFAULT_MODEL.to_string(),
    );

    tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            if let Some(window) = app.get_webview_window("panel") {
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
                    if !APP_SHUTTING_DOWN.load(Ordering::SeqCst) {
                        emit_debug(
                            &window.app_handle(),
                            "panel-window",
                            "panel close requested",
                            json!(null),
                        );

                        let state = window.app_handle().state::<SharedState>();
                        request_shutdown(&window.app_handle(), state.inner_clone());
                        schedule_app_exit(&window.app_handle());
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            clear_selection,
            close_selector_window,
            get_latest_translation,
            get_runtime_snapshot,
            open_selector_window,
            panel_close,
            panel_minimize,
            toggle_panel_pin,
            start_pipeline,
            stop_pipeline,
            submit_selection,
            toggle_copy_mode,
            update_overlay_selection,
        ])
        .run(tauri::generate_context!())
        .expect("tauri application failed")
}
