/// <reference types="node" />

import test from 'node:test'
import assert from 'node:assert/strict'

import { resolveOverlayStyle } from './overlay-style.ts'

test('high-confidence sampled styles are preserved', () => {
  const style = resolveOverlayStyle({
    foreground: '#F7F8FA',
    background: '#2965A8',
    styleConfidence: 0.9,
  })

  assert.deepEqual(style, {
    foreground: '#F7F8FA',
    background: '#2965A8',
  })
})

test('low-confidence light surfaces fall back to dark ink and softened background', () => {
  const style = resolveOverlayStyle({
    foreground: '#D6C21A',
    background: '#ECEAE4',
    styleConfidence: 0.18,
  })

  assert.equal(style.foreground, '#121A21')
  assert.notEqual(style.background, '#ECEAE4')
})

test('low-confidence dark surfaces fall back to light ink', () => {
  const style = resolveOverlayStyle({
    foreground: '#3A4450',
    background: '#1B2430',
    styleConfidence: 0.22,
  })

  assert.equal(style.foreground, '#F4EFE5')
  assert.notEqual(style.background, '#1B2430')
})