export type RuntimeStatus =
  | 'idle'
  | 'selecting'
  | 'capturing'
  | 'recognizing'
  | 'translating'
  | 'ready'
  | 'error'

export type OverlayInteractionMode = 'passThrough' | 'selectText' | 'dragWindow'

export type VisibleLayer = 'none' | 'ocr' | 'translation'

export type TextAlign = 'left' | 'center' | 'right'
export type PartialUpdateStage = 'ocr' | 'translation' | 'complete'
export type TranslationUnitState =
  | 'pending'
  | 'translated'
  | 'missing'
  | 'failed'
  | 'disabled'

export type SelectionRect = {
  x: number
  y: number
  width: number
  height: number
}

export type CaptureCoordinateSpace = 'selectionPhysicalPixels'

export type CaptureMetadata = {
  coordinateSpace: CaptureCoordinateSpace
  displayOriginX: number
  displayOriginY: number
  displayWidth: number
  displayHeight: number
  captureOriginX: number
  captureOriginY: number
  captureWidth: number
  captureHeight: number
  scaleFactor: number
}

export type OverlaySourceUnit = {
  id: string
  frameId: string
  order: number
  sourceText: string
  sourceRect: SelectionRect
  fontSize: number
  lineHeight: number
  confidence: number
  foreground: string
  background: string
  styleConfidence: number
  align: TextAlign
}

export type OverlayTranslationUnit = {
  sourceId: string
  order: number
  text: string
  state: TranslationUnitState
  confidence: number
  streaming: boolean
}

export type TranslationPayload = {
  generation: number
  frameId: string
  selection: SelectionRect | null
  capture: CaptureMetadata | null
  sourceLanguage: string
  targetLanguage: string
  detectedSource: string | null
  capturedAt: string | null
  unchanged: boolean
  visibleLayer: VisibleLayer
  provider: string
  sourceUnits: OverlaySourceUnit[]
  translationUnits: OverlayTranslationUnit[]
}

export type TranslationPartialPayload = {
  generation: number
  frameId: string
  selection: SelectionRect | null
  capture: CaptureMetadata | null
  sourceLanguage: string
  targetLanguage: string
  detectedSource: string | null
  capturedAt: string | null
  visibleLayer: VisibleLayer
  provider: string
  stage: PartialUpdateStage
  complete: boolean
  sourceUnits: OverlaySourceUnit[]
  translationUnits: OverlayTranslationUnit[]
}

export type ProviderDescriptor = {
  id: string
  label: string
  kind: string
  available: boolean
  detail: string | null
}

}

export type RuntimeCapabilities = {
  ocrProviders: ProviderDescriptor[]
  aiProviders: ProviderDescriptor[]
  defaultOcrProviderId: string | null
  defaultAiProviderId: string | null
}

export type BenchmarkCaseResult = {
  caseId: string
  providerId: string
  expectedTranslations: string[]
  actualTranslations: string[]
  exactMatchScore: number
  latencyMs: number
}

export type BenchmarkReport = {
  suiteId: string
  providerId: string
  caseCount: number
  averageExactMatchScore: number
  averageLatencyMs: number
  cases: BenchmarkCaseResult[]
}

export type RuntimeSnapshot = {
  running: boolean
  status: RuntimeStatus
  statusDetail: string
  sourceLanguage: string
  targetLanguage: string
  ocrProvider: string
  aiProvider: string
  aiTranslationEnabled: boolean
  panelPinned: boolean
  debugScreenshotMode: boolean
  selection: SelectionRect | null
  selectorBounds: SelectionRect | null
  overlayMode: OverlayInteractionMode
  endpoint: string
  model: string
  generation: number
  visibleLayer: VisibleLayer
  blockCount: number
  lastUpdated: string | null
  lastDetectedSource: string | null
  lastError: string | null
}