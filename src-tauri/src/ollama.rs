use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct TranslationResponse {
    pub model: String,
    pub detected_source: String,
    pub translations: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    model: String,
    stream: bool,
    format: &'static str,
    messages: Vec<ChatMessage>,
    options: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: String,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelTag>,
}

#[derive(Debug, Deserialize)]
struct ModelTag {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranslationEnvelope {
    detected_source: Option<String>,
    translations: Vec<String>,
}

pub async fn translate_texts(
    endpoint: &str,
    current_model: &str,
    source_language: &str,
    target_language: &str,
    texts: &[String],
) -> Result<TranslationResponse> {
    if texts.is_empty() {
        return Ok(TranslationResponse {
            model: current_model.to_string(),
            detected_source: source_language.to_string(),
            translations: Vec::new(),
        });
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(35))
        .build()
        .context("failed to build the Ollama HTTP client")?;
    let base = endpoint.trim_end_matches('/');
    let model = resolve_model(&client, base, current_model).await?;

    let prompt = format!(
        "You translate short desktop UI text. Return JSON only with this exact shape: \
        {{\"detected_source\":\"{source_language}\",\"translations\":[\"...\"]}}. \
        Rules: preserve the array order exactly; keep empty strings empty; avoid notes; keep each translation compact; \
        target language is {} ({}); detected_source must be a useful BCP-47 tag. Input JSON: {}",
        pretty_language(target_language),
        target_language,
        serde_json::to_string(texts).context("failed to serialize OCR text batch")?,
    );

    let request = ChatRequest {
        model: model.clone(),
        stream: false,
        format: "json",
        messages: vec![
            ChatMessage {
                role: "system",
                content: "You translate OCR text for a realtime desktop overlay. Output JSON only.".to_string(),
            },
            ChatMessage {
                role: "user",
                content: prompt,
            },
        ],
        options: json!({
            "temperature": 0.1,
            "top_p": 0.9
        }),
    };

    let response = client
        .post(format!("{base}/api/chat"))
        .json(&request)
        .send()
        .await
        .context("failed to reach the Ollama chat endpoint")?
        .error_for_status()
        .context("Ollama returned an unsuccessful status code")?;

    let content = response
        .json::<ChatResponse>()
        .await
        .context("failed to decode the Ollama response")?
        .message
        .content;

    let envelope: TranslationEnvelope = serde_json::from_str(&extract_json(&content)?)
        .context("failed to parse the translated JSON payload")?;

    let mut translations = envelope.translations;
    if translations.len() < texts.len() {
        translations.extend(texts[translations.len()..].iter().cloned());
    }
    if translations.len() > texts.len() {
        translations.truncate(texts.len());
    }

    Ok(TranslationResponse {
        model,
        detected_source: envelope
            .detected_source
            .unwrap_or_else(|| source_language.to_string()),
        translations,
    })
}

async fn resolve_model(client: &Client, base: &str, current_model: &str) -> Result<String> {
    let preferred = [
        "qwen3:8b",
        "qwen2.5:7b-instruct",
        "llama3.1:8b",
        "phi4:14b",
    ];

    let response = client.get(format!("{base}/api/tags")).send().await;
    if let Ok(response) = response {
        let tags = response
            .error_for_status()
            .context("unable to query the Ollama model list")?
            .json::<TagsResponse>()
            .await
            .context("unable to decode the Ollama model list")?;

        if !current_model.is_empty() && current_model != "discovering" {
            if tags.models.iter().any(|item| item.name == current_model) {
                return Ok(current_model.to_string());
            }
        }

        for preferred_model in preferred {
            if let Some(found) = tags.models.iter().find(|item| item.name == preferred_model) {
                return Ok(found.name.clone());
            }
        }

        if let Some(first) = tags.models.first() {
            return Ok(first.name.clone());
        }
    }

    Ok(if current_model.is_empty() {
        "qwen3:8b".to_string()
    } else {
        current_model.to_string()
    })
}

fn extract_json(raw: &str) -> Result<String> {
    if raw.trim_start().starts_with('{') {
        return Ok(raw.trim().to_string());
    }

    let start = raw.find('{').context("missing JSON object start")?;
    let end = raw.rfind('}').context("missing JSON object end")?;
    Ok(raw[start..=end].to_string())
}

fn pretty_language(tag: &str) -> &'static str {
    match tag {
        "zh-TW" => "Traditional Chinese",
        "zh-Hans" => "Simplified Chinese",
        "ja-JP" => "Japanese",
        "ko-KR" => "Korean",
        "fr-FR" => "French",
        "de-DE" => "German",
        "es-ES" => "Spanish",
        "ru-RU" => "Russian",
        "th-TH" => "Thai",
        "vi-VN" => "Vietnamese",
        "id-ID" => "Indonesian",
        _ => "English",
    }
}