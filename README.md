# RangeTranslator

RangeTranslator is a Tauri desktop app for selecting a screen region, running OCR on that region, translating the recognized text, and rendering the result back into a transparent overlay window.

This codebase now uses a provider-driven runtime shape:

- Rust/Tauri owns capture orchestration, window lifecycle, runtime state, and overlay events.
- OCR and AI implementations live behind a persistent Python sidecar.
- The sidecar currently owns the active PaddleOCR GPU runtime and the Ollama translation provider.
- Prompt text is no longer embedded in Rust code; prompts live as external assets.

## Runtime Architecture

Core Rust modules:

- `src-tauri/src/app/`: Tauri command surface, selector/overlay lifecycle, event emission, and pipeline orchestration
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

- OCR provider: `paddleocr` via sidecar
- AI provider: `ollama` via sidecar
- Default prompt profile: `translation.ui_overlay.default`

The app keeps OCR and AI concerns provider-neutral at the contract layer so implementations can change without rewriting overlay logic. The Python sidecar owns provider defaults plus the actual OCR/translation implementations.

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
- `RANGE_TRANSLATOR_OLLAMA_TAGS_TIMEOUT_SECONDS`: override the Ollama `/api/tags` discovery timeout in the sidecar
- `RANGE_TRANSLATOR_OLLAMA_CHAT_TIMEOUT_SECONDS`: override the Ollama `/api/chat` timeout in the sidecar
- `RANGE_TRANSLATOR_OLLAMA_KEEP_ALIVE`: override the Ollama model keep-alive sent by the sidecar

## Development Setup

Install frontend dependencies:

```bash
npm install
```

Prepare the Python runtime if you want a bundled local interpreter for the sidecar:

```bash
cd range-translator-runtime
py -3.12 -m venv .venv
./.venv/Scripts/python.exe -m pip install --upgrade pip setuptools wheel
./.venv/Scripts/python.exe -m pip install paddlepaddle-gpu==3.2.2 -i https://www.paddlepaddle.org.cn/packages/stable/cu129/
./.venv/Scripts/python.exe -m pip install -e .
```

Notes:

- The validated Windows GPU path on this machine is the official Paddle `cu129` wheel channel plus `paddleocr 3.5.x`; older `paddlepaddle-gpu 2.6.2` / `paddleocr 2.10.0` wheels do not support this RTX 5070 Laptop GPU.
- The sidecar is GPU-only for OCR. If the Paddle GPU runtime is unavailable, startup should fail fast instead of falling back to CPU.
- Paddle model downloads are redirected into `range-translator-runtime/.runtime/paddlex`.

## Pitfalls And Fixes

The following issues already happened in this repo and should be treated as hard-earned constraints, not optional cleanup notes.

- Windows Paddle GPU setup on this machine must use the official `paddlepaddle-gpu 3.2.2` `cu129` channel together with `paddleocr 3.5.x`. Older Windows wheels such as `2.6.2` report GPU compute capability `12.0` as unsupported on the RTX 5070 Laptop GPU.
- Do not reintroduce CPU fallback for OCR. If the sidecar cannot load a supported Paddle GPU runtime, it should surface an explicit error and stop there.
- The Python sidecar must bootstrap all required cu12 wheel DLL directories, not only `cudnn/cublas/cuda_runtime/cuda_nvrtc`. The working set here also includes `cufft`, `curand`, `cusolver`, `cusparse`, and `nvjitlink`.
- Keep PaddleX model cache inside `range-translator-runtime/.runtime/paddlex` via `PADDLE_PDX_CACHE_HOME`. Leaving it under the user profile cache makes runtime ownership blurry and makes debugging which model set is actually in use much harder.
- Selector window creation must stay on the async command path and use a `run_on_main_thread` callback. A blocking selector command that waits synchronously around window build can hang before `build()` returns.
- In dev mode, do not let the app resolve `src-tauri/target/debug/range-translator-runtime` ahead of the workspace-root `range-translator-runtime`. That copied debug runtime can keep an old `.venv` and silently resurrect stale Paddle wheels even after the real runtime has already been upgraded.
- When the workspace-root runtime is selected in dev mode, remove any leftover `src-tauri/target/debug/range-translator-runtime` copy. Leaving the old copy in place invites the next regression because people forget it exists and start debugging the wrong environment again.
- `tauri dev` must not bundle or watch the full sidecar runtime tree. If dev mode merges the production `bundle.resources` config, Cargo starts tracking `.venv` and `.runtime` churn while Vite also sees copied runtime files under `src-tauri/target/debug`, which leads to rebuild/reload loops and can present as a blank white app window.
- Selection commit must not be gated on sidecar readiness checks. The correct order is: commit the selected region, ensure/show the overlay window, hide the selector immediately, then start sidecar capability/OCR readiness work. If sidecar readiness is checked first and it blocks or fails, the visible symptom is exactly what happened here: the selector stays open and no overlay appears.
- Closing the selector should hide it immediately and only then perform the delayed `close()`. Relying only on delayed close can leave a full-screen always-on-top selector window covering the overlay even when the backend flow already moved on.
- The selector/overlay flow is a window-management responsibility owned by Rust/Tauri, not by OCR provider startup. Keep these concerns separated when refactoring.
- Auto OCR cannot eagerly initialize every fallback recognition family on the first pass. For this app that creates the exact post-selector stall seen in logs. Prefer the most likely models first and stop as soon as one high-confidence candidate succeeds.
- Showing OCR as the waiting state is fine, but only if every app-owned window that can overlap the capture region is excluded from capture. Without capture protection, the overlay/panel/selector can feed their own text back into OCR and create the repeated OCR/AI loop seen earlier.
- For `qwen3` on Ollama, realtime overlay translation must send `think:false`. On this repo's endpoint, leaving thinking enabled pushed the same structured translation request from roughly 1.1s to roughly 9s with no quality benefit for the overlay use case.
- Sidecar stderr on Windows cannot be assumed to be valid UTF-8. Decode it lossily when piping it back into Rust logs, otherwise useful diagnostics get replaced by a misleading `failed to read sidecar stderr: stream did not contain valid UTF-8` message.
- Sidecar stdout is reserved for JSON-RPC frames only. `paddle.utils.run_check()` prints `Running verify PaddlePaddle program ...` to stdout, so provider-side GPU validation must capture that output instead of letting it leak into the transport and break response parsing.
- Remote Ollama behind ngrok is not equivalent to a local `127.0.0.1` server. In this repo we verified that `/api/tags` can succeed while both `/api/chat` and `/api/generate` fail to emit even response headers for minutes. Client-side batching and `keep_alive` reduce avoidable overhead, but they do not fix a remote inference server or tunnel that is not actually returning generation responses. Keep the chat timeout configurable, fail with an explicit server-side inference warning, and debug the remote Ollama host itself when metadata works but generation never starts.
- A successful `discovering` flow depends on the sidecar choosing the right model, not just any available model. We verified on this endpoint that Python `urllib` and the current structured prompt both work when the provider uses `qwen3:8b`, while the old discovery priority incorrectly preferred `mistral-small3.2:latest`. Keep realtime-safe models such as `qwen3:8b` at the front of the sidecar preference list.
- When a remote Ollama endpoint stalls, do not hammer it every frame. This pipeline now enters a short AI retry cooldown while keeping OCR blocks visible and surfacing a concise warning in the runtime snapshot instead of spamming the same full traceback on every capture cycle.
- Frontend route resolution and view bootstrap must not assume `getCurrentWindow()` is always synchronously available during initial render. If that access throws before React mounts the view tree, the visible symptom is a blank white window with no panel UI.

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
