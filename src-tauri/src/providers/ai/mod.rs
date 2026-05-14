mod ollama;
mod sidecar_client;

use std::{future::Future, pin::Pin, sync::Arc};

use crate::models::{
    AiTranslationDelta, AiTranslationRequest, AiTranslationResponse, RuntimeCapabilities,
};

pub use ollama::{descriptor as ollama_descriptor, DEFAULT_PROMPT_PROFILE, OLLAMA_PROVIDER_ID};
use sidecar_client::SidecarAiRuntimeClient;

pub trait AiRuntimeClient: Send + Sync {
    fn query_capabilities<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<RuntimeCapabilities, String>> + Send + 'a>>;

    fn translate<'a>(
        &'a self,
        request: AiTranslationRequest,
        on_partial: Arc<dyn Fn(AiTranslationDelta) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<AiTranslationResponse, String>> + Send + 'a>>;
}

impl AiRuntimeClient for SidecarAiRuntimeClient {
    fn query_capabilities<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<RuntimeCapabilities, String>> + Send + 'a>> {
        self.query_capabilities()
    }

    fn translate<'a>(
        &'a self,
        request: AiTranslationRequest,
        on_partial: Arc<dyn Fn(AiTranslationDelta) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<AiTranslationResponse, String>> + Send + 'a>> {
        self.translate(request, on_partial)
    }
}

pub fn default_runtime_client() -> Box<dyn AiRuntimeClient> {
    Box::new(SidecarAiRuntimeClient)
}
