import type { OverlaySourceUnit, SelectionRect } from '../types.ts'

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

export type OverlayGeometryContext = {
  selection: SelectionRect | null
  viewport: OverlayViewport
  scaleX: number
  scaleY: number
}

export function createOverlayGeometryContext(
  selection: SelectionRect | null | undefined,
  viewport: OverlayViewport,
): OverlayGeometryContext {
  const safeViewportWidth = Math.max(viewport.width, 1)
  const safeViewportHeight = Math.max(viewport.height, 1)
  const resolvedSelection = selection ?? null

  if (!resolvedSelection) {
    return {
      selection: null,
      viewport: {
        width: safeViewportWidth,
        height: safeViewportHeight,
      },
      scaleX: 1,
      scaleY: 1,
    }
  }

  return {
    selection: resolvedSelection,
    viewport: {
      width: safeViewportWidth,
      height: safeViewportHeight,
    },
    scaleX: safeViewportWidth / Math.max(resolvedSelection.width, 1),
    scaleY: safeViewportHeight / Math.max(resolvedSelection.height, 1),
  }
}

export function resolveOverlayUnitRect(
  unit: OverlaySourceUnit,
  context: OverlayGeometryContext,
  options?: {
    expandX?: number
    expandY?: number
  },
): OverlayCssRect {
  return resolveOverlayCssRect(unit.sourceRect, context, options)
}

export function resolveOverlayCssRect(
  sourceRect: SelectionRect,
  context: OverlayGeometryContext,
  options?: {
    expandX?: number
    expandY?: number
  },
): OverlayCssRect {
  const expandX = Math.max(0, options?.expandX ?? 0)
  const expandY = Math.max(0, options?.expandY ?? 0)

  if (!context.selection) {
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

  const left = Math.max(0, sourceRect.x * context.scaleX - expandX)
  const top = Math.max(0, sourceRect.y * context.scaleY - expandY)
  const right = Math.min(
    context.viewport.width,
    (sourceRect.x + Math.max(1, sourceRect.width)) * context.scaleX + expandX,
  )
  const bottom = Math.min(
    context.viewport.height,
    (sourceRect.y + Math.max(1, sourceRect.height)) * context.scaleY + expandY,
  )

  return {
    left,
    top,
    width: Math.max(1, right - left),
    height: Math.max(1, bottom - top),
  }
}
