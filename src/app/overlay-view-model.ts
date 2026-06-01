import type {
  OverlaySourceUnit,
  OverlayTranslationUnit,
  RuntimeSnapshot,
  TranslationPayload,
  VisibleLayer,
} from '../types'
import {
  createOverlayGeometryContext,
  type OverlayGeometryContext,
  type OverlayViewport,
} from './overlay-geometry'

export type OverlayRenderModel = {
  sourceUnits: OverlaySourceUnit[]
  translationBySourceId: Map<string, OverlayTranslationUnit>
  geometryContext: OverlayGeometryContext
  visibleLayer: VisibleLayer
  isInteractive: boolean
  allowsTextSelection: boolean
}

export function buildOverlayRenderModel(
  snapshot: RuntimeSnapshot,
  translation: TranslationPayload,
  viewport: OverlayViewport,
): OverlayRenderModel {
  return {
    sourceUnits: translation.sourceUnits,
    translationBySourceId: new Map(
      translation.translationUnits.map((unit) => [unit.sourceId, unit]),
    ),
    geometryContext: createOverlayGeometryContext(
      snapshot.selection ?? translation.selection,
      viewport,
    ),
    visibleLayer: translation.visibleLayer,
    isInteractive: snapshot.overlayMode !== 'passThrough',
    allowsTextSelection: snapshot.overlayMode === 'selectText',
  }
}

export function resolveOverlayDisplayText(
  visibleLayer: VisibleLayer,
  sourceUnit: OverlaySourceUnit,
  translationUnit: OverlayTranslationUnit | undefined,
) {
  const translatedText = translationUnit?.text.trim() ?? ''
  const hasTranslatedText = Boolean(
    translationUnit &&
      (translationUnit.state === 'translated' || translationUnit.streaming) &&
      translatedText,
  )

  if (visibleLayer === 'ocr') {
    return sourceUnit.sourceText
  }

  if (hasTranslatedText) {
    return translatedText
  }

  return sourceUnit.sourceText
}
