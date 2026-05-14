use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeStatus {
    #[default]
    Idle,
    Selecting,
    Capturing,
    Recognizing,
    Translating,
    Ready,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PartialUpdateStage {
    #[default]
    Ocr,
    Translation,
    Complete,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VisibleLayer {
    #[default]
    None,
    Ocr,
    Translation,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayBlock {
    pub id: String,
    pub source_text: String,
    pub translated_text: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub font_size: f32,
    pub confidence: f32,
    pub foreground: String,
    pub background: String,
    pub align: TextAlign,
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationPayload {
    pub generation: u64,
    pub selection: Option<SelectionRect>,
    pub source_language: String,
    pub target_language: String,
    pub detected_source: Option<String>,
    pub captured_at: Option<String>,
    pub unchanged: bool,
    pub visible_layer: VisibleLayer,
    pub provider: String,
    pub prompt_profile: String,
    pub blocks: Vec<OverlayBlock>,
}

impl Default for TranslationPayload {
    fn default() -> Self {
        Self {
            generation: 0,
            selection: None,
            source_language: "auto".to_string(),
            target_language: "zh-TW".to_string(),
            detected_source: None,
            captured_at: None,
            unchanged: false,
            visible_layer: VisibleLayer::None,
            provider: String::new(),
            prompt_profile: String::new(),
            blocks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TranslationPartialPayload {
    pub generation: u64,
    pub selection: Option<SelectionRect>,
    pub source_language: String,
    pub target_language: String,
    pub detected_source: Option<String>,
    pub captured_at: Option<String>,
    pub visible_layer: VisibleLayer,
    pub provider: String,
    pub prompt_profile: String,
    pub stage: PartialUpdateStage,
    pub complete: bool,
    pub blocks: Vec<OverlayBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub available: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PromptProfileDescriptor {
    pub id: String,
    pub label: String,
    pub version: String,
    pub task: String,
    pub provider_family: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilities {
    pub ocr_providers: Vec<ProviderDescriptor>,
    pub ai_providers: Vec<ProviderDescriptor>,
    pub prompt_profiles: Vec<PromptProfileDescriptor>,
    pub default_ocr_provider_id: Option<String>,
    pub default_ai_provider_id: Option<String>,
    pub default_prompt_profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiTranslationRequest {
    pub endpoint: String,
    pub provider_id: String,
    pub model: String,
    pub prompt_profile: String,
    pub source_language: String,
    pub target_language: String,
    pub texts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OcrRecognitionRequest {
    pub provider_id: String,
    pub image_png_base64: String,
    pub source_language: String,
    pub hint_language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OcrRecognitionLine {
    pub text: String,
    pub rect: PixelRect,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OcrRecognitionResponse {
    pub provider_id: String,
    pub language: String,
    pub lines: Vec<OcrRecognitionLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiTranslationResponse {
    pub provider_id: String,
    pub model: String,
    pub prompt_profile: String,
    pub detected_source: String,
    pub translations: Vec<String>,
    pub confidences: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiTranslationDelta {
    pub index: usize,
    pub provider_id: String,
    pub model: String,
    pub prompt_profile: String,
    pub detected_source: Option<String>,
    pub translated_text: String,
    pub confidence: Option<f32>,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkCase {
    pub id: String,
    pub description: String,
    pub source_language: String,
    pub target_language: String,
    pub texts: Vec<String>,
    pub expected_translations: Vec<String>,
    pub prompt_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkSuite {
    pub id: String,
    pub version: String,
    pub title: String,
    pub cases: Vec<BenchmarkCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkCaseResult {
    pub case_id: String,
    pub prompt_profile: String,
    pub provider_id: String,
    pub expected_translations: Vec<String>,
    pub actual_translations: Vec<String>,
    pub exact_match_score: f32,
    pub latency_ms: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkReport {
    pub suite_id: String,
    pub provider_id: String,
    pub prompt_profile: String,
    pub case_count: usize,
    pub average_exact_match_score: f32,
    pub average_latency_ms: f32,
    pub cases: Vec<BenchmarkCaseResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshot {
    pub running: bool,
    pub status: RuntimeStatus,
    pub status_detail: String,
    pub source_language: String,
    pub target_language: String,
    pub ocr_provider: String,
    pub ai_provider: String,
    pub prompt_profile: String,
    pub panel_pinned: bool,
    pub selection: Option<SelectionRect>,
    pub selector_bounds: Option<SelectionRect>,
    pub copy_mode: bool,
    pub endpoint: String,
    pub model: String,
    pub generation: u64,
    pub visible_layer: VisibleLayer,
    pub block_count: usize,
    pub last_updated: Option<String>,
    pub last_detected_source: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineSettings {
    pub source_language: String,
    pub target_language: String,
}

impl Default for RuntimeSnapshot {
    fn default() -> Self {
        Self {
            running: false,
            status: RuntimeStatus::Idle,
            status_detail: "Ready".to_string(),
            source_language: "auto".to_string(),
            target_language: "zh-TW".to_string(),
            ocr_provider: String::new(),
            ai_provider: String::new(),
            prompt_profile: String::new(),
            panel_pinned: true,
            selection: None,
            selector_bounds: None,
            copy_mode: false,
            endpoint: String::new(),
            model: "discovering".to_string(),
            generation: 0,
            visible_layer: VisibleLayer::None,
            block_count: 0,
            last_updated: None,
            last_detected_source: None,
            last_error: None,
        }
    }
}