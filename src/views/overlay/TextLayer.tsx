import { resolveOverlayTextLayout } from '../../app/overlay-text-layout'
import {
  resolveOverlayDisplayText,
  type OverlayRenderModel,
} from '../../app/overlay-view-model'

type OverlayTextLayerProps = Pick<
  OverlayRenderModel,
  'sourceUnits' | 'translationBySourceId' | 'geometryContext' | 'visibleLayer' | 'allowsTextSelection'
>

export function OverlayTextLayer({
  sourceUnits,
  translationBySourceId,
  geometryContext,
  visibleLayer,
  allowsTextSelection,
}: OverlayTextLayerProps) {
  if (visibleLayer === 'none') {
    return null
  }

  return sourceUnits.map((block) => {
    const translationUnit = translationBySourceId.get(block.id)
    const text = resolveOverlayDisplayText(visibleLayer, block, translationUnit)

    if (!text) {
      return null
    }

    const textLayout = resolveOverlayTextLayout(block, text, geometryContext)

    return (
      <article
        key={block.id}
        className={`overlay-block overlay-block-${block.align} ${translationUnit?.streaming ? 'overlay-block-streaming' : ''}`}
        data-no-drag={allowsTextSelection ? 'true' : undefined}
        style={{
          left: textLayout.rect.left,
          top: textLayout.rect.top,
          width: textLayout.rect.width,
          height: textLayout.rect.height,
          color: block.foreground,
          fontSize: textLayout.fontSize,
          lineHeight: `${textLayout.lineHeight}px`,
          whiteSpace: textLayout.whiteSpace,
          wordBreak: textLayout.wordBreak,
          overflowWrap: textLayout.overflowWrap,
        }}
      >
        {text}
      </article>
    )
  })
}
