use std::sync::Arc;

use chrono::{SecondsFormat, Utc};
use parking_lot::Mutex;

use crate::models::{
    PipelineSettings, RuntimeSnapshot, RuntimeStatus, SelectionRect, TranslationPayload,
    VisibleLayer,
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
        inner.snapshot.running && inner.pipeline_token == token
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

    pub fn set_copy_mode(&self, enabled: bool) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.copy_mode = enabled;
        inner.snapshot.clone()
    }

    pub fn set_panel_pinned(&self, enabled: bool) -> RuntimeSnapshot {
        let mut inner = self.inner.lock();
        inner.snapshot.panel_pinned = enabled;
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
                inner.snapshot.status = RuntimeStatus::Ready;
                inner.snapshot.status_detail = if inner.snapshot.running {
                    "Translation visible".to_string()
                } else {
                    "Translation ready".to_string()
                };
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
        inner.snapshot.block_count = payload.blocks.len();
        inner.snapshot.last_updated = payload.captured_at.clone();
        inner.snapshot.last_detected_source = payload.detected_source.clone();
        inner.snapshot.last_error = None;
        if payload.visible_layer == VisibleLayer::Translation {
            inner.snapshot.ai_provider = payload.provider.clone();
            inner.snapshot.prompt_profile = payload.prompt_profile.clone();
        }
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
