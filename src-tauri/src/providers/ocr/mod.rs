mod windows_native;

use anyhow::Result;

use crate::{
    capture::CapturedFrame,
    models::{PixelRect, ProviderDescriptor},
};

#[derive(Debug, Clone)]
pub struct OcrTextLine {
    pub text: String,
    pub rect: PixelRect,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct OcrResult {
    pub language: String,
    pub lines: Vec<OcrTextLine>,
}

pub trait OcrProvider: Send + Sync {
    fn id(&self) -> &'static str;

    fn label(&self) -> &'static str;

    fn recognize(
        &self,
        frame: &CapturedFrame,
        requested_source: &str,
        hint: Option<&str>,
    ) -> Result<OcrResult>;

    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: self.id().to_string(),
            label: self.label().to_string(),
            kind: "ocr".to_string(),
            available: true,
            detail: None,
        }
    }
}

pub fn default_ocr_provider_id() -> &'static str {
    #[cfg(windows)]
    {
        "windows-native"
    }

    #[cfg(not(windows))]
    {
        "unsupported"
    }
}

pub fn resolve_ocr_provider(provider_id: &str) -> Box<dyn OcrProvider> {
    #[cfg(windows)]
    {
        match provider_id {
            "windows-native" => Box::new(windows_native::WindowsOcrProvider),
            _ => Box::new(windows_native::WindowsOcrProvider),
        }
    }

    #[cfg(not(windows))]
    {
        let _ = provider_id;
        Box::new(windows_native::UnsupportedOcrProvider)
    }
}

pub fn available_provider_descriptors() -> Vec<ProviderDescriptor> {
    vec![resolve_ocr_provider(default_ocr_provider_id()).descriptor()]
}
