use crate::models::ProviderDescriptor;

pub const OLLAMA_PROVIDER_ID: &str = "ollama";
pub const OLLAMA_PROVIDER_LABEL: &str = "Ollama";
pub const DEFAULT_PROMPT_PROFILE: &str = "translation.ui_overlay.default";

pub fn descriptor(available: bool, detail: Option<String>) -> ProviderDescriptor {
    ProviderDescriptor {
        id: OLLAMA_PROVIDER_ID.to_string(),
        label: OLLAMA_PROVIDER_LABEL.to_string(),
        kind: "ai".to_string(),
        available,
        detail,
    }
}
