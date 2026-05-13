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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationPayload {
    pub selection: Option<SelectionRect>,
    pub source_language: String,
    pub target_language: String,
    pub detected_source: Option<String>,
    pub captured_at: Option<String>,
    pub unchanged: bool,
    pub blocks: Vec<OverlayBlock>,
}

impl Default for TranslationPayload {
    fn default() -> Self {
        Self {
            selection: None,
            source_language: "auto".to_string(),
            target_language: "zh-TW".to_string(),
            detected_source: None,
            captured_at: None,
            unchanged: false,
            blocks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshot {
    pub running: bool,
    pub status: RuntimeStatus,
    pub status_detail: String,
    pub source_language: String,
    pub target_language: String,
    pub panel_pinned: bool,
    pub selection: Option<SelectionRect>,
    pub selector_bounds: Option<SelectionRect>,
    pub copy_mode: bool,
    pub endpoint: String,
    pub model: String,
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
            panel_pinned: true,
            selection: None,
            selector_bounds: None,
            copy_mode: false,
            endpoint: String::new(),
            model: "discovering".to_string(),
            block_count: 0,
            last_updated: None,
            last_detected_source: None,
            last_error: None,
        }
    }
}