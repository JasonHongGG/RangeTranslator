use serde_json::json;
use tauri::{AppHandle, Manager, State};

use crate::{
    app::{events::{emit_debug, emit_snapshot, emit_translation}, pipeline, windows},
    benchmark::run_default_prompt_benchmark,
    models::{BenchmarkReport, PipelineSettings, RuntimeCapabilities, RuntimeSnapshot, SelectionRect, TranslationPayload},
    sidecar::runtime_gateway,
    state::SharedState,
};

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
        &snapshot.endpoint,
        &snapshot.model,
        &snapshot.ai_provider,
        &snapshot.prompt_profile,
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
pub fn close_selector_window(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    windows::close_selector_window(&app, state.inner_clone())
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

    windows::ensure_overlay_window(&app, &selection, snapshot_before.copy_mode).await?;

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

    windows::hide_window(&app, "selector");
    windows::schedule_window_close(&app, "selector", 30);

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
    if let Some(window) = app.get_webview_window("overlay") {
        window.close().map_err(|error| error.to_string())?;
    }
    if let Some(window) = app.get_webview_window("selector") {
        window.close().map_err(|error| error.to_string())?;
    }

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
    let selection = windows::selection_or_error(&state.inner_clone())?;
    let capabilities = runtime_gateway().query_capabilities().await?;
    let synced_snapshot = sync_runtime_defaults(&app, state.inner_clone(), &capabilities);
    ensure_ocr_runtime_ready(&synced_snapshot, &capabilities)?;
    windows::ensure_overlay_window(&app, &selection, state.snapshot().copy_mode).await?;
    pipeline::begin_pipeline(&app, state.inner_clone(), settings);
    Ok(())
}

#[tauri::command]
pub fn update_overlay_selection(
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
pub fn stop_pipeline(app: AppHandle, state: State<'_, SharedState>) -> Result<(), String> {
    let snapshot = state.stop_pipeline();
    if let Some(window) = app.get_webview_window("overlay") {
        window
            .set_ignore_cursor_events(!snapshot.copy_mode)
            .map_err(|error| error.to_string())?;
        if snapshot.copy_mode {
            window.set_focus().map_err(|error| error.to_string())?;
        }
    }
    emit_snapshot(&app, &snapshot);
    Ok(())
}

#[tauri::command]
pub fn toggle_copy_mode(
    app: AppHandle,
    state: State<'_, SharedState>,
    enabled: bool,
) -> Result<(), String> {
    let snapshot = state.set_copy_mode(enabled);
    if let Some(window) = app.get_webview_window("overlay") {
        window
            .set_ignore_cursor_events(!enabled)
            .map_err(|error| error.to_string())?;
        if enabled {
            window.set_focus().map_err(|error| error.to_string())?;
        }
    }
    emit_snapshot(&app, &snapshot);
    Ok(())
}

fn sync_runtime_defaults(
    app: &AppHandle,
    state: SharedState,
    capabilities: &RuntimeCapabilities,
) -> RuntimeSnapshot {
    let snapshot = state.snapshot();
    let current_ocr_provider = snapshot.ocr_provider.clone();
    let current_ai_provider = snapshot.ai_provider.clone();
    let current_prompt_profile = snapshot.prompt_profile.clone();

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

    let prompt_profile = if current_prompt_profile.is_empty() {
        capabilities
            .default_prompt_profile_id
            .clone()
            .unwrap_or_default()
    } else {
        current_prompt_profile.clone()
    };

    if ocr_provider == current_ocr_provider
        && ai_provider == current_ai_provider
        && prompt_profile == current_prompt_profile
    {
        return snapshot;
    }

    let next_snapshot = state.set_provider_stack(ocr_provider, ai_provider, prompt_profile);
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
