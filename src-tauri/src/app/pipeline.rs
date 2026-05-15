use std::{
    collections::HashMap,
    io::Cursor,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use image::{DynamicImage, ImageFormat};
use parking_lot::Mutex;
use serde_json::json;
use tauri::AppHandle;

use crate::{
    app::events::{emit_debug, emit_snapshot, emit_translation, emit_translation_partial},
    capture::{capture_region, estimate_colors, FrameSignature},
    models::{
        AiTranslationDelta, AiTranslationRequest, AiTranslationResponse, OcrRecognitionLine,
        OcrRecognitionRequest, OverlayBlock,
        PartialUpdateStage, PipelineSettings, RuntimeStatus, TextAlign,
        TranslationPartialPayload, TranslationPayload,
        VisibleLayer,
    },
    sidecar::runtime_gateway,
    state::{self, SharedState},
};

static TRANSLATION_CACHE: OnceLock<Mutex<HashMap<String, AiTranslationResponse>>> = OnceLock::new();
const AI_RETRY_COOLDOWN: Duration = Duration::from_secs(15);

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
    let mut ai_retry_after: Option<Instant> = None;
    let mut ai_error_summary: Option<String> = None;

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
        if !state.is_token_active(token) {
            break;
        }

        let signature = FrameSignature::from_image(&frame.image);
        if let Some(previous) = last_signature.as_ref() {
            if !signature.is_meaningfully_different(previous) {
                tokio::time::sleep(Duration::from_millis(180)).await;
                continue;
            }
        }
        last_signature = Some(signature);

        emit_snapshot(&app, &state.set_status(RuntimeStatus::Recognizing, "OCR"));
        let encoded_frame = encode_frame_png_base64(&frame)?;
        let recognized = runtime_gateway()
            .recognize(OcrRecognitionRequest {
            provider_id: snapshot.ocr_provider.clone(),
            image_png_base64: encoded_frame,
            source_language: snapshot.source_language.clone(),
            hint_language: detected_source_hint.clone(),
        })
        .await
        .map_err(anyhow::Error::msg)?;
        if !state.is_token_active(token) {
            break;
        }

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
            recognized.provider_id.clone(),
            snapshot.ai_provider.clone(),
            snapshot.prompt_profile.clone(),
        );
        emit_snapshot(&app, &provider_snapshot);

        if base_blocks.is_empty() {
            emit_ocr_payload(
                &app,
                &state,
                token,
                &selection,
                &snapshot.source_language,
                &snapshot.target_language,
                Some(recognized.language.clone()),
                Some(captured_at.clone()),
                recognized.provider_id.clone(),
                snapshot.prompt_profile.clone(),
                base_blocks.clone(),
            );
            tokio::time::sleep(Duration::from_millis(180)).await;
            continue;
        }

        if let Some(retry_after) = ai_retry_after {
            let now = Instant::now();
            if now < retry_after {
                emit_ocr_payload(
                    &app,
                    &state,
                    token,
                    &selection,
                    &snapshot.source_language,
                    &snapshot.target_language,
                    Some(recognized.language.clone()),
                    Some(captured_at.clone()),
                    recognized.provider_id.clone(),
                    snapshot.prompt_profile.clone(),
                    base_blocks.clone(),
                );
                let remaining = retry_after
                    .saturating_duration_since(now)
                    .as_secs()
                    .max(1);
                let warning_snapshot = state.set_status_with_error(
                    RuntimeStatus::Recognizing,
                    format!("AI unavailable · OCR visible · retry in {remaining}s"),
                    ai_error_summary
                        .clone()
                        .unwrap_or_else(|| "Ollama unavailable; keeping OCR visible.".to_string()),
                );
                emit_snapshot(&app, &warning_snapshot);
                tokio::time::sleep(Duration::from_millis(180)).await;
                continue;
            }

            ai_retry_after = None;
            ai_error_summary = None;
        }

        emit_ocr_payload(
            &app,
            &state,
            token,
            &selection,
            &snapshot.source_language,
            &snapshot.target_language,
            Some(recognized.language.clone()),
            Some(captured_at.clone()),
            recognized.provider_id.clone(),
            snapshot.prompt_profile.clone(),
            base_blocks.clone(),
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
                        generation: token,
                        selection: Some(selection_for_partial.clone()),
                        source_language: source_language_for_partial.clone(),
                        target_language: target_language_for_partial.clone(),
                        detected_source: delta.detected_source.clone(),
                        captured_at: Some(state::timestamp()),
                        visible_layer: VisibleLayer::Translation,
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
            match runtime_gateway().translate(ai_request, partial_handler).await {
                Ok(response) => {
                    if !cache_key.is_empty() {
                        translation_cache()
                            .lock()
                            .insert(cache_key.clone(), response.clone());
                    }
                    response
                }
                Err(error) => {
                    let error_text = error.to_string();
                    let error_summary = summarize_ai_error(&error_text);
                    emit_debug(
                        &app,
                        "ai-provider",
                        "sidecar translate failed",
                        json!({
                            "error": error_text,
                            "provider": snapshot.ai_provider,
                            "promptProfile": snapshot.prompt_profile,
                        }),
                    );
                    ai_retry_after = Some(Instant::now() + AI_RETRY_COOLDOWN);
                    ai_error_summary = Some(error_summary.clone());
                    emit_ocr_payload(
                        &app,
                        &state,
                        token,
                        &selection,
                        &snapshot.source_language,
                        &snapshot.target_language,
                        Some(recognized.language.clone()),
                        Some(captured_at.clone()),
                        recognized.provider_id.clone(),
                        snapshot.prompt_profile.clone(),
                        base_blocks.clone(),
                    );
                    let fallback_snapshot = state.set_status_with_error(
                        RuntimeStatus::Recognizing,
                        format!(
                            "AI unavailable · OCR visible · retry in {}s",
                            AI_RETRY_COOLDOWN.as_secs()
                        ),
                        error_summary,
                    );
                    emit_snapshot(&app, &fallback_snapshot);
                    tokio::time::sleep(Duration::from_millis(180)).await;
                    continue;
                }
            }
        };
        if !state.is_token_active(token) {
            break;
        }

        let provider_snapshot = state.set_provider_stack(
            recognized.provider_id,
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
            generation: token,
            selection: Some(selection.clone()),
            source_language: snapshot.source_language.clone(),
            target_language: snapshot.target_language.clone(),
            detected_source: Some(translation.detected_source),
            captured_at: Some(captured_at),
            unchanged: false,
            visible_layer: VisibleLayer::Translation,
            provider: translation.provider_id,
            prompt_profile: translation.prompt_profile,
            blocks,
        };

        emit_translation_partial(
            &app,
            &TranslationPartialPayload {
                generation: token,
                selection: payload.selection.clone(),
                source_language: payload.source_language.clone(),
                target_language: payload.target_language.clone(),
                detected_source: payload.detected_source.clone(),
                captured_at: payload.captured_at.clone(),
                visible_layer: VisibleLayer::Translation,
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
    line: &OcrRecognitionLine,
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
        width: line.rect.width.max(1),
        height: line.rect.height.max(1),
        font_size: (line.rect.height as f32 * 0.64).clamp(9.0, 42.0),
        confidence: line.confidence,
        foreground,
        background,
        align: TextAlign::Left,
        streaming,
    }
}

