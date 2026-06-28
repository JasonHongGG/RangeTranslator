/// <reference types="node" />

import test from 'node:test'
import assert from 'node:assert/strict'

import {
  buildOverlayRenderModel,
  resolveOverlayDisplayText,
} from './overlay-view-model.ts'
import {
  mergeTranslationPartial,
  mergeTranslationUpdate,
} from './overlay.ts'
import type {
  OverlaySourceUnit,
  RuntimeSnapshot,
  TranslationPartialPayload,
  TranslationPayload,
} from '../types'

function createSourceUnit(overrides: Partial<OverlaySourceUnit> = {}): OverlaySourceUnit {
  return {
    id: 'source-1',
    frameId: '1:1',
    order: 0,
    sourceText: 'General',
    sourceRect: { x: 20, y: 10, width: 180, height: 40 },
    fontSize: 24,
    lineHeight: 28,
    confidence: 0.95,
    foreground: '#ffffff',
    background: '#111111',
    styleConfidence: 0.9,
    align: 'left',
    ...overrides,
  }
}

function createSnapshot(overrides: Partial<RuntimeSnapshot> = {}): RuntimeSnapshot {
  return {
    running: true,
    status: 'ready',
    statusDetail: 'Ready',
    sourceLanguage: 'en',
    targetLanguage: 'zh-TW',
    ocrProvider: 'paddleocr',
    aiProvider: 'ollama',
    aiTranslationEnabled: true,
    panelPinned: true,
    debugScreenshotMode: false,
    selection: { x: 0, y: 0, width: 1000, height: 400 },
    selectorBounds: null,
    overlayMode: 'selectText',
    generation: 2,
    visibleLayer: 'translation',
    blockCount: 1,
    lastUpdated: null,
    lastDetectedSource: 'en',
    lastError: null,
    ...overrides,
  }
}

function createPayload(overrides: Partial<TranslationPayload> = {}): TranslationPayload {
  const sourceUnit = createSourceUnit()
  return {
    generation: 1,
    frameId: '1:1',
    selection: { x: 0, y: 0, width: 500, height: 200 },
    capture: {
      coordinateSpace: 'selectionPhysicalPixels',
      displayOriginX: 0,
      displayOriginY: 0,
      displayWidth: 1920,
      displayHeight: 1080,
      captureOriginX: 0,
      captureOriginY: 0,
      captureWidth: 500,
      captureHeight: 200,
      scaleFactor: 1,
    },
    sourceLanguage: 'en',
    targetLanguage: 'zh-TW',
    detectedSource: 'en',
    capturedAt: '2026-06-01T00:00:00.000Z',
    unchanged: false,
    visibleLayer: 'translation',
    provider: 'ollama',
    sourceUnits: [sourceUnit],
    translationUnits: [],
    ...overrides,
  }
}

function createPartialPayload(
  overrides: Partial<TranslationPartialPayload> = {},
): TranslationPartialPayload {
  return {
    generation: 1,
    frameId: '1:1',
    selection: { x: 0, y: 0, width: 500, height: 200 },
    capture: null,
    sourceLanguage: 'en',
    targetLanguage: 'zh-TW',
    detectedSource: 'en',
    capturedAt: '2026-06-01T00:00:01.000Z',
    visibleLayer: 'translation',
    provider: 'ollama',
    stage: 'translation',
    complete: false,
    sourceUnits: [],
    translationUnits: [],
    ...overrides,
  }
}

test('translation layer keeps OCR text visible until translated text is ready', () => {
  const sourceUnit = createSourceUnit()

  assert.equal(resolveOverlayDisplayText('translation', sourceUnit, undefined), 'General')
  assert.equal(resolveOverlayDisplayText('ocr', sourceUnit, undefined), 'General')
  assert.equal(
    resolveOverlayDisplayText('translation', sourceUnit, {
      sourceId: sourceUnit.id,
      order: 0,
      text: '一般',
      state: 'translated',
      confidence: 0.9,
      streaming: false,
    }),
    '一般',
  )
  assert.equal(
    resolveOverlayDisplayText('translation', sourceUnit, {
      sourceId: sourceUnit.id,
      order: 0,
      text: '',
      state: 'pending',
      confidence: 0,
      streaming: false,
    }),
    'General',
  )
})

test('render model geometry uses payload selection as the authoritative scale source', () => {
  const snapshot = createSnapshot({
    selection: { x: 0, y: 0, width: 1000, height: 400 },
  })
  const translation = createPayload({
    selection: { x: 0, y: 0, width: 500, height: 200 },
  })

  const model = buildOverlayRenderModel(snapshot, translation, {
    width: 250,
    height: 100,
  })

  assert.equal(model.geometryContext.scaleX, 0.5)
  assert.equal(model.geometryContext.scaleY, 0.5)
})

test('full payload updates remain authoritative even when capture metadata is absent', () => {
  const current = createPayload()
  const next = createPayload({
    capture: null,
    translationUnits: [
      {
        sourceId: 'source-1',
        order: 0,
        text: '一般',
        state: 'translated',
        confidence: 0.93,
        streaming: false,
      },
    ],
  })

  const merged = mergeTranslationUpdate(current, next)

  assert.equal(merged.capture, null)
  assert.equal(merged.translationUnits[0]?.text, '一般')
})

test('same-frame partials merge incrementally, but cross-frame partials replace the old frame', () => {
  const current = createPayload({
    translationUnits: [
      {
        sourceId: 'source-1',
        order: 0,
        text: '',
        state: 'pending',
        confidence: 0,
        streaming: false,
      },
    ],
  })

  const sameFrame = mergeTranslationPartial(
    current,
    createPartialPayload({
      translationUnits: [
        {
          sourceId: 'source-1',
          order: 0,
          text: '一般',
          state: 'translated',
          confidence: 0.94,
          streaming: true,
        },
      ],
    }),
  )

  assert.equal(sameFrame.frameId, '1:1')
  assert.equal(sameFrame.sourceUnits.length, 1)
  assert.equal(sameFrame.translationUnits[0]?.text, '一般')

  const nextFrame = mergeTranslationPartial(
    sameFrame,
    createPartialPayload({
      frameId: '1:2',
      sourceUnits: [
        createSourceUnit({
          id: 'source-2',
          frameId: '1:2',
          sourceText: 'Settings',
        }),
      ],
      translationUnits: [
        {
          sourceId: 'source-2',
          order: 0,
          text: '設定',
          state: 'translated',
          confidence: 0.95,
          streaming: false,
        },
      ],
    }),
  )

  assert.equal(nextFrame.frameId, '1:2')
  assert.deepEqual(nextFrame.sourceUnits.map((unit) => unit.id), ['source-2'])
  assert.deepEqual(nextFrame.translationUnits.map((unit) => unit.sourceId), ['source-2'])
})