import { getCurrentWindow } from '@tauri-apps/api/window'
import { isTauri } from '../bridge'
import type {
  OverlaySourceUnit,
  RuntimeStatus,
  SelectionRect,
  TranslationPartialPayload,
  TranslationPayload,
  VisibleLayer,
} from '../types'

export function normalizeSelection(
  start: { x: number; y: number },
  end: { x: number; y: number },
): SelectionRect {
  const left = Math.min(start.x, end.x)
  const top = Math.min(start.y, end.y)
  return {
    x: Math.round(left),
    y: Math.round(top),
    width: Math.round(Math.abs(end.x - start.x)),
    height: Math.round(Math.abs(end.y - start.y)),
  }
}

export function mergeTranslationPartial(
  current: TranslationPayload,
  partial: TranslationPartialPayload,
): TranslationPayload {
  if (partial.generation < current.generation) {
    return current
  }

  const replacesFrame =
    partial.generation > current.generation || partial.frameId !== current.frameId

  const baseline: TranslationPayload =
    replacesFrame
      ? {
          generation: partial.generation,
          frameId: partial.frameId,
          selection: partial.selection,
          capture: partial.capture,
          sourceLanguage: partial.sourceLanguage,
          targetLanguage: partial.targetLanguage,
          detectedSource: partial.detectedSource,
          capturedAt: partial.capturedAt,
          unchanged: false,
          visibleLayer: partial.visibleLayer,
          provider: partial.provider,
          promptProfile: partial.promptProfile,
          sourceUnits: [],
          translationUnits: [],
        }
      : current

  const sourceOrder = new Map(
    baseline.sourceUnits.map((unit, index) => [unit.id, unit.order ?? index]),
  )
  const sourceMap = new Map(baseline.sourceUnits.map((unit) => [unit.id, unit]))

  for (const unit of partial.sourceUnits) {
    const previous = sourceMap.get(unit.id)
    sourceMap.set(unit.id, previous ? { ...previous, ...unit } : unit)
    if (!sourceOrder.has(unit.id)) {
      sourceOrder.set(unit.id, unit.order ?? sourceOrder.size)
    }
  }

  const translationOrder = new Map(
    baseline.translationUnits.map((unit, index) => [unit.sourceId, unit.order ?? index]),
  )
  const translationMap = new Map(
    baseline.translationUnits.map((unit) => [unit.sourceId, unit]),
  )

  for (const unit of partial.translationUnits) {
    const previous = translationMap.get(unit.sourceId)
    translationMap.set(unit.sourceId, previous ? { ...previous, ...unit } : unit)
    if (!translationOrder.has(unit.sourceId)) {
      translationOrder.set(unit.sourceId, unit.order ?? translationOrder.size)
    }
  }

  const sourceUnits = Array.from(sourceMap.values()).sort(
    (left, right) => (sourceOrder.get(left.id) ?? 0) - (sourceOrder.get(right.id) ?? 0),
  )
  const translationUnits = Array.from(translationMap.values()).sort(
    (left, right) =>
      (translationOrder.get(left.sourceId) ?? 0) -
      (translationOrder.get(right.sourceId) ?? 0),
  )

  return {
    generation: partial.generation,
    frameId: partial.frameId,
    selection: partial.selection ?? baseline.selection,
    capture: partial.capture ?? baseline.capture,
    sourceLanguage: partial.sourceLanguage || baseline.sourceLanguage,
    targetLanguage: partial.targetLanguage || baseline.targetLanguage,
    detectedSource: partial.detectedSource ?? baseline.detectedSource,
    capturedAt: partial.capturedAt ?? baseline.capturedAt,
    unchanged: false,
    visibleLayer: (partial.visibleLayer || baseline.visibleLayer) as VisibleLayer,
    provider: partial.provider || baseline.provider,
    promptProfile: partial.promptProfile || baseline.promptProfile,
    sourceUnits,
    translationUnits,
  }
}

export function mergeTranslationUpdate(
  current: TranslationPayload,
  next: TranslationPayload,
): TranslationPayload {
  if (next.generation < current.generation) {
    return current
  }

  const canReuseCurrentCapture =
    next.generation === current.generation && next.frameId === current.frameId

  return {
    ...next,
    capture: next.capture ?? (canReuseCurrentCapture ? current.capture : null),
  }
}

