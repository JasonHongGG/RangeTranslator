use std::{future::Future, pin::Pin, sync::Arc};

use serde_json::json;

use crate::models::{
    AiTranslationDelta, AiTranslationRequest, AiTranslationResponse, OcrRecognitionRequest,
    OcrRecognitionResponse, OcrWarmupRequest, OcrWarmupResponse, RuntimeCapabilities,
};

use super::transport::{invoke, invoke_streaming};

pub trait RuntimeGateway: Send + Sync {
    fn query_capabilities<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<RuntimeCapabilities, String>> + Send + 'a>>;

    fn recognize<'a>(
        &'a self,
        request: OcrRecognitionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<OcrRecognitionResponse, String>> + Send + 'a>>;

    fn prewarm_ocr<'a>(
        &'a self,
        request: OcrWarmupRequest,
    ) -> Pin<Box<dyn Future<Output = Result<OcrWarmupResponse, String>> + Send + 'a>>;

    fn translate<'a>(
        &'a self,
        request: AiTranslationRequest,
        on_partial: Arc<dyn Fn(AiTranslationDelta) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<AiTranslationResponse, String>> + Send + 'a>>;
}

#[derive(Debug, Default)]
pub struct SidecarRuntimeGateway;

impl RuntimeGateway for SidecarRuntimeGateway {
    fn query_capabilities<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<RuntimeCapabilities, String>> + Send + 'a>> {
        Box::pin(async move {
            tokio::task::spawn_blocking(|| invoke("status", &json!({})))
                .await
                .map_err(|error| error.to_string())?
        })
    }

    fn recognize<'a>(
        &'a self,
        request: OcrRecognitionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<OcrRecognitionResponse, String>> + Send + 'a>> {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || invoke("recognize", &request))
                .await
                .map_err(|error| error.to_string())?
        })
    }

    fn prewarm_ocr<'a>(
        &'a self,
        request: OcrWarmupRequest,
    ) -> Pin<Box<dyn Future<Output = Result<OcrWarmupResponse, String>> + Send + 'a>> {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || invoke("prewarm", &request))
                .await
                .map_err(|error| error.to_string())?
        })
    }

    fn translate<'a>(
        &'a self,
        request: AiTranslationRequest,
        on_partial: Arc<dyn Fn(AiTranslationDelta) + Send + Sync>,
    ) -> Pin<Box<dyn Future<Output = Result<AiTranslationResponse, String>> + Send + 'a>> {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                invoke_streaming("translate", &request, move |delta: AiTranslationDelta| {
                    on_partial(delta);
                })
            })
            .await
            .map_err(|error| error.to_string())?
        })
    }
}

static RUNTIME_GATEWAY: SidecarRuntimeGateway = SidecarRuntimeGateway;

pub fn runtime_gateway() -> &'static dyn RuntimeGateway {
    &RUNTIME_GATEWAY
}
