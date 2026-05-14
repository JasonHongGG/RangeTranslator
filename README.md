# RangeTranslator

RangeTranslator is a Tauri desktop app for selecting a screen region, running OCR on that region, translating the recognized text, and rendering the result back into a transparent overlay window.

This codebase now uses a provider-driven runtime shape:

- Rust/Tauri owns capture orchestration, window lifecycle, runtime state, and overlay events.
- OCR is wrapped behind an interface so the current Windows OCR implementation can be replaced later.
- AI translation is routed through a persistent Python sidecar.
- Prompt text is no longer embedded in Rust code; prompts live as external assets.

## Runtime Architecture

Core Rust modules:

- `src-tauri/src/app/`: Tauri command surface, selector/overlay lifecycle, event emission, and pipeline orchestration
- `src-tauri/src/providers/`: OCR provider implementations and AI runtime client interfaces
- `src-tauri/src/sidecar/`: persistent sidecar worker lifecycle, JSON-lines RPC, runtime discovery, and retry logic
- `src-tauri/src/benchmark/`: prompt benchmark suite loading and execution
- `src-tauri/src/state/`: shared runtime snapshot and latest translation state
- `src-tauri/src/models.rs`: provider-neutral runtime, OCR, AI, benchmark, and partial-update contracts

Frontend modules:

- `src/views/`: dedicated panel, selector, and overlay views
- `src/app/`: routing, preview data, debug helpers, and overlay utility functions
- `src/ui/`: shared icon components

Sidecar runtime:

- `range-translator-runtime/range_translator_runtime/app/`: runtime application dispatch and service wiring
- `range-translator-runtime/range_translator_runtime/providers/`: AI provider implementations and OCR provider interfaces
- `range-translator-runtime/range_translator_runtime/prompts/`: prompt repository and prompt rendering helpers
- `range-translator-runtime/prompts/`: external prompt profiles
- `benchmarks/`: benchmark corpus for prompt and provider evaluation

## Provider Model

Current provider setup:

- OCR provider: `windows-native`
- AI provider: `ollama` via sidecar
- Default prompt profile: `translation.ui_overlay.default`

The app keeps OCR and AI concerns provider-neutral at the contract layer so the current implementations can be replaced without rewriting overlay logic. The Python sidecar now mirrors that split with separate prompt, provider, and application modules.

## Prompt Assets

Prompts are stored as JSON assets under `range-translator-runtime/prompts/`.

Each prompt profile includes:

- `id`
- `version`
- `label`
- `task`
- `providerFamily`
- `system`
- `userTemplate`
- `outputSchema`

The sidecar loads and validates prompt profiles at runtime, and the Rust app only passes `promptProfile` identifiers through provider-neutral request contracts.

## Benchmarks

Benchmark suites live under `benchmarks/`.

The current default suite is:

- `benchmarks/ui_overlay.translation_suite.json`

This suite is intended to support prompt research and provider comparison using repeatable OCR translation samples.

## Environment Overrides

The sidecar runtime supports these environment variables:

- `RANGE_TRANSLATOR_RUNTIME_DIR`: explicit path to the sidecar runtime root
- `RANGE_TRANSLATOR_PYTHON`: explicit Python executable for the sidecar
- `RANGE_TRANSLATOR_PROMPT_DIR`: explicit path to prompt assets
- `RANGE_TRANSLATOR_BENCHMARK_DIR`: explicit path to benchmark suites

## Development Setup

Install frontend dependencies:

```bash
npm install
```

Prepare the Python runtime if you want a bundled local interpreter for the sidecar:

```bash
cd range-translator-runtime
python -m venv .venv
```

Run the frontend and Tauri app:

```bash
npm run tauri dev
```

## Validation Commands

Frontend build:

```bash
npm run build
```

Rust/Tauri compile check:

```bash
npm run tauri:check
```

## Current Scope

The current implementation includes:

- OCR provider abstraction in Rust
- sidecar-based AI invocation pathway
- external prompt assets
- prompt benchmark asset loading
- partial translation event flow to the overlay frontend

Follow-up implementation work is still expected for richer OCR providers, stronger prompt evaluation tooling, and broader runtime status UI, but the provider-driven foundation is now in place.
