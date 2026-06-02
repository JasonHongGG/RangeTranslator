import { resolveOverlayUnitRect, type OverlayGeometryContext } from '../../app/overlay-geometry'
import { resolveOverlayStyle } from '../../app/overlay-style'
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
    const rect = resolveOverlayUnitRect(block, geometryContext)
    const resolvedStyle = resolveOverlayStyle(block)

    return (
      <div
        key={`bg-${block.id}`}
        className="overlay-backdrop"
        style={{
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
          background: resolvedStyle.background,
        }}
      />
    )
  })
}
