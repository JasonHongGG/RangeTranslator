use std::{fs, path::PathBuf};

use anyhow::{Context, Result};

use crate::models::BenchmarkSuite;

const BENCHMARK_DIR_ENV: &str = "RANGE_TRANSLATOR_BENCHMARK_DIR";
const DEFAULT_SUITE_FILE: &str = "ui_overlay.translation_suite.json";

pub fn load_default_benchmark_suite() -> Result<BenchmarkSuite> {
    for candidate in candidate_suite_paths() {
        if candidate.exists() {
            let raw = fs::read_to_string(&candidate).with_context(|| {
                format!("failed to read benchmark suite at {}", candidate.display())
            })?;

            return serde_json::from_str(&raw).with_context(|| {
                format!("failed to parse benchmark suite at {}", candidate.display())
            });
        }
    }

    Err(anyhow::anyhow!(
        "benchmark suite not found. Expected benchmarks/{DEFAULT_SUITE_FILE}"
    ))
}

fn candidate_suite_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(custom_dir) = std::env::var(BENCHMARK_DIR_ENV) {
        let custom_dir = custom_dir.trim();
        if !custom_dir.is_empty() {
            paths.push(PathBuf::from(custom_dir).join(DEFAULT_SUITE_FILE));
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            paths.push(dir.join("benchmarks").join(DEFAULT_SUITE_FILE));
            if let Some(parent) = dir.parent() {
                paths.push(parent.join("benchmarks").join(DEFAULT_SUITE_FILE));
            }
        }
    }

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest_dir = PathBuf::from(manifest_dir);
        if let Some(workspace_root) = manifest_dir.parent() {
            paths.push(workspace_root.join("benchmarks").join(DEFAULT_SUITE_FILE));
        }
    }

    paths
}
