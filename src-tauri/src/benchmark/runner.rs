use std::{sync::Arc, time::Instant};

use crate::{
    models::{AiTranslationRequest, BenchmarkCaseResult, BenchmarkReport},
    sidecar::runtime_gateway,
};

use super::suite::load_default_benchmark_suite;

pub async fn run_default_prompt_benchmark(
    endpoint: &str,
    model: &str,
    provider_id: &str,
    prompt_profile: &str,
) -> Result<BenchmarkReport, String> {
    let suite = load_default_benchmark_suite().map_err(|error| error.to_string())?;
    let mut total_score = 0.0_f32;
    let mut total_latency_ms = 0.0_f32;
    let mut cases = Vec::new();

    for case in &suite.cases {
        let case_prompt_profile = case
            .prompt_profile
            .clone()
            .unwrap_or_else(|| prompt_profile.to_string());

        let started_at = Instant::now();
        let response = runtime_gateway().translate(
            AiTranslationRequest {
                endpoint: endpoint.to_string(),
                provider_id: provider_id.to_string(),
                model: model.to_string(),
                prompt_profile: case_prompt_profile.clone(),
                source_language: case.source_language.clone(),
                target_language: case.target_language.clone(),
                texts: case.texts.clone(),
            },
            Arc::new(|_| {}),
        )
        .await
        .map_err(|error| format!("benchmark case {} failed: {error}", case.id))?;

        let latency_ms = started_at.elapsed().as_secs_f32() * 1000.0;
        let exact_match_score = exact_match_score(
            &case.expected_translations,
            &response.translations,
        );

        total_score += exact_match_score;
        total_latency_ms += latency_ms;
        cases.push(BenchmarkCaseResult {
            case_id: case.id.clone(),
            prompt_profile: case_prompt_profile,
            provider_id: response.provider_id.clone(),
            expected_translations: case.expected_translations.clone(),
            actual_translations: response.translations.clone(),
            exact_match_score,
            latency_ms,
        });
    }

    let case_count = cases.len();
    Ok(BenchmarkReport {
        suite_id: suite.id,
        provider_id: provider_id.to_string(),
        prompt_profile: prompt_profile.to_string(),
        case_count,
        average_exact_match_score: if case_count == 0 {
            0.0
        } else {
            total_score / case_count as f32
        },
        average_latency_ms: if case_count == 0 {
            0.0
        } else {
            total_latency_ms / case_count as f32
        },
        cases,
    })
}

fn exact_match_score(expected: &[String], actual: &[String]) -> f32 {
    if expected.is_empty() {
        return 1.0;
    }

    let total = expected.len().max(actual.len()) as f32;
    if total == 0.0 {
        return 1.0;
    }

    let matches = expected
        .iter()
        .zip(actual.iter())
        .filter(|(left, right)| normalize(left) == normalize(right))
        .count() as f32;

    matches / total
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
