export type RuntimeStatus =
  | 'idle'
  | 'selecting'
  | 'capturing'
  | 'recognizing'
  | 'translating'
  | 'ready'
  | 'error'

export type VisibleLayer = 'none' | 'ocr' | 'translation'

export type TextAlign = 'left' | 'center' | 'right'
export type PartialUpdateStage = 'ocr' | 'translation' | 'complete'

export type SelectionRect = {
  x: number
  y: number
  width: number
  height: number
}

export type OverlayBlock = {
  id: string
  sourceText: string
  translatedText: string
  x: number
  y: number
  width: number
  height: number
  fontSize: number
  confidence: number
  foreground: string
  background: string
  align: TextAlign
  streaming: boolean
}

export type TranslationPayload = {
  generation: number
  selection: SelectionRect | null
  sourceLanguage: string
  targetLanguage: string
  detectedSource: string | null
  capturedAt: string | null
  unchanged: boolean
  visibleLayer: VisibleLayer
  provider: string
  promptProfile: string
  blocks: OverlayBlock[]
}

export type TranslationPartialPayload = {
  generation: number
  selection: SelectionRect | null
  sourceLanguage: string
  targetLanguage: string
  detectedSource: string | null
  capturedAt: string | null
  visibleLayer: VisibleLayer
  provider: string
  promptProfile: string
  stage: PartialUpdateStage
  complete: boolean
  blocks: OverlayBlock[]
}

export type ProviderDescriptor = {
  id: string
  label: string
  kind: string
  available: boolean
  detail: string | null
}

export type PromptProfileDescriptor = {
  id: string
  label: string
  version: string
  task: string
  providerFamily: string
}

export type RuntimeCapabilities = {
  ocrProviders: ProviderDescriptor[]
  aiProviders: ProviderDescriptor[]
  promptProfiles: PromptProfileDescriptor[]
  defaultOcrProviderId: string | null
  defaultAiProviderId: string | null
  defaultPromptProfileId: string | null
}

export type BenchmarkCaseResult = {
  caseId: string
  promptProfile: string
  providerId: string
  expectedTranslations: string[]
  actualTranslations: string[]
  exactMatchScore: number
  latencyMs: number
}

export type BenchmarkReport = {
  suiteId: string
  providerId: string
  promptProfile: string
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
  promptProfile: string
  panelPinned: boolean
  debugScreenshotMode: boolean
  selection: SelectionRect | null
  selectorBounds: SelectionRect | null
  copyMode: boolean
  endpoint: string
  model: string
  generation: number
  visibleLayer: VisibleLayer
  blockCount: number
  lastUpdated: string | null
  lastDetectedSource: string | null
  lastError: string | null
}