fn emit_ocr_payload(
    app: &AppHandle,
    state: &SharedState,
    generation: u64,
    selection: &crate::models::SelectionRect,
    source_language: &str,
    target_language: &str,
    detected_source: Option<String>,
    captured_at: Option<String>,
    provider: String,
    prompt_profile: String,
    blocks: Vec<OverlayBlock>,
) {
    let visible_layer = if blocks.is_empty() {
        VisibleLayer::None
    } else {
        VisibleLayer::Ocr
    };

    let payload = TranslationPayload {
        generation,
        selection: Some(selection.clone()),
        source_language: source_language.to_string(),
        target_language: target_language.to_string(),
        detected_source: detected_source.clone(),
        captured_at: captured_at.clone(),
        unchanged: false,
        visible_layer,
        provider: provider.clone(),
        prompt_profile: prompt_profile.clone(),
        blocks: blocks.clone(),
    };

    let snapshot = state.set_translation(payload.clone());
    emit_snapshot(app, &snapshot);
    emit_translation(app, &payload);
    emit_translation_partial(
        app,
        &TranslationPartialPayload {
            generation,
            selection: Some(selection.clone()),
            source_language: source_language.to_string(),
            target_language: target_language.to_string(),
            detected_source,
            captured_at,
            visible_layer,
            provider,
            prompt_profile,
            stage: PartialUpdateStage::Ocr,
            complete: false,
            blocks,
        },
    );
}

fn summarize_ai_error(error: &str) -> String {
    let headline = error.lines().next().unwrap_or("AI translation failed").trim();

    if headline.contains("did not produce response headers within") {
        return "Ollama inference stalled; keeping OCR visible.".to_string();
    }

    if headline.contains("Failed to reach Ollama endpoint") {
        return "Ollama endpoint unreachable; keeping OCR visible.".to_string();
    }

    if headline.contains("Ollama returned HTTP") {
        return headline.to_string();
    }

    "AI translation failed; keeping OCR visible.".to_string()
}

fn encode_frame_png_base64(frame: &crate::capture::CapturedFrame) -> Result<String> {
    let mut buffer = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(frame.image.clone())
        .write_to(&mut buffer, ImageFormat::Png)
        .context("failed to encode captured frame as PNG")?;
    Ok(BASE64_STANDARD.encode(buffer.into_inner()))
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
