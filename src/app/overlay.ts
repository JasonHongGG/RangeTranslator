import { getCurrentWindow } from '@tauri-apps/api/window'
import { isTauri } from '../bridge'
import type {
  OverlayTranslationUnit,
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

  const baseline: TranslationPayload =
    partial.generation > current.generation
      ? {
          generation: partial.generation,
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

  if (next.generation > current.generation) {
    return next
  }

  const sourceMap = new Map(current.sourceUnits.map((unit) => [unit.id, unit]))
  for (const unit of next.sourceUnits) {
    const previous = sourceMap.get(unit.id)
    sourceMap.set(unit.id, previous ? { ...previous, ...unit } : unit)
  }

  const translationMap = new Map(
    current.translationUnits.map((unit) => [unit.sourceId, unit]),
  )
  for (const unit of next.translationUnits) {
    const previous = translationMap.get(unit.sourceId)
    if (previous && shouldKeepExistingTranslation(previous, unit)) {
      translationMap.set(unit.sourceId, previous)
    } else {
      translationMap.set(unit.sourceId, unit)
    }
  }

  return {
    ...next,
    capture: next.capture ?? current.capture,
    sourceUnits: Array.from(sourceMap.values()).sort((left, right) => left.order - right.order),
    translationUnits: Array.from(translationMap.values()).sort(
      (left, right) => left.order - right.order,
    ),
  }
}

function shouldKeepExistingTranslation(
  previous: OverlayTranslationUnit | undefined,
  next: OverlayTranslationUnit,
) {
  if (!previous) {
    return false
  }

  const nextIsEmptyWaitingState =
    (next.state === 'pending' || next.state === 'disabled') && !next.text.trim()
  const previousHasTranslation = previous.state === 'translated' && previous.text.trim()
  return Boolean(nextIsEmptyWaitingState && previousHasTranslation)
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
