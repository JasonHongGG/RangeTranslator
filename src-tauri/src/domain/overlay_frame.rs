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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CaptureCoordinateSpace, PixelRect, TextAlign, TranslationUnitState};

    fn context() -> OverlayFrameContext {
        OverlayFrameContext::new(
            7,
            "7:3".to_string(),
            SelectionRect {
                x: 10,
                y: 20,
                width: 320,
                height: 180,
            },
            Some(CaptureMetadata {
                coordinate_space: CaptureCoordinateSpace::SelectionPhysicalPixels,
                display_origin_x: 0,
                display_origin_y: 0,
                display_width: 1920,
                display_height: 1080,
                capture_origin_x: 10,
                capture_origin_y: 20,
                capture_width: 320,
                capture_height: 180,
                scale_factor: 1.25,
            }),
            "en-US".to_string(),
            "zh-TW".to_string(),
            Some("en-US".to_string()),
            Some("2026-06-02T12:00:00.000Z".to_string()),
            "ollama".to_string(),
        )
    }

    fn source_unit() -> OverlaySourceUnit {
        OverlaySourceUnit {
            id: "7:3/span-0".to_string(),
            frame_id: "7:3".to_string(),
            order: 0,
            source_text: "General".to_string(),
            source_rect: PixelRect {
                x: 24,
                y: 12,
                width: 80,
                height: 20,
            },
            font_size: 20.0,
            line_height: 24.0,
            confidence: 0.94,
            foreground: "#F7F8FA".to_string(),
            background: "#2965A8".to_string(),
            style_confidence: 0.88,
            align: TextAlign::Left,
        }
    }

    fn translation_unit() -> OverlayTranslationUnit {
        OverlayTranslationUnit {
            source_id: "7:3/span-0".to_string(),
            order: 0,
            text: "一般".to_string(),
            state: TranslationUnitState::Translated,
            confidence: 0.91,
            streaming: false,
        }
    }

    #[test]
    fn payload_preserves_frame_context_and_style_confidence() {
        let scene = OverlayFrameScene::new(
            context(),
            vec![source_unit()],
            vec![translation_unit()],
            VisibleLayer::Translation,
        );

        let payload = scene.payload();

        assert_eq!(payload.frame_id, "7:3");
        assert_eq!(payload.visible_layer, VisibleLayer::Translation);
        assert_eq!(payload.source_units[0].style_confidence, 0.88);
        assert_eq!(payload.translation_units[0].text, "一般");
    }

    #[test]
    fn partial_resolves_to_none_when_the_frame_has_no_source_units() {
        let scene = OverlayFrameScene::new(context(), Vec::new(), Vec::new(), VisibleLayer::Translation);

        let payload = scene.payload();
        let partial = scene.partial(PartialUpdateStage::Ocr, false);

        assert_eq!(payload.visible_layer, VisibleLayer::None);
        assert_eq!(partial.visible_layer, VisibleLayer::None);
        assert_eq!(partial.frame_id, "7:3");
        assert_eq!(partial.stage, PartialUpdateStage::Ocr);
        assert!(!partial.complete);
    }
}
