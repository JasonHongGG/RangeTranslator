import { getCurrentWindow } from '@tauri-apps/api/window'
import { isTauri } from '../bridge'
import type {
  RuntimeStatus,
  SelectionRect,
  TranslationPartialPayload,
  TranslationPayload,
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
  const order = new Map(current.blocks.map((block, index) => [block.id, index]))
  const blockMap = new Map(current.blocks.map((block) => [block.id, block]))

  for (const block of partial.blocks) {
    const previous = blockMap.get(block.id)
    blockMap.set(block.id, previous ? { ...previous, ...block } : block)
    if (!order.has(block.id)) {
      order.set(block.id, order.size)
    }
  }

  const blocks = Array.from(blockMap.values()).sort(
    (left, right) => (order.get(left.id) ?? 0) - (order.get(right.id) ?? 0),
  )

  return {
    selection: partial.selection ?? current.selection,
    sourceLanguage: partial.sourceLanguage || current.sourceLanguage,
    targetLanguage: partial.targetLanguage || current.targetLanguage,
    detectedSource: partial.detectedSource ?? current.detectedSource,
    capturedAt: partial.capturedAt ?? current.capturedAt,
    unchanged: false,
    provider: partial.provider || current.provider,
    promptProfile: partial.promptProfile || current.promptProfile,
    blocks,
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
