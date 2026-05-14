use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    time::Duration,
};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde_json::json;
use tauri::AppHandle;

use crate::{
    app::events::{emit_debug, emit_snapshot, emit_translation, emit_translation_partial},
    capture::{capture_region, estimate_colors, FrameSignature},
    models::{
        AiTranslationDelta, AiTranslationRequest, AiTranslationResponse, OverlayBlock,
        PartialUpdateStage, PipelineSettings, RuntimeStatus, TextAlign,
        TranslationPartialPayload, TranslationPayload,
    },
    providers::{
        ai::default_runtime_client,
        ocr::{resolve_ocr_provider, OcrTextLine},
    },
    state::{self, SharedState},
};

static TRANSLATION_CACHE: OnceLock<Mutex<HashMap<String, AiTranslationResponse>>> = OnceLock::new();

pub fn begin_pipeline(app: &AppHandle, state: SharedState, settings: PipelineSettings) {
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

pub async fn pipeline_loop(app: AppHandle, state: SharedState, token: u64) -> Result<()> {
    let mut last_signature: Option<FrameSignature> = None;
    let mut detected_source_hint: Option<String> = None;
    let ai_runtime = default_runtime_client();

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

        let ocr_provider = resolve_ocr_provider(&snapshot.ocr_provider);

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
        let recognized = ocr_provider.recognize(
            &frame,
            &snapshot.source_language,
            detected_source_hint.as_deref(),
        )?;
        if snapshot.source_language == "auto" {
            detected_source_hint = Some(recognized.language.clone());
        }

        let captured_at = state::timestamp();
        let base_blocks = recognized
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| build_overlay_block(&frame, line, index, line.text.clone(), true))
            .collect::<Vec<_>>();

        let provider_snapshot = state.set_provider_stack(
            ocr_provider.id().to_string(),
            snapshot.ai_provider.clone(),
            snapshot.prompt_profile.clone(),
        );
        emit_snapshot(&app, &provider_snapshot);
        emit_translation_partial(
            &app,
            &TranslationPartialPayload {
                selection: Some(selection.clone()),
                source_language: snapshot.source_language.clone(),
                target_language: snapshot.target_language.clone(),
                detected_source: Some(recognized.language.clone()),
                captured_at: Some(captured_at.clone()),
                provider: ocr_provider.id().to_string(),
                prompt_profile: snapshot.prompt_profile.clone(),
                stage: PartialUpdateStage::Ocr,
                complete: false,
                blocks: base_blocks.clone(),
            },
        );

        emit_snapshot(&app, &state.set_status(RuntimeStatus::Translating, "AI"));
        let texts = recognized
            .lines
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>();

        let app_handle = app.clone();
        let selection_for_partial = selection.clone();
        let source_language_for_partial = snapshot.source_language.clone();
        let target_language_for_partial = snapshot.target_language.clone();
        let base_blocks_for_partial = base_blocks.clone();
        let state_for_partial = state.clone();
        let partial_handler = Arc::new(move |delta: AiTranslationDelta| {
            if !state_for_partial.is_token_active(token) {
                return;
            }

            if let Some(base_block) = base_blocks_for_partial.get(delta.index).cloned() {
                let mut block = base_block;
                block.translated_text = delta.translated_text.clone();
                block.streaming = !delta.done;
                if let Some(ai_confidence) = delta.confidence {
                    block.confidence = (block.confidence * ai_confidence).clamp(0.0, 1.0);
                }

                emit_translation_partial(
                    &app_handle,
                    &TranslationPartialPayload {
                        selection: Some(selection_for_partial.clone()),
                        source_language: source_language_for_partial.clone(),
                        target_language: target_language_for_partial.clone(),
                        detected_source: delta.detected_source.clone(),
                        captured_at: Some(state::timestamp()),
                        provider: delta.provider_id.clone(),
                        prompt_profile: delta.prompt_profile.clone(),
                        stage: PartialUpdateStage::Translation,
                        complete: delta.done,
                        blocks: vec![block],
                    },
                );
            }
        });

        let ai_request = AiTranslationRequest {
            endpoint: snapshot.endpoint.clone(),
            provider_id: snapshot.ai_provider.clone(),
            model: snapshot.model.clone(),
            prompt_profile: snapshot.prompt_profile.clone(),
            source_language: recognized.language.clone(),
            target_language: snapshot.target_language.clone(),
            texts: texts.clone(),
        };

        let cache_key = build_translation_cache_key(&ai_request).unwrap_or_default();
        let translation = if let Some(cached) = translation_cache().lock().get(&cache_key).cloned() {
            emit_debug(
                &app,
                "ai-provider",
                "translation cache hit",
                json!({
                    "provider": cached.provider_id,
                    "promptProfile": cached.prompt_profile,
                }),
            );
            cached
        } else {
            ai_runtime
                .translate(ai_request, partial_handler)
                .await
                .map(|response| {
                    if !cache_key.is_empty() {
                        translation_cache()
                            .lock()
                            .insert(cache_key.clone(), response.clone());
                    }
                    response
                })
                .unwrap_or_else(|error| {
                    emit_debug(
                        &app,
                        "ai-provider",
                        "sidecar translate failed",
                        json!({
                            "error": error,
                            "provider": snapshot.ai_provider,
                            "promptProfile": snapshot.prompt_profile,
                        }),
                    );

                    AiTranslationResponse {
                        provider_id: snapshot.ai_provider.clone(),
                        model: snapshot.model.clone(),
                        prompt_profile: snapshot.prompt_profile.clone(),
                        detected_source: recognized.language.clone(),
                        translations: texts.clone(),
                        confidences: vec![1.0; texts.len()],
                    }
                })
        };

        let provider_snapshot = state.set_provider_stack(
            ocr_provider.id().to_string(),
            translation.provider_id.clone(),
            translation.prompt_profile.clone(),
        );
        emit_snapshot(&app, &provider_snapshot);

        let model_snapshot = state.set_model(translation.model.clone());
        emit_snapshot(&app, &model_snapshot);

        let blocks = recognized
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                build_overlay_block(
                    &frame,
                    line,
                    index,
                    translation
                        .translations
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| line.text.clone()),
                    false,
                )
                .with_confidence_multiplier(
                    translation.confidences.get(index).copied().unwrap_or(1.0),
                )
            })
            .collect::<Vec<_>>();

        let payload = TranslationPayload {
            selection: Some(selection.clone()),
            source_language: snapshot.source_language.clone(),
            target_language: snapshot.target_language.clone(),
            detected_source: Some(translation.detected_source),
            captured_at: Some(captured_at),
            unchanged: false,
            provider: translation.provider_id,
            prompt_profile: translation.prompt_profile,
            blocks,
        };

        emit_translation_partial(
            &app,
            &TranslationPartialPayload {
                selection: payload.selection.clone(),
                source_language: payload.source_language.clone(),
                target_language: payload.target_language.clone(),
                detected_source: payload.detected_source.clone(),
                captured_at: payload.captured_at.clone(),
                provider: payload.provider.clone(),
                prompt_profile: payload.prompt_profile.clone(),
                stage: PartialUpdateStage::Complete,
                complete: true,
                blocks: payload.blocks.clone(),
            },
        );

        let snapshot = state.set_translation(payload.clone());
        emit_snapshot(&app, &snapshot);
        emit_translation(&app, &payload);

        tokio::time::sleep(Duration::from_millis(180)).await;
    }

    Ok(())
}

