import type { CSSProperties } from 'react'
import type { OverlaySourceUnit } from '../types'
import {
  resolveOverlayUnitRect,
  type OverlayCssRect,
  type OverlayGeometryContext,
} from './overlay-geometry'

type ScriptProfile = 'cjk' | 'latin'

export type OverlayTextLayout = {
  rect: OverlayCssRect
  fontSize: number
  lineHeight: number
  whiteSpace: CSSProperties['whiteSpace']
  wordBreak: CSSProperties['wordBreak']
  overflowWrap: CSSProperties['overflowWrap']
}

export function resolveOverlayTextLayout(
  unit: OverlaySourceUnit,
  text: string,
  geometryContext: OverlayGeometryContext,
): OverlayTextLayout {
  const rect = resolveOverlayUnitRect(unit, geometryContext)
  const script = detectScriptProfile(text)
  const visibleCharCount = Array.from(text).filter((char) => !/\s/.test(char)).length
  const preserveSingleLine = script === 'cjk' && visibleCharCount > 0 && visibleCharCount <= 4
  const baseFontSize = Math.max(10, unit.fontSize * geometryContext.scaleY)
  const maxFontSize = Math.max(
    10,
    Math.min(baseFontSize, rect.height * 0.9, rect.width * (preserveSingleLine ? 0.92 : 1)),
  )
  const minFontSize = Math.min(10, maxFontSize)
  const lineHeightRatio = clamp(
    unit.fontSize > 0 ? unit.lineHeight / unit.fontSize : 1.12,
    script === 'cjk' ? 1.04 : 1.08,
    1.5,
  )

  const fontSize = fitFontSize(text, rect, script, preserveSingleLine, {
    min: minFontSize,
    max: maxFontSize,
    lineHeightRatio,
  })
  const lineHeight = Math.max(
    fontSize,
    Math.min(rect.height, fontSize * lineHeightRatio),
  )

  return {
    rect,
    fontSize,
    lineHeight,
    whiteSpace: preserveSingleLine ? 'nowrap' : 'pre-wrap',
    wordBreak: preserveSingleLine ? 'keep-all' : 'break-word',
    overflowWrap: preserveSingleLine ? 'normal' : 'anywhere',
  }
}

function fitFontSize(
  text: string,
  rect: OverlayCssRect,
  script: ScriptProfile,
  preserveSingleLine: boolean,
  bounds: {
    min: number
    max: number
    lineHeightRatio: number
  },
) {
  if (!text.trim()) {
    return bounds.min
  }

  let low = bounds.min
  let high = Math.max(bounds.max, bounds.min)
  let best = bounds.min

  for (let index = 0; index < 9; index += 1) {
    const candidate = (low + high) / 2
    if (textFits(text, rect, candidate, script, preserveSingleLine, bounds.lineHeightRatio)) {
      best = candidate
      low = candidate
    } else {
      high = candidate
    }
  }

  return roundMetric(best)
}

function textFits(
  text: string,
  rect: OverlayCssRect,
  fontSize: number,
  script: ScriptProfile,
  preserveSingleLine: boolean,
  lineHeightRatio: number,
) {
  const availableWidth = Math.max(rect.width, 1)
  const availableHeight = Math.max(rect.height, 1)
  const maxUnitsPerLine = Math.max(availableWidth / Math.max(fontSize, 1), 1)
  const layout = preserveSingleLine
    ? {
        lineCount: 1,
        widestLineUnits: measureTextUnits(text, script),
      }
    : wrapText(text, maxUnitsPerLine, script)
  const lineHeight = Math.max(fontSize, fontSize * lineHeightRatio)

  return (
    layout.widestLineUnits <= maxUnitsPerLine + 0.05 &&
    layout.lineCount * lineHeight <= availableHeight * 1.02
  )
}

function wrapText(
  text: string,
  maxUnitsPerLine: number,
  script: ScriptProfile,
) {
  const tokens = tokenizeText(text, script)
  let lineCount = 1
  let currentUnits = 0
  let widestLineUnits = 0

  const pushUnit = (unitWidth: number) => {
    if (currentUnits > 0 && currentUnits + unitWidth > maxUnitsPerLine) {
      widestLineUnits = Math.max(widestLineUnits, currentUnits)
      lineCount += 1
      currentUnits = 0
    }

    currentUnits += unitWidth
    widestLineUnits = Math.max(widestLineUnits, currentUnits)
  }

  for (const token of tokens) {
    const isWhitespace = token.trim().length === 0
    const tokenUnits = measureTextUnits(token, script)

    if (isWhitespace) {
      if (currentUnits === 0) {
        continue
      }
      if (currentUnits + tokenUnits <= maxUnitsPerLine) {
        currentUnits += tokenUnits
        widestLineUnits = Math.max(widestLineUnits, currentUnits)
      }
      continue
    }

    if (tokenUnits <= maxUnitsPerLine) {
      pushUnit(tokenUnits)
      continue
    }

    for (const char of Array.from(token)) {
      pushUnit(measureCharacterUnit(char, script))
    }
  }

  return {
    lineCount,
    widestLineUnits: Math.max(widestLineUnits, currentUnits),
  }
}

function tokenizeText(text: string, script: ScriptProfile) {
  if (script === 'cjk') {
    return Array.from(text)
  }

  return text.match(/\S+|\s+/g) ?? Array.from(text)
}

function detectScriptProfile(text: string): ScriptProfile {
  let cjkCount = 0
  let latinCount = 0

  for (const char of Array.from(text)) {
    if (isCjkLike(char)) {
      cjkCount += 1
      continue
    }
    if (/[A-Za-z0-9]/.test(char)) {
      latinCount += 1
    }
  }

  return cjkCount >= latinCount ? 'cjk' : 'latin'
}

function measureTextUnits(text: string, script: ScriptProfile) {
  return Array.from(text).reduce((sum, char) => sum + measureCharacterUnit(char, script), 0)
}

function measureCharacterUnit(char: string, script: ScriptProfile) {
  if (/\s/.test(char)) {
    return script === 'cjk' ? 0.35 : 0.32
  }

  if (isCjkLike(char)) {
    return 1
  }

  if (/[A-Z]/.test(char)) {
    return 0.7
  }

  if (/[a-z0-9]/.test(char)) {
    return 0.56
  }

  if (`,.;:!?()[]{}'"/\\|-`.includes(char)) {
    return 0.38
  }

  return script === 'cjk' ? 0.82 : 0.62
}

function isCjkLike(char: string) {
  return /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}]/u.test(char)
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max)
}

function roundMetric(value: number) {
  return Math.round(value * 100) / 100
}
