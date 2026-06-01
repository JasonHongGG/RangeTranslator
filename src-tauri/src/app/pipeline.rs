use std::{
    io::Cursor,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use image::{DynamicImage, ImageFormat};
use serde_json::json;
use tauri::AppHandle;

use crate::{
    app::events::{emit_debug, emit_snapshot, emit_translation, emit_translation_partial},
    capture::{FrameSignature, capture_region},
    domain::{
        overlay_frame::{OverlayFrameContext, OverlayFrameScene},
        scene::{SceneBuilder, SourceSpan, canonicalize_ocr_lines},
        translation::{
            align_translation_units, build_translation_cache_key, build_translation_units,
            translation_cache, translation_unit_from_delta,
        },
    },
    models::{
        AiTranslationDelta, AiTranslationRequest, AiTranslationSourceItem,
        OcrRecognitionRequest, OverlaySourceUnit, PartialUpdateStage, PipelineSettings,
        RuntimeStatus, TranslationUnitState, VisibleLayer,
    },
    sidecar::runtime_gateway,
    state::{self, SharedState},
};

const AI_RETRY_COOLDOWN: Duration = Duration::from_secs(15);
const FRAME_IDLE_POLL_DELAY: Duration = Duration::from_millis(180);
const FRAME_CHANGE_CONFIRM_DELAY: Duration = Duration::from_millis(90);

pub fn begin_pipeline(app: &AppHandle, state: SharedState, settings: PipelineSettings) {
    let (token, snapshot) = state.start_pipeline(settings);
    emit_snapshot(app, &snapshot);
    emit_translation(app, &state.translation());

    spawn_pipeline(app, state, token);
}

pub fn spawn_pipeline(app: &AppHandle, state: SharedState, token: u64) {
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
    let mut pending_signature: Option<FrameSignature> = None;
    let mut detected_source_hint: Option<String> = None;
    let mut ai_retry_after: Option<Instant> = None;
    let mut ai_error_summary: Option<String> = None;
    let mut frame_sequence = 0_u64;

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

        let frame = capture_region(&selection)?;
        if !state.is_token_active(token) {
            break;
        }

        let signature = FrameSignature::from_image(&frame.image);
        if let Some(previous) = last_signature.as_ref() {
            if !signature.is_meaningfully_different(previous) {
                pending_signature = None;
                tokio::time::sleep(FRAME_IDLE_POLL_DELAY).await;
                continue;
            }

            if let Some(pending) = pending_signature.as_ref() {
                if signature.is_meaningfully_different(pending) {
                    pending_signature = Some(signature);
                    tokio::time::sleep(FRAME_CHANGE_CONFIRM_DELAY).await;
                    continue;
                }
            } else {
                pending_signature = Some(signature);
                tokio::time::sleep(FRAME_CHANGE_CONFIRM_DELAY).await;
                continue;
            }
        }

        pending_signature = None;
        last_signature = Some(signature);
        frame_sequence = frame_sequence.saturating_add(1);
        let frame_id = format!("{token}:{frame_sequence}");

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
        let canonical_lines = canonicalize_ocr_lines(&recognized.lines);
        if canonical_lines.len() != recognized.lines.len() {
            emit_debug(
                &app,
                "ocr-canonicalization",
                "collapsed overlapping duplicate OCR lines",
                json!({
                    "rawLineCount": recognized.lines.len(),
                    "canonicalLineCount": canonical_lines.len(),
                }),
            );
        }

        let source_spans = SceneBuilder::new(&frame, &selection, &frame_id)
            .build_source_spans(&canonical_lines);
        let source_units = overlay_source_units(&source_spans);
        let pending_translation_units = build_translation_units(
            &source_spans,
            if snapshot.ai_translation_enabled {
                TranslationUnitState::Pending
            } else {
                TranslationUnitState::Disabled
            },
            false,
        );

        let capture_metadata = frame.metadata.clone();
        let base_context = OverlayFrameContext::new(
            token,
            frame_id.clone(),
            selection.clone(),
            Some(capture_metadata.clone()),
            snapshot.source_language.clone(),
            snapshot.target_language.clone(),
            Some(recognized.language.clone()),
            Some(captured_at.clone()),
            if snapshot.ai_translation_enabled {
                snapshot.ai_provider.clone()
            } else {
                recognized.provider_id.clone()
            },
            snapshot.prompt_profile.clone(),
        );

        let provider_snapshot = state.set_provider_stack(
            recognized.provider_id.clone(),
            snapshot.ai_provider.clone(),
            snapshot.prompt_profile.clone(),
        );
        emit_snapshot(&app, &provider_snapshot);

        if !state.is_token_active(token) {
            break;
        }

        if source_units.is_empty() {
            let empty_scene = OverlayFrameScene::new(
                base_context.clone(),
                Vec::new(),
                Vec::new(),
                VisibleLayer::None,
            );
            emit_overlay_frame(&app, &state, &empty_scene, PartialUpdateStage::Ocr, false);
            tokio::time::sleep(FRAME_IDLE_POLL_DELAY).await;
            continue;
        }

        if let Some(retry_after) = ai_retry_after {
            let now = Instant::now();
            if now < retry_after {
                let retry_scene = OverlayFrameScene::new(
                    base_context.clone(),
                    source_units.clone(),
                    build_translation_units(&source_spans, TranslationUnitState::Failed, false),
                    VisibleLayer::Translation,
                );
                emit_overlay_frame(
                    &app,
                    &state,
                    &retry_scene,
                    PartialUpdateStage::Translation,
                    false,
                );
                let remaining = retry_after.saturating_duration_since(now).as_secs().max(1);
                let warning_snapshot = state.set_status_with_error(
                    RuntimeStatus::Recognizing,
                    format!("AI unavailable · retry in {remaining}s"),
                    ai_error_summary.clone().unwrap_or_else(|| {
                        "Ollama unavailable; keeping original text masked.".to_string()
                    }),
                );
                emit_snapshot(&app, &warning_snapshot);
                tokio::time::sleep(FRAME_IDLE_POLL_DELAY).await;
                continue;
            }

            ai_retry_after = None;
            ai_error_summary = None;
        }

        let pending_scene = OverlayFrameScene::new(
            base_context.clone(),
            source_units.clone(),
            pending_translation_units.clone(),
            if snapshot.ai_translation_enabled {
                VisibleLayer::Translation
            } else {
                VisibleLayer::Ocr
            },
        );
        emit_overlay_frame(&app, &state, &pending_scene, PartialUpdateStage::Ocr, false);

        if !state.is_token_active(token) {
            break;
        }

        if !snapshot.ai_translation_enabled {
            ai_retry_after = None;
            ai_error_summary = None;
            tokio::time::sleep(FRAME_IDLE_POLL_DELAY).await;
            continue;
        }

        emit_snapshot(&app, &state.set_status(RuntimeStatus::Translating, "AI"));
        let ai_items = source_units
            .iter()
            .map(|unit| AiTranslationSourceItem {
                id: unit.id.clone(),
                index: unit.order,
                text: unit.source_text.clone(),
                rect: unit.source_rect.clone(),
            })
            .collect::<Vec<_>>();
        let app_handle = app.clone();
        let partial_context = base_context.clone();
        let source_spans_for_partial = source_spans.clone();
        let source_units_for_partial = source_units.clone();
        let state_for_partial = state.clone();
        let partial_handler = Arc::new(move |delta: AiTranslationDelta| {
            if !state_for_partial.is_token_active(token) {
                return;
            }

            if let Some(translation_unit) =
                translation_unit_from_delta(&source_spans_for_partial, &delta)
            {
                let partial_scene = OverlayFrameScene::new(
                    OverlayFrameContext {
                        detected_source: delta.detected_source.clone(),
                        captured_at: Some(state::timestamp()),
                        provider: delta.provider_id.clone(),
                        prompt_profile: delta.prompt_profile.clone(),
                        ..partial_context.clone()
                    },
                    source_units_for_partial.clone(),
                    vec![translation_unit],
                    VisibleLayer::Translation,
                );
                emit_translation_partial(
                    &app_handle,
                    &partial_scene.partial(PartialUpdateStage::Translation, delta.done),
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
            expected_item_count: ai_items.len(),
            items: ai_items,
        };

        let cache_key = build_translation_cache_key(&ai_request).unwrap_or_default();
        let translation = if let Some(cached) = translation_cache().lock().get(&cache_key).cloned()
        {
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
            match runtime_gateway()
                .translate(ai_request, partial_handler)
                .await
            {
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
                    let failed_scene = OverlayFrameScene::new(
                        OverlayFrameContext {
                            provider: recognized.provider_id.clone(),
                            ..base_context.clone()
                        },
                        source_units.clone(),
                        build_translation_units(&source_spans, TranslationUnitState::Failed, false),
                        VisibleLayer::Translation,
                    );
                    emit_overlay_frame(
                        &app,
                        &state,
                        &failed_scene,
                        PartialUpdateStage::Translation,
                        false,
                    );
                    let fallback_snapshot = state.set_status_with_error(
                        RuntimeStatus::Recognizing,
                        format!("AI unavailable · retry in {}s", AI_RETRY_COOLDOWN.as_secs()),
                        error_summary,
                    );
                    emit_snapshot(&app, &fallback_snapshot);
                    tokio::time::sleep(FRAME_IDLE_POLL_DELAY).await;
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

        let translated_scene = OverlayFrameScene::new(
            OverlayFrameContext {
                detected_source: Some(translation.detected_source.clone()),
                provider: translation.provider_id.clone(),
                prompt_profile: translation.prompt_profile.clone(),
                ..base_context.clone()
            },
            source_units.clone(),
            align_translation_units(&source_spans, &translation),
            VisibleLayer::Translation,
        );
        emit_overlay_frame(
            &app,
            &state,
            &translated_scene,
            PartialUpdateStage::Complete,
            true,
        );

        tokio::time::sleep(FRAME_IDLE_POLL_DELAY).await;
    }

    Ok(())
}

fn overlay_source_units(source_spans: &[SourceSpan]) -> Vec<OverlaySourceUnit> {
    source_spans
        .iter()
        .map(SourceSpan::as_overlay_unit)
        .collect()
}

fn emit_overlay_frame(
    app: &AppHandle,
    state: &SharedState,
    frame_scene: &OverlayFrameScene,
    stage: PartialUpdateStage,
    complete: bool,
) {
    let payload = frame_scene.payload();
    if !state.is_token_active(payload.generation) {
        return;
    }

    let snapshot = state.set_translation(payload.clone());
    emit_snapshot(app, &snapshot);
    emit_translation(app, &payload);
    emit_translation_partial(app, &frame_scene.partial(stage, complete));
}

fn summarize_ai_error(error: &str) -> String {
    let headline = error
        .lines()
        .next()
        .unwrap_or("AI translation failed")
        .trim();

    if headline.contains("did not produce response headers within") {
        return "Ollama inference stalled; keeping original text masked.".to_string();
    }

    if headline.contains("Failed to reach Ollama endpoint") {
        return "Ollama endpoint unreachable; keeping original text masked.".to_string();
    }

    if headline.contains("Ollama returned HTTP") {
        return headline.to_string();
    }

    "AI translation failed; keeping original text masked.".to_string()
}

fn encode_frame_png_base64(frame: &crate::capture::CapturedFrame) -> Result<String> {
    let mut buffer = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(frame.image.clone())
        .write_to(&mut buffer, ImageFormat::Png)
        .context("failed to encode captured frame as PNG")?;
    Ok(BASE64_STANDARD.encode(buffer.into_inner()))
}

#[cfg(test)]
mod tests {
    use image::RgbaImage;

    use super::{build_translation_cache_key, overlay_source_units};
    use crate::{
        domain::{
            scene::{SceneBuilder, canonicalize_ocr_lines},
            translation::align_translation_units,
        },
        capture::CapturedFrame,
        models::{
            AiTranslationItem, AiTranslationResponse, AiTranslationRequest,
            AiTranslationSourceItem, CaptureCoordinateSpace, CaptureMetadata,
            OcrRecognitionLine, OverlaySourceUnit, PixelRect, SelectionRect,
            TranslationUnitState,
        },
    };

    fn build_units(
        frame: &CapturedFrame,
        lines: &[OcrRecognitionLine],
        selection: &SelectionRect,
        frame_id: &str,
    ) -> Vec<OverlaySourceUnit> {
        overlay_source_units(&SceneBuilder::new(frame, selection, frame_id).build_source_spans(lines))
    }

    fn test_frame() -> CapturedFrame {
        CapturedFrame {
            image: RgbaImage::from_pixel(320, 180, image::Rgba([24, 28, 32, 255])),
            metadata: CaptureMetadata {
                coordinate_space: CaptureCoordinateSpace::SelectionPhysicalPixels,
                display_origin_x: 0,
                display_origin_y: 0,
                display_width: 320,
                display_height: 180,
                capture_origin_x: 0,
                capture_origin_y: 0,
                capture_width: 320,
                capture_height: 180,
                scale_factor: 1.0,
            },
        }
    }

    fn line(text: &str, x: u32, y: u32) -> OcrRecognitionLine {
        OcrRecognitionLine {
            text: text.to_string(),
            rect: PixelRect {
                x,
                y,
                width: 80,
                height: 20,
            },
            confidence: 0.9,
        }
    }

    #[test]
    fn source_units_are_sorted_and_id_aligned() {
        let frame = test_frame();
        let units = build_units(
            &frame,
            &[
                line("second", 16, 60),
                line("first", 12, 20),
                line("third", 120, 60),
            ],
            &SelectionRect {
                x: 0,
                y: 0,
                width: 320,
                height: 180,
            },
            "7:1",
        );

        assert_eq!(
            units
                .iter()
                .map(|unit| unit.source_text.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "second", "third"]
        );
        assert_eq!(
            units
                .iter()
                .map(|unit| unit.id.as_str())
                .collect::<Vec<_>>(),
            vec!["7:1/span-0", "7:1/span-1", "7:1/span-2"]
        );
    }

    #[test]
    fn align_translation_units_marks_missing_without_source_fallback() {
        let frame = test_frame();
        let selection = SelectionRect {
            x: 0,
            y: 0,
            width: 320,
            height: 180,
        };
        let source_spans = SceneBuilder::new(&frame, &selection, "7:2")
            .build_source_spans(&[line("Settings", 12, 20), line("Pinning", 12, 50)]);
        let response = AiTranslationResponse {
            provider_id: "ollama".to_string(),
            model: "qwen3:8b".to_string(),
            prompt_profile: "translation.ui_overlay.default".to_string(),
            detected_source: "en-US".to_string(),
            items: vec![AiTranslationItem {
                id: "7:2/span-0".to_string(),
                index: 0,
                translation: "設定".to_string(),
                confidence: 0.8,
            }],
        };

        let translation_units = align_translation_units(&source_spans, &response);

        assert_eq!(translation_units[0].state, TranslationUnitState::Translated);
        assert_eq!(translation_units[0].text, "設定");
        assert_eq!(translation_units[1].state, TranslationUnitState::Missing);
        assert_eq!(translation_units[1].text, "");
    }

    #[test]
    fn source_units_normalize_capture_pixels_back_to_overlay_selection_space() {
        let frame = CapturedFrame {
            image: RgbaImage::from_pixel(720, 405, image::Rgba([24, 28, 32, 255])),
            metadata: CaptureMetadata {
                coordinate_space: CaptureCoordinateSpace::SelectionPhysicalPixels,
                display_origin_x: 0,
                display_origin_y: 0,
                display_width: 1920,
                display_height: 1080,
                capture_origin_x: 300,
                capture_origin_y: 180,
                capture_width: 720,
                capture_height: 405,
                scale_factor: 1.5,
            },
        };

        let units = build_units(
            &frame,
            &[OcrRecognitionLine {
                text: "Settings".to_string(),
                rect: PixelRect {
                    x: 90,
                    y: 60,
                    width: 180,
                    height: 45,
                },
                confidence: 0.9,
            }],
            &SelectionRect {
                x: 300,
                y: 180,
                width: 480,
                height: 270,
            },
            "7:3",
        );

        assert_eq!(units[0].source_rect.x, 60);
        assert_eq!(units[0].source_rect.y, 40);
        assert_eq!(units[0].source_rect.width, 120);
        assert_eq!(units[0].source_rect.height, 30);
        assert!(units[0].font_size >= 10.0);
        assert!(units[0].line_height >= units[0].font_size);
    }

    #[test]
    fn canonicalize_ocr_lines_merges_overlapping_duplicate_text() {
        let canonical = canonicalize_ocr_lines(&[
            OcrRecognitionLine {
                text: "Settings Layering Restored".to_string(),
                rect: PixelRect {
                    x: 10,
                    y: 20,
                    width: 180,
                    height: 28,
                },
                confidence: 0.8,
            },
            OcrRecognitionLine {
                text: "Settings Layering Restored".to_string(),
                rect: PixelRect {
                    x: 16,
                    y: 18,
                    width: 176,
                    height: 32,
                },
                confidence: 0.92,
            },
            OcrRecognitionLine {
                text: "Synchronized Pinning".to_string(),
                rect: PixelRect {
                    x: 12,
                    y: 64,
                    width: 168,
                    height: 28,
                },
                confidence: 0.88,
            },
        ]);

        assert_eq!(canonical.len(), 2);
        assert_eq!(canonical[0].text, "Settings Layering Restored");
        assert_eq!(canonical[0].rect.x, 10);
        assert_eq!(canonical[0].rect.y, 18);
        assert_eq!(canonical[0].rect.width, 182);
        assert_eq!(canonical[0].rect.height, 32);
        assert_eq!(canonical[0].confidence, 0.92);
    }

    #[test]
    fn canonicalize_ocr_lines_keeps_adjacent_same_text_regions_separate() {
        let canonical = canonicalize_ocr_lines(&[
            OcrRecognitionLine {
                text: "General".to_string(),
                rect: PixelRect {
                    x: 10,
                    y: 20,
                    width: 76,
                    height: 24,
                },
                confidence: 0.88,
            },
            OcrRecognitionLine {
                text: "General".to_string(),
                rect: PixelRect {
                    x: 92,
                    y: 22,
                    width: 74,
                    height: 24,
                },
                confidence: 0.9,
            },
        ]);

        assert_eq!(canonical.len(), 2);
    }

    #[test]
    fn translation_cache_key_ignores_frame_scoped_span_ids() {
        let request_a = AiTranslationRequest {
            endpoint: "http://127.0.0.1:11434".to_string(),
            provider_id: "ollama".to_string(),
            model: "qwen3:8b".to_string(),
            prompt_profile: "translation.ui_overlay.default".to_string(),
            source_language: "en-US".to_string(),
            target_language: "zh-TW".to_string(),
            expected_item_count: 1,
            items: vec![AiTranslationSourceItem {
                id: "7:4/span-0".to_string(),
                index: 0,
                text: "General".to_string(),
                rect: PixelRect {
                    x: 10,
                    y: 20,
                    width: 76,
                    height: 24,
                },
            }],
        };

        let mut request_b = request_a.clone();
        request_b.items[0].id = "7:5/span-0".to_string();

        assert_eq!(
            build_translation_cache_key(&request_a).unwrap(),
            build_translation_cache_key(&request_b).unwrap(),
        );
    }
}
