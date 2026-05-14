use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    models::{
        AiTranslationDelta, AiTranslationRequest, AiTranslationResponse, RuntimeCapabilities,
    },
    sidecar,
};

pub struct SidecarAiRuntimeClient;

impl SidecarAiRuntimeClient {
    pub fn query_capabilities<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<RuntimeCapabilities, String>> + Send + 'a>> {
        Box::pin(async move { sidecar::query_capabilities().await })
    }

    pub fn translate<'a>(
        &'a self,
        request: AiTranslationRequest,
        on_partial: Arc<dyn Fn(AiTranslationDelta) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<AiTranslationResponse, String>> + Send + 'a>> {
        Box::pin(async move { sidecar::translate(request, on_partial).await })
    }
}
