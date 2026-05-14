use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::models::{RuntimeSnapshot, TranslationPartialPayload, TranslationPayload};
use crate::state;

pub const SNAPSHOT_EVENT: &str = "runtime-snapshot";
pub const TRANSLATION_EVENT: &str = "translation-update";
pub const TRANSLATION_PARTIAL_EVENT: &str = "translation-partial";
pub const DEBUG_EVENT: &str = "selector-debug";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugPayload {
    pub scope: String,
    pub message: String,
    pub detail: Value,
    pub timestamp: String,
}

pub fn emit_snapshot(app: &AppHandle, snapshot: &RuntimeSnapshot) {
    let _ = app.emit(SNAPSHOT_EVENT, snapshot.clone());
}

pub fn emit_translation(app: &AppHandle, payload: &TranslationPayload) {
    let _ = app.emit(TRANSLATION_EVENT, payload.clone());
}

pub fn emit_translation_partial(app: &AppHandle, payload: &TranslationPartialPayload) {
    let _ = app.emit(TRANSLATION_PARTIAL_EVENT, payload.clone());
}

pub fn emit_debug(app: &AppHandle, scope: &str, message: &str, detail: Value) {
    let payload = DebugPayload {
        scope: scope.to_string(),
        message: message.to_string(),
        detail,
        timestamp: state::timestamp(),
    };
    let _ = app.emit(DEBUG_EVENT, payload.clone());
    println!(
        "[RangeTranslator:{}] {} {}",
        payload.scope, payload.message, payload.detail
    );
}
