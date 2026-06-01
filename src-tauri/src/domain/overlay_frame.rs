use crate::models::{
    CaptureMetadata, OverlaySourceUnit, OverlayTranslationUnit, PartialUpdateStage, SelectionRect,
    TranslationPartialPayload, TranslationPayload, VisibleLayer,
};

#[derive(Debug, Clone)]
pub struct OverlayFrameContext {
    pub generation: u64,
    pub frame_id: String,
    pub selection: SelectionRect,
    pub capture: Option<CaptureMetadata>,
    pub source_language: String,
    pub target_language: String,
    pub detected_source: Option<String>,
    pub captured_at: Option<String>,
    pub provider: String,
    pub prompt_profile: String,
}

impl OverlayFrameContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        generation: u64,
        frame_id: String,
        selection: SelectionRect,
        capture: Option<CaptureMetadata>,
        source_language: String,
        target_language: String,
        detected_source: Option<String>,
        captured_at: Option<String>,
        provider: String,
        prompt_profile: String,
    ) -> Self {
        Self {
            generation,
            frame_id,
            selection,
            capture,
            source_language,
            target_language,
            detected_source,
            captured_at,
            provider,
            prompt_profile,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OverlayFrameScene {
    context: OverlayFrameContext,
    source_units: Vec<OverlaySourceUnit>,
    translation_units: Vec<OverlayTranslationUnit>,
    visible_layer: VisibleLayer,
}

impl OverlayFrameScene {
    pub fn new(
        context: OverlayFrameContext,
        source_units: Vec<OverlaySourceUnit>,
        translation_units: Vec<OverlayTranslationUnit>,
        visible_layer: VisibleLayer,
    ) -> Self {
        Self {
            context,
            source_units,
            translation_units,
            visible_layer,
        }
    }

    pub fn payload(&self) -> TranslationPayload {
        TranslationPayload {
            generation: self.context.generation,
            frame_id: self.context.frame_id.clone(),
            selection: Some(self.context.selection.clone()),
            capture: self.context.capture.clone(),
            source_language: self.context.source_language.clone(),
            target_language: self.context.target_language.clone(),
            detected_source: self.context.detected_source.clone(),
            captured_at: self.context.captured_at.clone(),
            unchanged: false,
            visible_layer: self.resolved_visible_layer(),
            provider: self.context.provider.clone(),
            prompt_profile: self.context.prompt_profile.clone(),
            source_units: self.source_units.clone(),
            translation_units: self.translation_units.clone(),
        }
    }

    pub fn partial(&self, stage: PartialUpdateStage, complete: bool) -> TranslationPartialPayload {
        TranslationPartialPayload {
            generation: self.context.generation,
            frame_id: self.context.frame_id.clone(),
            selection: Some(self.context.selection.clone()),
            capture: self.context.capture.clone(),
            source_language: self.context.source_language.clone(),
            target_language: self.context.target_language.clone(),
            detected_source: self.context.detected_source.clone(),
            captured_at: self.context.captured_at.clone(),
            visible_layer: self.resolved_visible_layer(),
            provider: self.context.provider.clone(),
            prompt_profile: self.context.prompt_profile.clone(),
            stage,
            complete,
            source_units: self.source_units.clone(),
            translation_units: self.translation_units.clone(),
        }
    }

    fn resolved_visible_layer(&self) -> VisibleLayer {
        if self.source_units.is_empty() {
            VisibleLayer::None
        } else {
            self.visible_layer
        }
    }
}
