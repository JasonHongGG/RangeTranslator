use serde_json::json;
use tauri::{AppHandle, Manager, State};

use crate::{
    app::{events::{emit_debug, emit_snapshot, emit_translation}, pipeline, windows},
    benchmark::run_default_prompt_benchmark,
    models::{BenchmarkReport, PipelineSettings, RuntimeCapabilities, SelectionRect, TranslationPayload},
    providers::{
        ai::{default_runtime_client, ollama_descriptor, DEFAULT_PROMPT_PROFILE, OLLAMA_PROVIDER_ID},
        ocr::{available_provider_descriptors, default_ocr_provider_id},
    },
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
pub async fn get_runtime_capabilities() -> Result<RuntimeCapabilities, String> {
    let ai_runtime = default_runtime_client();
    let mut capabilities = ai_runtime.query_capabilities().await.unwrap_or_else(|error| {
        RuntimeCapabilities {
            ocr_providers: Vec::new(),
            ai_providers: vec![ollama_descriptor(false, Some(error))],
            prompt_profiles: Vec::new(),
        }
    });

    let mut ocr_providers = capabilities.ocr_providers;
    for provider in available_provider_descriptors() {
        if !ocr_providers.iter().any(|candidate| candidate.id == provider.id) {
            ocr_providers.push(provider);
        }
    }
    capabilities.ocr_providers = ocr_providers;

    if capabilities.ai_providers.is_empty() {
        capabilities.ai_providers.push(ollama_descriptor(
            false,
            Some("No AI providers discovered".to_string()),
        ));
    }

    Ok(capabilities)
}

#[tauri::command]
pub async fn run_prompt_benchmark(
    state: State<'_, SharedState>,
) -> Result<BenchmarkReport, String> {
    let snapshot = state.snapshot();
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
    pipeline::begin_pipeline(&app, state.inner_clone(), settings);
    windows::schedule_window_close(&app, "selector", 30);
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
            .set_ignore_cursor_events(true)
            .map_err(|error| error.to_string())?;
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

pub fn default_provider_stack() -> (String, String, String) {
    (
        default_ocr_provider_id().to_string(),
        OLLAMA_PROVIDER_ID.to_string(),
        DEFAULT_PROMPT_PROFILE.to_string(),
    )
}
