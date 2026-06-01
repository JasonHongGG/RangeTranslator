# Overlay Runtime Rearchitecture

## 目標

這份文件定義目前 overlay runtime 的新內部架構。這次重構的原則只有三條：

1. 幾何真相只來自 `sourceRect` 與當前 `selection -> viewport` 映射。
2. Rust pipeline 只負責協調，不再直接擁有場景建模、幾何推導、translation 對齊細節。
3. 前端 overlay 只消費 render model，不再把幾何、文字 fitting、layer rendering 混在同一個 React component。

## 新契約

### Rust domain core

- `domain::geometry`
  - 負責 `capture frame -> selection space` 的幾何正規化。
  - 左上使用 `floor`，右下使用 `ceil`，避免 OCR 邊界被默默縮小。
- `domain::scene`
  - 負責 OCR line canonicalization。
  - 產出 `SourceSpan` domain object，再轉成 transport `OverlaySourceUnit`。
- `domain::translation`
  - 負責 pending/final translation unit 建構。
  - 負責 frame-independent translation cache key。
  - 負責 partial delta 對齊到既有 source span。
- `domain::overlay_frame`
  - 負責把 domain scene 組裝成 `TranslationPayload` 與 `TranslationPartialPayload`。

### Frontend overlay core

- `app/overlay-geometry.ts`
  - 建立 viewport geometry context。
  - 將 `sourceRect` 轉成 CSS rect。
- `app/overlay-text-layout.ts`
  - 執行 width-aware / height-aware text fitting。
  - 對短 CJK 文本避免不必要換行。
- `app/overlay-view-model.ts`
  - 將 snapshot + translation payload 組裝為 render model。
- `views/overlay/*Layer.tsx`
  - 每個 layer 只負責一種繪製責任：mask、text、debug box。

## 拆除清單

以下責任不再允許留在單一檔案內：

- `src-tauri/src/app/pipeline.rs`
  - OCR duplicate merge
  - selection normalization
  - source unit text metric estimation
  - translation unit alignment
  - payload assembly
- `src/views/OverlayView.tsx`
  - geometry scaling
  - translation fallback 決策
  - text fitting
  - debug box rendering

## Phase 0~2 完成條件

- Rust `pipeline.rs` 成為 coordinator，核心規則移入 `domain/*`。
- 前端 `OverlayView.tsx` 只保留 runtime sync 與容器責任。
- 文字 sizing 不能再只靠 box height 推估，必須把實際寬度納入 fitting。
- 既有 `sourceRect` 幾何基線不能回退成 `renderRect` 或其他後端預算出的 CSS 座標。

## 後續 phase

- Phase 3 之後再重做 translation prompt/profile/provider orchestration。
- 本文件不處理 UI 視覺 redesign；現有 UI 介面必須維持。