fn build_overlay_block(
    frame: &crate::capture::CapturedFrame,
    line: &OcrTextLine,
    index: usize,
    translated_text: String,
    streaming: bool,
) -> OverlayBlock {
    let (foreground, background) = estimate_colors(&frame.image, &line.rect);
    OverlayBlock {
        id: format!("block-{index}"),
        source_text: line.text.clone(),
        translated_text,
        x: line.rect.x,
        y: line.rect.y,
        width: line.rect.width.max(48),
        height: line.rect.height.max(24),
        font_size: (line.rect.height as f32 * 0.72).clamp(14.0, 42.0),
        confidence: line.confidence,
        foreground,
        background,
        align: TextAlign::Left,
        streaming,
    }
}

fn translation_cache() -> &'static Mutex<HashMap<String, AiTranslationResponse>> {
    TRANSLATION_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn build_translation_cache_key(request: &AiTranslationRequest) -> Result<String> {
    serde_json::to_string(request).context("failed to serialize translation cache key")
}

trait OverlayBlockConfidenceExt {
    fn with_confidence_multiplier(self, multiplier: f32) -> Self;
}

impl OverlayBlockConfidenceExt for OverlayBlock {
    fn with_confidence_multiplier(mut self, multiplier: f32) -> Self {
        self.confidence = (self.confidence * multiplier).clamp(0.0, 1.0);
        self
    }
}
