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
    capture::{FrameSignature, capture_region, estimate_colors},
    models::{
        AiTranslationDelta, AiTranslationRequest, AiTranslationResponse, AiTranslationSourceItem,
        CaptureMetadata, OcrRecognitionLine, OcrRecognitionRequest, OverlaySourceUnit,
        OverlayTranslationUnit, PartialUpdateStage, PipelineSettings, PixelRect, RuntimeStatus,
        SelectionRect, TextAlign, TranslationPartialPayload, TranslationPayload,
        TranslationUnitState, VisibleLayer,
    },
    sidecar::runtime_gateway,
    state::{self, SharedState},
};

static TRANSLATION_CACHE: OnceLock<Mutex<HashMap<String, AiTranslationResponse>>> = OnceLock::new();
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

        let source_units = build_source_units(&frame, &canonical_lines, &selection);
        let pending_translation_units = build_translation_units(
            &source_units,
            if snapshot.ai_translation_enabled {
                TranslationUnitState::Pending
            } else {
                TranslationUnitState::Disabled
            },
            false,
        );

        let capture_metadata = frame.metadata.clone();

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
            emit_overlay_payload(
                &app,
                &state,
                token,
                &selection,
                Some(capture_metadata.clone()),
                &snapshot.source_language,
                &snapshot.target_language,
                Some(recognized.language.clone()),
                Some(captured_at.clone()),
                recognized.provider_id.clone(),
                snapshot.prompt_profile.clone(),
                Vec::new(),
                Vec::new(),
                VisibleLayer::None,
                PartialUpdateStage::Ocr,
                false,
            );
            tokio::time::sleep(FRAME_IDLE_POLL_DELAY).await;
            continue;
        }

        if let Some(retry_after) = ai_retry_after {
            let now = Instant::now();
            if now < retry_after {
                emit_overlay_payload(
                    &app,
                    &state,
                    token,
                    &selection,
                    Some(capture_metadata.clone()),
                    &snapshot.source_language,
                    &snapshot.target_language,
                    Some(recognized.language.clone()),
                    Some(captured_at.clone()),
                    snapshot.ai_provider.clone(),
                    snapshot.prompt_profile.clone(),
                    source_units.clone(),
                    build_translation_units(&source_units, TranslationUnitState::Failed, false),
                    VisibleLayer::Translation,
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

        emit_overlay_payload(
            &app,
            &state,
            token,
            &selection,
            Some(capture_metadata.clone()),
            &snapshot.source_language,
            &snapshot.target_language,
            Some(recognized.language.clone()),
            Some(captured_at.clone()),
            if snapshot.ai_translation_enabled {
                snapshot.ai_provider.clone()
            } else {
                recognized.provider_id.clone()
            },
            snapshot.prompt_profile.clone(),
            source_units.clone(),
            pending_translation_units.clone(),
            if snapshot.ai_translation_enabled {
                VisibleLayer::Translation
            } else {
                VisibleLayer::Ocr
            },
            PartialUpdateStage::Ocr,
            false,
        );

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
                rect: PixelRect {
                    x: unit.x,
                    y: unit.y,
                    width: unit.width,
                    height: unit.height,
                },
            })
            .collect::<Vec<_>>();
        let app_handle = app.clone();
        let selection_for_partial = selection.clone();
        let capture_for_partial = capture_metadata.clone();
        let source_language_for_partial = snapshot.source_language.clone();
        let target_language_for_partial = snapshot.target_language.clone();
        let source_units_for_partial = source_units.clone();
        let state_for_partial = state.clone();
        let partial_handler = Arc::new(move |delta: AiTranslationDelta| {
            if !state_for_partial.is_token_active(token) {
                return;
            }

            if let Some(source_unit) = source_units_for_partial
                .iter()
                .find(|unit| unit.id == delta.source_id && unit.order == delta.index)
                .cloned()
            {
                let translation_unit = OverlayTranslationUnit {
                    source_id: source_unit.id,
                    order: source_unit.order,
                    text: delta.translated_text.clone(),
                    state: if delta.translated_text.trim().is_empty() {
                        TranslationUnitState::Missing
                    } else {
                        TranslationUnitState::Translated
                    },
                    confidence: (source_unit.confidence * delta.confidence.unwrap_or(1.0))
                        .clamp(0.0, 1.0),
                    streaming: !delta.done,
                };

                emit_translation_partial(
                    &app_handle,
                    &TranslationPartialPayload {
                        generation: token,
                        selection: Some(selection_for_partial.clone()),
                        capture: Some(capture_for_partial.clone()),
                        source_language: source_language_for_partial.clone(),
                        target_language: target_language_for_partial.clone(),
                        detected_source: delta.detected_source.clone(),
                        captured_at: Some(state::timestamp()),
                        visible_layer: VisibleLayer::Translation,
                        provider: delta.provider_id.clone(),
                        prompt_profile: delta.prompt_profile.clone(),
                        stage: PartialUpdateStage::Translation,
                        complete: delta.done,
                        source_units: source_units_for_partial.clone(),
                        translation_units: vec![translation_unit],
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
                    emit_overlay_payload(
                        &app,
                        &state,
                        token,
                        &selection,
                        Some(capture_metadata.clone()),
                        &snapshot.source_language,
                        &snapshot.target_language,
                        Some(recognized.language.clone()),
                        Some(captured_at.clone()),
                        recognized.provider_id.clone(),
                        snapshot.prompt_profile.clone(),
                        source_units.clone(),
                        build_translation_units(&source_units, TranslationUnitState::Failed, false),
                        VisibleLayer::Translation,
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

        let translation_units = align_translation_units(&source_units, &translation);

        let payload = TranslationPayload {
            generation: token,
            selection: Some(selection.clone()),
            capture: Some(capture_metadata.clone()),
            source_language: snapshot.source_language.clone(),
            target_language: snapshot.target_language.clone(),
            detected_source: Some(translation.detected_source),
            captured_at: Some(captured_at),
            unchanged: false,
            visible_layer: VisibleLayer::Translation,
            provider: translation.provider_id,
            prompt_profile: translation.prompt_profile,
            source_units: source_units.clone(),
            translation_units,
        };

        emit_translation_partial(
            &app,
            &TranslationPartialPayload {
                generation: token,
                selection: payload.selection.clone(),
                capture: payload.capture.clone(),
                source_language: payload.source_language.clone(),
                target_language: payload.target_language.clone(),
                detected_source: payload.detected_source.clone(),
                captured_at: payload.captured_at.clone(),
                visible_layer: VisibleLayer::Translation,
                provider: payload.provider.clone(),
                prompt_profile: payload.prompt_profile.clone(),
                stage: PartialUpdateStage::Complete,
                complete: true,
                source_units: payload.source_units.clone(),
                translation_units: payload.translation_units.clone(),
            },
        );

        let snapshot = state.set_translation(payload.clone());
        emit_snapshot(&app, &snapshot);
        emit_translation(&app, &payload);

        tokio::time::sleep(FRAME_IDLE_POLL_DELAY).await;
    }

    Ok(())
}

fn build_source_units(
    frame: &crate::capture::CapturedFrame,
    lines: &[OcrRecognitionLine],
    selection: &SelectionRect,
) -> Vec<OverlaySourceUnit> {
    let mut ordered_lines = lines.iter().collect::<Vec<_>>();
    ordered_lines.sort_by_key(|line| (line.rect.y, line.rect.x));

    ordered_lines
        .into_iter()
        .enumerate()
        .map(|(order, line)| build_source_unit(frame, line, order, selection))
        .collect()
}

fn canonicalize_ocr_lines(lines: &[OcrRecognitionLine]) -> Vec<OcrRecognitionLine> {
    let mut canonical = Vec::new();

    for line in lines {
        let normalized_text = normalize_ocr_text(&line.text);
        if normalized_text.is_empty() {
            continue;
        }

        if let Some(existing) = canonical
            .iter_mut()
            .find(|existing| should_merge_ocr_line(existing, line))
        {
            existing.rect = merge_pixel_rect(&existing.rect, &line.rect);
            existing.confidence = existing.confidence.max(line.confidence);
            if line.text.chars().count() > existing.text.chars().count() {
                existing.text = line.text.clone();
            }
            continue;
        }

        canonical.push(line.clone());
    }

    canonical
}

fn normalize_ocr_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}

fn should_merge_ocr_line(left: &OcrRecognitionLine, right: &OcrRecognitionLine) -> bool {
    normalize_ocr_text(&left.text) == normalize_ocr_text(&right.text)
        && rects_refer_to_same_region(&left.rect, &right.rect)
}

fn rects_refer_to_same_region(left: &PixelRect, right: &PixelRect) -> bool {
    rect_intersection_area(left, right) > 0
        && (rect_iou(left, right) >= 0.35
            || rect_contains_center(left, right)
            || rect_contains_center(right, left))
}

fn rect_intersection_area(left: &PixelRect, right: &PixelRect) -> u32 {
    let x0 = left.x.max(right.x);
    let y0 = left.y.max(right.y);
    let x1 = (left.x + left.width).min(right.x + right.width);
    let y1 = (left.y + left.height).min(right.y + right.height);

    if x1 <= x0 || y1 <= y0 {
        return 0;
    }

    (x1 - x0) * (y1 - y0)
}

fn rect_iou(left: &PixelRect, right: &PixelRect) -> f32 {
    let intersection = rect_intersection_area(left, right) as f32;
    if intersection == 0.0 {
        return 0.0;
    }

    let left_area = (left.width * left.height) as f32;
    let right_area = (right.width * right.height) as f32;
    let union = left_area + right_area - intersection;
    if union <= 0.0 {
        return 0.0;
    }

    intersection / union
}

fn rect_contains_center(container: &PixelRect, target: &PixelRect) -> bool {
    let center_x = target.x + (target.width / 2);
    let center_y = target.y + (target.height / 2);
    center_x >= container.x
        && center_x <= container.x + container.width
        && center_y >= container.y
        && center_y <= container.y + container.height
}

fn merge_pixel_rect(left: &PixelRect, right: &PixelRect) -> PixelRect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = (left.x + left.width).max(right.x + right.width);
    let bottom_edge = (left.y + left.height).max(right.y + right.height);

    PixelRect {
        x,
        y,
        width: (right_edge - x).max(1),
        height: (bottom_edge - y).max(1),
    }
}

fn build_source_unit(
    frame: &crate::capture::CapturedFrame,
    line: &OcrRecognitionLine,
    order: usize,
    selection: &SelectionRect,
) -> OverlaySourceUnit {
    let (foreground, background) = estimate_colors(&frame.image, &line.rect);
    let normalized_rect = normalize_rect_to_selection(&line.rect, frame, selection);
    OverlaySourceUnit {
        id: format!("source-{order}"),
        order,
        source_text: line.text.clone(),
        x: normalized_rect.x,
        y: normalized_rect.y,
        width: normalized_rect.width.max(1),
        height: normalized_rect.height.max(1),
        font_size: (normalized_rect.height as f32 * 0.64).clamp(9.0, 42.0),
        confidence: line.confidence,
        foreground,
        background,
        align: TextAlign::Left,
    }
}

fn normalize_rect_to_selection(
    rect: &PixelRect,
    frame: &crate::capture::CapturedFrame,
    selection: &SelectionRect,
) -> PixelRect {
    let frame_width = frame.metadata.capture_width.max(1) as f32;
    let frame_height = frame.metadata.capture_height.max(1) as f32;
    let selection_width = selection.width.max(1) as f32;
    let selection_height = selection.height.max(1) as f32;

    let left = ((rect.x as f32 / frame_width) * selection_width).round();
    let top = ((rect.y as f32 / frame_height) * selection_height).round();
    let right = ((((rect.x + rect.width) as f32) / frame_width) * selection_width).round();
    let bottom = ((((rect.y + rect.height) as f32) / frame_height) * selection_height).round();

    PixelRect {
        x: left.max(0.0) as u32,
        y: top.max(0.0) as u32,
        width: (right - left).max(1.0) as u32,
        height: (bottom - top).max(1.0) as u32,
    }
}

fn build_translation_units(
    source_units: &[OverlaySourceUnit],
    state: TranslationUnitState,
    streaming: bool,
) -> Vec<OverlayTranslationUnit> {
    source_units
        .iter()
        .map(|unit| OverlayTranslationUnit {
            source_id: unit.id.clone(),
            order: unit.order,
            text: String::new(),
            state,
            confidence: unit.confidence,
            streaming,
        })
        .collect()
}

fn align_translation_units(
    source_units: &[OverlaySourceUnit],
    translation: &AiTranslationResponse,
) -> Vec<OverlayTranslationUnit> {
    let translated_by_source = translation
        .items
        .iter()
        .map(|item| ((item.id.as_str(), item.index), item))
        .collect::<HashMap<_, _>>();

    source_units
        .iter()
        .map(|unit| {
            if let Some(item) = translated_by_source.get(&(unit.id.as_str(), unit.order)) {
                let text = item.translation.trim().to_string();
                return OverlayTranslationUnit {
                    source_id: unit.id.clone(),
                    order: unit.order,
                    text,
                    state: if item.translation.trim().is_empty() {
                        TranslationUnitState::Missing
                    } else {
                        TranslationUnitState::Translated
                    },
                    confidence: (unit.confidence * item.confidence).clamp(0.0, 1.0),
                    streaming: false,
                };
            }

            OverlayTranslationUnit {
                source_id: unit.id.clone(),
                order: unit.order,
                text: String::new(),
                state: TranslationUnitState::Missing,
                confidence: unit.confidence,
                streaming: false,
            }
        })
        .collect()
}

fn emit_overlay_payload(
    app: &AppHandle,
    state: &SharedState,
    generation: u64,
    selection: &crate::models::SelectionRect,
    capture: Option<CaptureMetadata>,
    source_language: &str,
    target_language: &str,
    detected_source: Option<String>,
    captured_at: Option<String>,
    provider: String,
    prompt_profile: String,
    source_units: Vec<OverlaySourceUnit>,
    translation_units: Vec<OverlayTranslationUnit>,
    visible_layer: VisibleLayer,
    stage: PartialUpdateStage,
    complete: bool,
) {
    if !state.is_token_active(generation) {
        return;
    }

    let visible_layer = if source_units.is_empty() {
        VisibleLayer::None
    } else {
        visible_layer
    };

    let payload = TranslationPayload {
        generation,
        selection: Some(selection.clone()),
        capture: capture.clone(),
        source_language: source_language.to_string(),
        target_language: target_language.to_string(),
        detected_source: detected_source.clone(),
        captured_at: captured_at.clone(),
        unchanged: false,
        visible_layer,
        provider: provider.clone(),
        prompt_profile: prompt_profile.clone(),
        source_units: source_units.clone(),
        translation_units: translation_units.clone(),
    };

    let snapshot = state.set_translation(payload.clone());
    emit_snapshot(app, &snapshot);
    emit_translation(app, &payload);
    emit_translation_partial(
        app,
        &TranslationPartialPayload {
            generation,
            selection: Some(selection.clone()),
            capture,
            source_language: source_language.to_string(),
            target_language: target_language.to_string(),
            detected_source,
            captured_at,
            visible_layer,
            provider,
            prompt_profile,
            stage,
            complete,
            source_units,
            translation_units,
        },
    );
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

fn translation_cache() -> &'static Mutex<HashMap<String, AiTranslationResponse>> {
    TRANSLATION_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn build_translation_cache_key(request: &AiTranslationRequest) -> Result<String> {
    serde_json::to_string(request).context("failed to serialize translation cache key")
}

#[cfg(test)]
mod tests {
    use image::RgbaImage;

    use super::*;
    use crate::{
        capture::CapturedFrame,
        models::{AiTranslationItem, CaptureCoordinateSpace, CaptureMetadata, PixelRect},
    };

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
        let units = build_source_units(
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
            vec!["source-0", "source-1", "source-2"]
        );
    }

    #[test]
    fn align_translation_units_marks_missing_without_source_fallback() {
        let frame = test_frame();
        let source_units = build_source_units(
            &frame,
            &[line("Settings", 12, 20), line("Pinning", 12, 50)],
            &SelectionRect {
                x: 0,
                y: 0,
                width: 320,
                height: 180,
            },
        );
        let response = AiTranslationResponse {
            provider_id: "ollama".to_string(),
            model: "qwen3:8b".to_string(),
            prompt_profile: "translation.ui_overlay.default".to_string(),
            detected_source: "en-US".to_string(),
            items: vec![AiTranslationItem {
                id: "source-0".to_string(),
                index: 0,
                translation: "設定".to_string(),
                confidence: 0.8,
            }],
        };

        let translation_units = align_translation_units(&source_units, &response);

        assert_eq!(translation_units[0].state, TranslationUnitState::Translated);
        assert_eq!(translation_units[0].text, "設定");
        assert_eq!(translation_units[1].state, TranslationUnitState::Missing);
        assert_eq!(translation_units[1].text, "");
    }

    #[test]
    fn source_units_normalize_capture_pixels_back_to_selection_space() {
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

        let units = build_source_units(
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
        );

        assert_eq!(units[0].x, 60);
        assert_eq!(units[0].y, 40);
        assert_eq!(units[0].width, 120);
        assert_eq!(units[0].height, 30);
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
}
