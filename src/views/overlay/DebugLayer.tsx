import { resolveOverlayUnitRect, type OverlayGeometryContext } from '../../app/overlay-geometry'
import type { OverlaySourceUnit } from '../../types'

type OverlayDebugLayerProps = {
  sourceUnits: OverlaySourceUnit[]
  geometryContext: OverlayGeometryContext
}

export function OverlayDebugLayer({
  sourceUnits,
  geometryContext,
}: OverlayDebugLayerProps) {
  return sourceUnits.map((block) => {
    const rect = resolveOverlayUnitRect(block, geometryContext)

    return (
      <div
        key={`ocr-box-${block.id}`}
        className="overlay-ocr-box"
        style={rect}
      />
    )
  })
}
