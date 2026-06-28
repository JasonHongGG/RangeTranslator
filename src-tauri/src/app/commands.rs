use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Manager, State};

use crate::{
    app::{
        events::{emit_debug, emit_snapshot, emit_translation},
        pipeline, windows,
    },
    benchmark::run_default_prompt_benchmark,
    models::{
        BenchmarkReport, OcrWarmupRequest, OverlayInteractionMode, PipelineSettings,
        RuntimeCapabilities, RuntimeSnapshot, SelectionRect, TranslationPayload,
    },
    sidecar::runtime_gateway,
    state::SharedState,
};

const BACKGROUND_OCR_PREWARM_DELAY: Duration = Duration::from_millis(100);

#[tauri::command]
pub fn get_runtime_snapshot(state: State<'_, SharedState>) -> crate::models::RuntimeSnapshot {
    state.snapshot()
}

#[tauri::command]
pub fn get_latest_translation(state: State<'_, SharedState>) -> TranslationPayload {
    state.translation()
}

#[tauri::command]
pub async fn get_runtime_capabilities(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<RuntimeCapabilities, String> {
    let capabilities = runtime_gateway().query_capabilities().await?;
    sync_runtime_defaults(&app, state.inner_clone(), &capabilities);
    Ok(capabilities)
}

#[tauri::command]
pub async fn run_prompt_benchmark(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<BenchmarkReport, String> {
    let capabilities = runtime_gateway().query_capabilities().await?;
    let snapshot = sync_runtime_defaults(&app, state.inner_clone(), &capabilities);
    run_default_prompt_benchmark(
        &snapshot.ai_provider,
    )
    .await
}

#[tauri::command]
pub fn panel_minimize(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("panel")
        .ok_or_else(|| "panel window missing".to_string())?;
    window.minimize().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn toggle_panel_pin(
    app: AppHandle,
    state: State<'_, SharedState>,
    enabled: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("panel")
        .ok_or_else(|| "panel window missing".to_string())?;

    window
        .set_always_on_top(enabled)
        .map_err(|error| error.to_string())?;

    if let Some(settings_window) = app.get_webview_window("settings") {
        let _ = settings_window.set_always_on_top(enabled);
    }

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
pub fn toggle_ai_translation(
    app: AppHandle,
    state: State<'_, SharedState>,
    enabled: bool,
) -> Result<(), String> {
    let snapshot_before = state.snapshot();
    let snapshot = state.set_ai_translation_enabled(enabled);
    emit_debug(
        &app,
        "panel-backend",
        "AI translation toggled",
        json!({
            "enabled": enabled,
            "running": snapshot_before.running,
        }),
    );
    emit_snapshot(&app, &snapshot);

    Ok(())
}

#[tauri::command]
pub fn get_magnifier_region(x: i32, y: i32, size: u32) -> Result<String, String> {
    use crate::capture::capture_region;
    use crate::models::SelectionRect;
    use image::codecs::png::{PngEncoder, CompressionType, FilterType};
    use std::io::Cursor;
    use base64::{Engine as _, engine::general_purpose};
    use image::{ColorType, ImageEncoder};

    let half = (size / 2) as i32;
    let crop_x = x - half;
    let crop_y = y - half;

    let selection = SelectionRect {
        x: crop_x,
        y: crop_y,
        width: size,
        height: size,
    };

    let frame = capture_region(&selection).map_err(|e| e.to_string())?;

    let mut buffer = Cursor::new(Vec::new());
    let encoder = PngEncoder::new_with_quality(
        &mut buffer,
        CompressionType::Fast,
        FilterType::NoFilter,
    );
    encoder.write_image(
        frame.image.as_raw(),
        frame.image.width(),
        frame.image.height(),
        ColorType::Rgba8.into(),
    ).map_err(|e| e.to_string())?;
    
    let b64 = general_purpose::STANDARD.encode(buffer.into_inner());
    Ok(format!("data:image/png;base64,{}", b64))
}

#[tauri::command]
pub fn set_languages(
    app: AppHandle,
    state: State<'_, SharedState>,
    source_language: String,
    target_language: String,
) -> Result<(), String> {
    let snapshot = state.set_languages(source_language, target_language);
    emit_debug(
        &app,
        "panel-backend",
        "languages updated",
        json!({
            "source_language": snapshot.source_language,
            "target_language": snapshot.target_language,
        }),
    );
    emit_snapshot(&app, &snapshot);
    Ok(())
}

#[tauri::command]
pub fn panel_close(app: AppHandle, state: State<'_, SharedState>) -> Result<(), String> {
    emit_debug(
        &app,
        "panel-backend",
        "panel_close invoked",
        json!({
            "hasSelection": state.snapshot().selection.is_some(),
            "running": state.snapshot().running,
        }),
    );
    windows::request_shutdown(&app, state.inner_clone());
    windows::schedule_app_exit(&app);
    Ok(())
}

#[tauri::command]
pub async fn open_selector_window(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    windows::open_selector_window(&app, state.inner_clone()).await
}

#[tauri::command]
pub fn close_selector_window(app: AppHandle, state: State<'_, SharedState>) -> Result<(), String> {
    windows::close_selector_window(&app, state.inner_clone())
}

#[tauri::command]
pub async fn open_settings_window(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    windows::open_settings_window(&app, state.inner_clone()).await
}

#[tauri::command]
pub async fn submit_selection(
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

    windows::hide_window(&app, "selector");
    windows::show_window(&app, "panel");

    let content_protected = !snapshot_before.debug_screenshot_mode;
    windows::ensure_overlay_window(
        &app,
        &selection,
        snapshot_before.overlay_mode.is_interactive(),
        content_protected,
    )
    .await?;

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

    pipeline::begin_pipeline(&app, state.inner_clone(), settings);

    let app_handle = app.clone();
    let shared_state = state.inner_clone();
    tauri::async_runtime::spawn(async move {
        match runtime_gateway().query_capabilities().await {
            Ok(capabilities) => {
                sync_runtime_defaults(&app_handle, shared_state, &capabilities);
            }
            Err(error) => {
                emit_debug(
                    &app_handle,
                    "selector-backend",
                    "selection committed; capability sync moved off hot path and failed",
                    json!({
                        "error": error,
                    }),
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub fn clear_selection(app: AppHandle, state: State<'_, SharedState>) -> Result<(), String> {
    emit_debug(
        &app,
        "selector-backend",
        "clear_selection invoked",
        json!({
            "hasSelection": state.snapshot().selection.is_some(),
        }),
    );
    if let Some(_window) = app.get_webview_window("overlay") {
        windows::hide_window(&app, "overlay");
    }
    if let Some(_window) = app.get_webview_window("selector") {
        windows::hide_window(&app, "selector");
    }
    windows::show_window(&app, "panel");

    let snapshot = state.clear_selection();
    emit_snapshot(&app, &snapshot);
    emit_translation(&app, &state.translation());
    Ok(())
}

#[tauri::command]
pub async fn start_pipeline(
    app: AppHandle,
    state: State<'_, SharedState>,
    settings: PipelineSettings,
) -> Result<(), String> {
    let content_protected = !state.snapshot().debug_screenshot_mode;

    let selection = windows::selection_or_error(&state.inner_clone())?;
    let snapshot_before = state.snapshot();
    let needs_runtime_query = snapshot_before.ocr_provider.is_empty()
        || (snapshot_before.ai_translation_enabled
            && snapshot_before.ai_provider.is_empty());

    if needs_runtime_query {
        let capabilities = runtime_gateway().query_capabilities().await?;
        let synced_snapshot = sync_runtime_defaults(&app, state.inner_clone(), &capabilities);
        ensure_ocr_runtime_ready(&synced_snapshot, &capabilities)?;
    }

    windows::ensure_overlay_window(
        &app,
        &selection,
        state.snapshot().overlay_mode.is_interactive(),
        content_protected,
    )
    .await?;
    pipeline::begin_pipeline(&app, state.inner_clone(), settings);
    Ok(())
}

#[tauri::command]
pub fn update_overlay_selection(
    app: AppHandle,
    state: State<'_, SharedState>,
    selection: SelectionRect,
) -> Result<(), String> {
    let snapshot_before = state.snapshot();

    if !snapshot_before.overlay_mode.is_interactive() {
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

    if snapshot_before.running {
        let (token, snapshot, translation) = state.restart_pipeline_with_selection(selection);
        emit_debug(
            &app,
            "overlay-backend",
            "overlay selection update restarted live pipeline",
            json!({
                "generation": snapshot.generation,
                "selection": snapshot.selection,
            }),
        );
        emit_snapshot(&app, &snapshot);
        emit_translation(&app, &translation);
        pipeline::spawn_pipeline(&app, state.inner_clone(), token);
        return Ok(());
    }

    let snapshot = state.set_selection(selection);
    emit_snapshot(&app, &snapshot);
    emit_translation(&app, &state.translation());
    Ok(())
}

#[tauri::command]
pub fn stop_pipeline(app: AppHandle, state: State<'_, SharedState>) -> Result<(), String> {
    let snapshot = state.stop_pipeline();
    if let Some(window) = app.get_webview_window("overlay") {
        window
            .set_ignore_cursor_events(!snapshot.overlay_mode.is_interactive())
            .map_err(|error| error.to_string())?;
        if snapshot.overlay_mode.is_interactive() {
            window.set_focus().map_err(|error| error.to_string())?;
        }
    }
    emit_snapshot(&app, &snapshot);
    Ok(())
}

#[tauri::command]
pub fn set_overlay_interaction_mode(
    app: AppHandle,
    state: State<'_, SharedState>,
    mode: OverlayInteractionMode,
) -> Result<(), String> {
    let snapshot = state.set_overlay_mode(mode);
    if let Some(window) = app.get_webview_window("overlay") {
        window
            .set_ignore_cursor_events(!mode.is_interactive())
            .map_err(|error| error.to_string())?;
        if mode.is_interactive() {
            window.set_focus().map_err(|error| error.to_string())?;
        }
    }
    emit_snapshot(&app, &snapshot);

    Ok(())
}

#[tauri::command]
pub fn toggle_debug_screenshot_mode(
    app: AppHandle,
    state: State<'_, SharedState>,
    enabled: bool,
) -> Result<(), String> {
    let snapshot_before = state.snapshot();
    let was_running = snapshot_before.running;

    // Toggle capture protection so the user can take screenshots.
    windows::set_capture_protection(&app, !enabled)?;

    let snapshot = state.set_debug_screenshot_mode(enabled);
    emit_debug(
        &app,
        "panel-backend",
        "debug screenshot mode toggled",
        json!({
            "enabled": enabled,
            "wasRunning": was_running,
            "preservedStatus": snapshot_before.status,
        }),
    );
    emit_snapshot(&app, &snapshot);

    // When debug mode is turned OFF and the pipeline was running,
    // re-spawn the pipeline loop since it exited when debug was activated.
    if !enabled && was_running {
        let settings = PipelineSettings {
            source_language: snapshot.source_language.clone(),
            target_language: snapshot.target_language.clone(),
        };
        pipeline::begin_pipeline(&app, state.inner_clone(), settings);
    }

    Ok(())
}

pub fn spawn_runtime_prewarm(app: &AppHandle, state: SharedState) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(BACKGROUND_OCR_PREWARM_DELAY).await;

        let snapshot_before = state.snapshot();
        if snapshot_before.running || snapshot_before.selection.is_some() {
            emit_debug(
                &app_handle,
                "runtime-backend",
                "skipped OCR prewarm because the user already started interacting",
                json!({
                    "running": snapshot_before.running,
                    "hasSelection": snapshot_before.selection.is_some(),
                }),
            );
            return;
        }

        let capabilities = match runtime_gateway().query_capabilities().await {
            Ok(capabilities) => capabilities,
            Err(error) => {
                emit_debug(
                    &app_handle,
                    "runtime-backend",
                    "background capability query failed",
                    json!({
                        "error": error,
                    }),
                );
                return;
            }
        };

        let snapshot = sync_runtime_defaults(&app_handle, state.clone(), &capabilities);
        let latest_snapshot = state.snapshot();
        if latest_snapshot.running || latest_snapshot.selection.is_some() {
            emit_debug(
                &app_handle,
                "runtime-backend",
                "aborted OCR prewarm because the user became active",
                json!({
                    "running": latest_snapshot.running,
                    "hasSelection": latest_snapshot.selection.is_some(),
                }),
            );
            return;
        }

        if snapshot.ocr_provider.is_empty() {
            emit_debug(
                &app_handle,
                "runtime-backend",
                "skipped OCR prewarm because no default provider is available",
                json!({
                    "ocrProviders": capabilities.ocr_providers,
                }),
            );
            return;
        }

        match runtime_gateway()
            .prewarm_ocr(OcrWarmupRequest {
                provider_id: snapshot.ocr_provider.clone(),
                source_language: snapshot.source_language.clone(),
                hint_language: snapshot.last_detected_source.clone(),
            })
            .await
        {
            Ok(response) => emit_debug(
                &app_handle,
                "runtime-backend",
                "background OCR prewarm complete",
                json!({
                    "provider": response.provider_id,
                    "language": response.language,
                    "detail": response.detail,
                }),
            ),
            Err(error) => emit_debug(
                &app_handle,
                "runtime-backend",
                "background OCR prewarm failed",
                json!({
                    "error": error,
                }),
            ),
        }
    });
}

fn sync_runtime_defaults(
    app: &AppHandle,
    state: SharedState,
    capabilities: &RuntimeCapabilities,
) -> RuntimeSnapshot {
    let snapshot = state.snapshot();
    let current_ocr_provider = snapshot.ocr_provider.clone();
    let current_ai_provider = snapshot.ai_provider.clone();

    let ocr_provider = if current_ocr_provider.is_empty() {
        capabilities
            .default_ocr_provider_id
            .clone()
            .unwrap_or_default()
    } else {
        current_ocr_provider.clone()
    };

    let ai_provider = if current_ai_provider.is_empty() {
        capabilities
            .default_ai_provider_id
            .clone()
            .unwrap_or_default()
    } else {
        current_ai_provider.clone()
    };

    if ocr_provider == current_ocr_provider
        && ai_provider == current_ai_provider
    {
        return snapshot;
    }

    let next_snapshot = state.set_provider_stack(ocr_provider, ai_provider);
    emit_snapshot(app, &next_snapshot);
    next_snapshot
}

fn ensure_ocr_runtime_ready(
    snapshot: &RuntimeSnapshot,
    capabilities: &RuntimeCapabilities,
) -> Result<(), String> {
    if !snapshot.ocr_provider.is_empty() {
        return Ok(());
    }

    let details = capabilities
        .ocr_providers
        .iter()
        .map(|provider| {
            provider
                .detail
                .as_ref()
                .map(|detail| format!("{}: {detail}", provider.id))
                .unwrap_or_else(|| provider.id.clone())
        })
        .collect::<Vec<_>>();

    if details.is_empty() {
        return Err("No OCR provider is registered in the sidecar runtime.".to_string());
    }

    Err(format!(
        "No OCR provider is available in the sidecar runtime. {}",
        details.join(" | ")
    ))
}
