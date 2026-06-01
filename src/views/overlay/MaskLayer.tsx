import { resolveOverlayUnitRect, type OverlayGeometryContext } from '../../app/overlay-geometry'
import type { OverlaySourceUnit } from '../../types'

type OverlayMaskLayerProps = {
  visible: boolean
  sourceUnits: OverlaySourceUnit[]
  geometryContext: OverlayGeometryContext
}

export function OverlayMaskLayer({
  visible,
  sourceUnits,
  geometryContext,
}: OverlayMaskLayerProps) {
  if (!visible) {
    return null
  }

  return sourceUnits.map((block) => {
    const rect = resolveOverlayUnitRect(block, geometryContext, {
      expandX: 1,
      expandY: 1,
    })

    return (
      <div
        key={`bg-${block.id}`}
        className="overlay-backdrop"
        style={{
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
          background: block.background,
        }}
      />
    )
  })
}
