/// <reference types="node" />

import test from 'node:test'
import assert from 'node:assert/strict'

import { createOverlayGeometryContext, resolveOverlayCssRect } from './overlay-geometry.ts'
import { resolveOverlayTextLayout } from './overlay-text-layout.ts'
import type { OverlaySourceUnit } from '../types.ts'

function createSourceUnit(overrides: Partial<OverlaySourceUnit> = {}): OverlaySourceUnit {
  return {
    id: 'phase6-source',
    frameId: '1:1',
    order: 0,
    sourceText: 'General Settings',
    sourceRect: { x: 24, y: 12, width: 180, height: 42 },
    fontSize: 24,
    lineHeight: 28,
    confidence: 0.95,
    foreground: '#F7F8FA',
    background: '#2965A8',
    styleConfidence: 0.9,
    align: 'left',
    ...overrides,
  }
}

test('geometry rect does not expand without explicit faithful-renderer options', () => {
  const context = createOverlayGeometryContext(
    { x: 0, y: 0, width: 400, height: 200 },
    { width: 400, height: 200 },
  )

  const rect = resolveOverlayCssRect(
    { x: 24, y: 12, width: 180, height: 42 },
    context,
  )

  assert.deepEqual(rect, {
    left: 24,
    top: 12,
    width: 180,
    height: 42,
  })
})

test('short cjk text keeps a single-line layout when space allows', () => {
  const context = createOverlayGeometryContext(
    { x: 0, y: 0, width: 320, height: 180 },
    { width: 320, height: 180 },
  )

  const layout = resolveOverlayTextLayout(
    createSourceUnit({
      sourceRect: { x: 40, y: 30, width: 96, height: 28 },
      fontSize: 22,
      lineHeight: 26,
    }),
    '再見',
    context,
  )

  assert.equal(layout.whiteSpace, 'nowrap')
  assert.equal(layout.wordBreak, 'keep-all')
  assert.equal(layout.overflowWrap, 'normal')
  assert.ok(layout.fontSize <= layout.rect.height)
})

test('long latin translation shrinks to stay inside the source span bounds', () => {
  const context = createOverlayGeometryContext(
    { x: 0, y: 0, width: 320, height: 180 },
    { width: 320, height: 180 },
  )
  const unit = createSourceUnit({
    sourceRect: { x: 16, y: 40, width: 120, height: 34 },
    fontSize: 24,
    lineHeight: 28,
  })

  const layout = resolveOverlayTextLayout(
    unit,
    'Synchronized layered configuration restored',
    context,
  )

  assert.equal(layout.whiteSpace, 'pre-wrap')
  assert.equal(layout.wordBreak, 'break-word')
  assert.ok(layout.fontSize < unit.fontSize)
  assert.ok(layout.lineHeight <= layout.rect.height)
  assert.equal(layout.rect.width, 120)
  assert.equal(layout.rect.height, 34)
})