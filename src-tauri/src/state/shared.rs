use std::sync::Arc;

use chrono::{SecondsFormat, Utc};
use parking_lot::Mutex;

use crate::models::{
    OverlayInteractionMode, PipelineSettings, RuntimeSnapshot, RuntimeStatus, SelectionRect,
    TranslationPayload, VisibleLayer,
};

#[derive(Clone)]
pub struct SharedState {
    inner: Arc<Mutex<AppRuntime>>,
}

struct AppRuntime {
    snapshot: RuntimeSnapshot,
    translation: TranslationPayload,
    pipeline_token: u64,
}

impl SharedState {
    pub fn new(endpoint: String, model: String) -> Self {
        let snapshot = RuntimeSnapshot {
            endpoint,
            model,
            ..RuntimeSnapshot::default()
        };

        Self {
            inner: Arc::new(Mutex::new(AppRuntime {
                snapshot,
                translation: TranslationPayload::default(),
                pipeline_token: 0,
            })),
        }
    }

    pub fn inner_clone(&self) -> Self {
        self.clone()
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        self.inner.lock().snapshot.clone()
    }

    pub fn translation(&self) -> TranslationPayload {
        self.inner.lock().translation.clone()
    }

    pub fn activate_selector(&self, bounds: SelectionRect) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.status = RuntimeStatus::Selecting;
        inner.snapshot.status_detail = "Drag".to_string();
        inner.snapshot.selector_bounds = Some(bounds);
        inner.snapshot.last_error = None;
        inner.snapshot.clone()
    }

    pub fn cancel_selector(&self) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        if inner.snapshot.running {
            inner.snapshot.status = RuntimeStatus::Capturing;
            inner.snapshot.status_detail = "Live".to_string();
        } else if inner.snapshot.selection.is_some() {
            inner.snapshot.status = RuntimeStatus::Ready;
            inner.snapshot.status_detail = "Region locked".to_string();
        } else {
            inner.snapshot.status = RuntimeStatus::Idle;
            inner.snapshot.status_detail = "Ready".to_string();
        }
        inner.snapshot.selector_bounds = None;
        inner.snapshot.clone()
    }

    pub fn set_selection(&self, selection: SelectionRect) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.selection = Some(selection.clone());
        inner.snapshot.selector_bounds = None;
        inner.translation.selection = Some(selection);
        inner.snapshot.last_error = None;
        if inner.snapshot.running {
            inner.snapshot.status = RuntimeStatus::Capturing;
            inner.snapshot.status_detail = "Live".to_string();
        } else {
            inner.snapshot.status = RuntimeStatus::Ready;
            inner.snapshot.status_detail = "Region locked".to_string();
        }
        inner.snapshot.clone()
    }

    pub fn clear_selection(&self) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.pipeline_token = inner.pipeline_token.saturating_add(1);
        inner.translation = TranslationPayload {
            generation: inner.pipeline_token,
            frame_id: String::new(),
            ..TranslationPayload::default()
        };
        inner.snapshot.running = false;
        inner.snapshot.selection = None;
        inner.snapshot.selector_bounds = None;
        inner.snapshot.generation = inner.pipeline_token;
        inner.snapshot.visible_layer = VisibleLayer::None;
        inner.snapshot.block_count = 0;
        inner.snapshot.last_updated = None;
        inner.snapshot.last_detected_source = None;
        inner.snapshot.last_error = None;
        inner.snapshot.status = RuntimeStatus::Idle;
        inner.snapshot.status_detail = "Ready".to_string();
        inner.snapshot.clone()
    }

    pub fn start_pipeline(&self, settings: PipelineSettings) -> (u64, RuntimeSnapshot) {
        let mut inner = self.inner.lock();
        inner.pipeline_token = inner.pipeline_token.saturating_add(1);
        inner.snapshot.running = true;
        inner.snapshot.source_language = settings.source_language.clone();
        inner.snapshot.target_language = settings.target_language.clone();
        inner.snapshot.generation = inner.pipeline_token;
        inner.snapshot.visible_layer = VisibleLayer::None;
        inner.translation = TranslationPayload {
            generation: inner.pipeline_token,
            selection: inner.snapshot.selection.clone(),
            capture: None,
            frame_id: String::new(),
            source_language: settings.source_language,
            target_language: settings.target_language,
            visible_layer: VisibleLayer::None,
            ..TranslationPayload::default()
        };
        inner.snapshot.status = RuntimeStatus::Capturing;
        inner.snapshot.status_detail = "Sampling".to_string();
        inner.snapshot.last_error = None;
        (inner.pipeline_token, inner.snapshot.clone())
    }

    pub fn restart_pipeline_with_selection(
        &self,
        selection: SelectionRect,
    ) -> (u64, RuntimeSnapshot, TranslationPayload) {
        let mut inner = self.inner.lock();
        inner.pipeline_token = inner.pipeline_token.saturating_add(1);
        inner.snapshot.running = true;
        inner.snapshot.selection = Some(selection.clone());
        inner.snapshot.selector_bounds = None;
        inner.snapshot.generation = inner.pipeline_token;
        inner.snapshot.visible_layer = VisibleLayer::None;
        inner.snapshot.block_count = 0;
        inner.snapshot.last_updated = None;
        inner.snapshot.last_detected_source = None;
        inner.snapshot.last_error = None;
        inner.snapshot.status = RuntimeStatus::Capturing;
        inner.snapshot.status_detail = "Sampling".to_string();
        inner.translation = TranslationPayload {
            generation: inner.pipeline_token,
            selection: Some(selection),
            capture: None,
            frame_id: String::new(),
            source_language: inner.snapshot.source_language.clone(),
            target_language: inner.snapshot.target_language.clone(),
            visible_layer: VisibleLayer::None,
            ..TranslationPayload::default()
        };
        (
            inner.pipeline_token,
            inner.snapshot.clone(),
            inner.translation.clone(),
        )
    }

    pub fn stop_pipeline(&self) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.pipeline_token = inner.pipeline_token.saturating_add(1);
        inner.snapshot.running = false;
        if inner.snapshot.selection.is_some() {
            inner.snapshot.status = RuntimeStatus::Ready;
            inner.snapshot.status_detail = "Region locked".to_string();
        } else {
            inner.snapshot.status = RuntimeStatus::Idle;
            inner.snapshot.status_detail = "Ready".to_string();
        }
        inner.snapshot.clone()
    }

    pub fn is_token_active(&self, token: u64) -> bool {
        let inner = self.inner.lock();
        inner.snapshot.running
            && inner.pipeline_token == token
            && !inner.snapshot.debug_screenshot_mode
    }

    pub fn set_status(&self, status: RuntimeStatus, detail: impl Into<String>) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.status = status;
        inner.snapshot.status_detail = detail.into();
        if status != RuntimeStatus::Error {
            inner.snapshot.last_error = None;
        }
        inner.snapshot.clone()
    }

    pub fn set_status_with_error(
        &self,
        status: RuntimeStatus,
        detail: impl Into<String>,
        message: impl Into<String>,
    ) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.status = status;
        inner.snapshot.status_detail = detail.into();
        inner.snapshot.last_error = Some(message.into());
        inner.snapshot.clone()
    }

    pub fn set_model(&self, model: String) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.model = model;
        inner.snapshot.clone()
    }

    pub fn set_overlay_mode(&self, mode: OverlayInteractionMode) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.overlay_mode = mode;
        inner.snapshot.clone()
    }

    pub fn set_panel_pinned(&self, enabled: bool) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.panel_pinned = enabled;
        inner.snapshot.clone()
    }

    pub fn set_ai_translation_enabled(&self, enabled: bool) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.ai_translation_enabled = enabled;
        inner.snapshot.clone()
    }

    pub fn set_debug_screenshot_mode(&self, enabled: bool) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.debug_screenshot_mode = enabled;
        inner.snapshot.clone()
    }

    pub fn set_provider_stack(
        &self,
        ocr_provider: String,
        ai_provider: String,
        prompt_profile: String,
    ) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.ocr_provider = ocr_provider;
        inner.snapshot.ai_provider = ai_provider;
        inner.snapshot.prompt_profile = prompt_profile;
        inner.snapshot.clone()
    }

    pub fn set_translation(&self, payload: TranslationPayload) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.generation = payload.generation;
        inner.snapshot.visible_layer = payload.visible_layer;
        match payload.visible_layer {
            VisibleLayer::Translation => {
                if inner.snapshot.running {
                    inner.snapshot.status = RuntimeStatus::Capturing;
                    inner.snapshot.status_detail = "Live".to_string();
                } else {
                    inner.snapshot.status = RuntimeStatus::Ready;
                    inner.snapshot.status_detail = "Translation ready".to_string();
                }
            }
            VisibleLayer::Ocr => {
                inner.snapshot.status = RuntimeStatus::Recognizing;
                inner.snapshot.status_detail = "OCR visible".to_string();
            }
            VisibleLayer::None => {
                inner.snapshot.status = RuntimeStatus::Recognizing;
                inner.snapshot.status_detail = "No text detected".to_string();
            }
        }
        inner.snapshot.block_count = payload.source_units.len();
        inner.snapshot.last_updated = payload.captured_at.clone();
        inner.snapshot.last_detected_source = payload.detected_source.clone();
        inner.snapshot.last_error = None;
        inner.translation = payload;
        inner.snapshot.clone()
    }

    pub fn set_error(&self, message: String) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.status = RuntimeStatus::Error;
        inner.snapshot.status_detail = "Issue".to_string();
        inner.snapshot.last_error = Some(message);
        inner.snapshot.clone()
    }
}

pub fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_payload_does_not_overwrite_ai_provider_stack() {
        let state = SharedState::new("http://localhost:11434".to_string(), "qwen3:8b".to_string());

        state.set_provider_stack(
            "paddleocr".to_string(),
            "ollama".to_string(),
            "translation.ui_overlay.default".to_string(),
        );

        let snapshot = state.set_translation(TranslationPayload {
            generation: 1,
            frame_id: "7:1".to_string(),
            visible_layer: VisibleLayer::Translation,
            provider: "paddleocr".to_string(),
            prompt_profile: String::new(),
            ..TranslationPayload::default()
        });

        assert_eq!(snapshot.ocr_provider, "paddleocr");
        assert_eq!(snapshot.ai_provider, "ollama");
        assert_eq!(snapshot.prompt_profile, "translation.ui_overlay.default");
    }
}
