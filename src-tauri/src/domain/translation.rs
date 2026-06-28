use std::{collections::HashMap, sync::OnceLock};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde_json::json;

use crate::models::{
    AiTranslationDelta, AiTranslationRequest, AiTranslationResponse, OverlayTranslationUnit,
    TranslationUnitState,
};

use super::scene::SourceSpan;

static TRANSLATION_CACHE: OnceLock<Mutex<HashMap<String, AiTranslationResponse>>> = OnceLock::new();

pub fn translation_cache() -> &'static Mutex<HashMap<String, AiTranslationResponse>> {
    TRANSLATION_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn build_translation_units(
    source_spans: &[SourceSpan],
    state: TranslationUnitState,
    streaming: bool,
) -> Vec<OverlayTranslationUnit> {
    source_spans
        .iter()
        .map(|span| OverlayTranslationUnit {
            source_id: span.id.clone(),
            order: span.order,
            text: String::new(),
            state,
            confidence: span.confidence,
            streaming,
        })
        .collect()
}

pub fn align_translation_units(
    source_spans: &[SourceSpan],
    translation: &AiTranslationResponse,
) -> Vec<OverlayTranslationUnit> {
    let translated_by_source = translation
        .items
        .iter()
        .map(|item| ((item.id.as_str(), item.index), item))
        .collect::<HashMap<_, _>>();

    source_spans
        .iter()
        .map(|span| {
            if let Some(item) = translated_by_source.get(&(span.id.as_str(), span.order)) {
                let text = item.translation.trim().to_string();
                return OverlayTranslationUnit {
                    source_id: span.id.clone(),
                    order: span.order,
                    text,
                    state: if item.translation.trim().is_empty() {
                        TranslationUnitState::Missing
                    } else {
                        TranslationUnitState::Translated
                    },
                    confidence: (span.confidence * item.confidence).clamp(0.0, 1.0),
                    streaming: false,
                };
            }

            OverlayTranslationUnit {
                source_id: span.id.clone(),
                order: span.order,
                text: String::new(),
                state: TranslationUnitState::Missing,
                confidence: span.confidence,
                streaming: false,
            }
        })
        .collect()
}

pub fn translation_unit_from_delta(
    source_spans: &[SourceSpan],
    delta: &AiTranslationDelta,
) -> Option<OverlayTranslationUnit> {
    let source_span = source_spans
        .iter()
        .find(|span| span.id == delta.source_id && span.order == delta.index)?;

    Some(OverlayTranslationUnit {
        source_id: source_span.id.clone(),
        order: source_span.order,
        text: delta.translated_text.clone(),
        state: if delta.translated_text.trim().is_empty() {
            TranslationUnitState::Missing
        } else {
            TranslationUnitState::Translated
        },
        confidence: (source_span.confidence * delta.confidence.unwrap_or(1.0)).clamp(0.0, 1.0),
        streaming: !delta.done,
    })
}

pub fn build_translation_cache_key(request: &AiTranslationRequest) -> Result<String> {
    serde_json::to_string(&json!({
        "endpoint": request.endpoint,
        "providerId": request.provider_id,
        "model": request.model,
        "sourceLanguage": request.source_language,
        "targetLanguage": request.target_language,
        "expectedItemCount": request.expected_item_count,
        "items": request
            .items
            .iter()
            .map(|item| {
                json!({
                    "index": item.index,
                    "text": item.text,
                    "rect": item.rect,
                })
            })
            .collect::<Vec<_>>(),
    }))
    .context("failed to serialize translation cache key")
}
