use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRequest<'a, TRequest: Serialize> {
    pub request_id: u64,
    pub subcommand: &'a str,
    pub payload: &'a TRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFrame {
    pub request_id: u64,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub payload: Option<Value>,
    #[serde(default)]
    pub ok: Option<bool>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub traceback: Option<String>,
}

pub enum RuntimeInvokeError {
    Recoverable(String),
    Unrecoverable(String),
}

pub fn decode_payload<T: DeserializeOwned>(value: Value, context: &str) -> Result<T, RuntimeInvokeError> {
    serde_json::from_value(value).map_err(|error| {
        RuntimeInvokeError::Recoverable(format!("failed to decode sidecar {context}: {error}"))
    })
}

pub fn format_worker_error(frame: RuntimeFrame) -> String {
    let mut detail = frame
        .error
        .unwrap_or_else(|| "sidecar runtime reported an error".to_string());

    if let Some(traceback) = frame.traceback {
        let traceback = traceback.trim();
        if !traceback.is_empty() {
            detail = format!("{detail}\n{traceback}");
        }
    }

    detail
}