export function sameSelection(
  left: SelectionRect | null | undefined,
  right: SelectionRect | null | undefined,
) {
  if (!left || !right) {
    return false
  }

  return (
    left.x === right.x &&
    left.y === right.y &&
    left.width === right.width &&
    left.height === right.height
  )
}

export type OverlayViewport = {
  width: number
  height: number
}

export type OverlayCssRect = {
  left: number
  top: number
  width: number
  height: number
}

export function resolveOverlayUnitRect(
  unit: OverlaySourceUnit,
  selection: SelectionRect | null | undefined,
  viewport: OverlayViewport,
  options?: {
    expandX?: number
    expandY?: number
  },
): OverlayCssRect {
  return resolveOverlayCssRect(unit.sourceRect, selection, viewport, options)
}

export function resolveOverlayCssRect(
  sourceRect: SelectionRect,
  selection: SelectionRect | null | undefined,
  viewport: OverlayViewport,
  options?: {
    expandX?: number
    expandY?: number
  },
): OverlayCssRect {
  const safeViewportWidth = Math.max(viewport.width, 1)
  const safeViewportHeight = Math.max(viewport.height, 1)
  const expandX = Math.max(0, options?.expandX ?? 0)
  const expandY = Math.max(0, options?.expandY ?? 0)

  if (!selection) {
    const left = Math.max(0, sourceRect.x - expandX)
    const top = Math.max(0, sourceRect.y - expandY)
    const right = left + Math.max(1, sourceRect.width) + expandX * 2
    const bottom = top + Math.max(1, sourceRect.height) + expandY * 2
    return {
      left,
      top,
      width: Math.max(1, right - left),
      height: Math.max(1, bottom - top),
    }
  }

  const scaleX = safeViewportWidth / Math.max(selection.width, 1)
  const scaleY = safeViewportHeight / Math.max(selection.height, 1)
  const left = Math.max(0, sourceRect.x * scaleX - expandX)
  const top = Math.max(0, sourceRect.y * scaleY - expandY)
  const right = Math.min(
    safeViewportWidth,
    (sourceRect.x + Math.max(1, sourceRect.width)) * scaleX + expandX,
  )
  const bottom = Math.min(
    safeViewportHeight,
    (sourceRect.y + Math.max(1, sourceRect.height)) * scaleY + expandY,
  )

  return {
    left,
    top,
    width: Math.max(1, right - left),
    height: Math.max(1, bottom - top),
  }
}

export function resolveOverlayTextMetrics(
  unit: OverlaySourceUnit,
  selection: SelectionRect | null | undefined,
  viewport: OverlayViewport,
) {
  const scaleY = selection
    ? Math.max(viewport.height, 1) / Math.max(selection.height, 1)
    : 1

  const fontSize = Math.max(10, unit.fontSize * scaleY)
  const lineHeight = Math.max(fontSize, unit.lineHeight * scaleY)

  return {
    fontSize,
    lineHeight,
  }
}

export function toLogicalPixels(value: number, scaleFactor: number) {
  return value / Math.max(scaleFactor, 1)
}

export async function readWindowScale(
  appWindow: ReturnType<typeof getCurrentWindow> | null,
) {
  if (!appWindow || !isTauri()) {
    return 1
  }

  try {
    return await appWindow.scaleFactor()
  } catch {
    return 1
  }
}

export function shouldIgnoreWindowDrag(target: HTMLElement) {
  return Boolean(target.closest('button, select, option, [data-no-drag="true"]'))
}

export function toneForStatus(status: RuntimeStatus) {
  switch (status) {
    case 'capturing':
    case 'recognizing':
    case 'translating':
      return 'live'
    case 'error':
      return 'danger'
    case 'ready':
      return 'ready'
    case 'selecting':
      return 'selecting'
    default:
      return 'idle'
  }
}

export function labelForStatus(status: RuntimeStatus) {
  switch (status) {
    case 'capturing':
      return 'Live'
    case 'recognizing':
      return 'OCR'
    case 'translating':
      return 'Wait'
    case 'selecting':
      return 'Pick'
    case 'ready':
      return 'Ready'
    case 'error':
      return 'Error'
    default:
      return 'Idle'
  }
}

export function withAlpha(color: string, alpha: number) {
  const normalized = color.replace('#', '')
  if (normalized.length !== 6) {
    return color
  }

  const suffix = Math.round(Math.max(0, Math.min(1, alpha)) * 255)
    .toString(16)
    .padStart(2, '0')
  return `#${normalized}${suffix}`
}
