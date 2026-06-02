# Overlay Phase 6 Sign-Off

## Purpose

This document is the Phase 6 close-out artifact for the overlay rearchitecture. It defines the automated regression surface, the manual validation matrix, and the hard stop conditions for declaring the rewrite complete.

## Automated Coverage

### Rust

- `cargo test --manifest-path src-tauri/Cargo.toml capture::tests`
  - Verifies badge/pill background sampling.
  - Verifies gradient surfaces do not collapse to black/white defaults.
  - Verifies low-evidence surfaces are downgraded to low style confidence.
- `cargo test --manifest-path src-tauri/Cargo.toml app::pipeline::tests`
  - Verifies OCR canonicalization only merges true duplicates.
  - Verifies selection-space normalization keeps source units aligned.
  - Verifies translation cache keys ignore frame-scoped span ids.
- `cargo test --manifest-path src-tauri/Cargo.toml domain::overlay_frame::tests`
  - Verifies full/partial payloads preserve frame context.
  - Verifies source-unit style confidence survives transport.
  - Verifies empty frames resolve to `VisibleLayer::None`.

### Python sidecar

- `python -m unittest discover -s range-translator-runtime/tests -p 'test_*.py'`
  - `test_paddleocr_provider.py`
    - Verifies OCR geometry contract from `rec_boxes`, `rec_polys`, and `dt_polys`.
    - Verifies raw OCR detections remain ordered instead of being collapsed in-provider.
    - Verifies empty text and invalid geometry are dropped instead of poisoning the payload.
  - `test_ollama_provider.py`
    - Verifies id/index alignment.
    - Verifies merged/reordered/untranslated output is rejected.
    - Verifies long multi-span batches stay ordered and stable.

### Frontend

- `npm run test:overlay`
  - Phase 4 tests verify frame-aware merge and OCR visibility before translation is ready.
  - Phase 5 tests verify conservative style fallback for low-confidence surfaces.
  - Phase 6 tests verify there is no implicit mask expansion and that text fitting keeps short CJK single-line while shrinking long Latin strings to stay inside the source span.
- `npm run build`
- `npm run lint`

## Manual Validation Matrix

The same golden case must be checked across every row below. Do not substitute synthetic samples for final sign-off.

| Dimension | Required variants |
| --- | --- |
| DPI | 100%, 125%, 150% |
| Monitor topology | single monitor, dual monitor |
| Surface type | plain body text, badge/pill, button label, long paragraph |
| Background style | light flat, dark flat, gradient/mixed surface |
| Translation phase | OCR only, pending translation, streaming partials, final translation |

For each run, verify these exact outcomes:

1. Red OCR debug boxes stay anchored to the same visible text block.
2. OCR text remains visible until translated text is available for that same span.
3. Final translated text replaces only its own source span and does not leak into neighbors.
4. Badge/pill blocks keep a surface color close to the original chip instead of inheriting the page background.
5. Low-confidence style samples degrade conservatively instead of producing obviously wrong foreground/background colors.
6. Multi-line translations stay inside the original source span bounds.

## Stop Conditions

Phase 6 is only complete when all of the following are true:

1. All automated checks in this document are green.
2. The golden case passes every row in the manual validation matrix.
3. No overlay block shows cross-span intrusion, hidden OCR, or frame-mixed translation state.
4. No low-confidence style sample produces an obviously unrelated surface color with high certainty.
5. No future fix needs a renderer-side compatibility branch for old geometry or old style contracts.

If any single item fails, Phase 6 is not complete.