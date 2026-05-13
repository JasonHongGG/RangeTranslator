export type RuntimeStatus =
  | 'idle'
  | 'selecting'
  | 'capturing'
  | 'recognizing'
  | 'translating'
  | 'ready'
  | 'error'

export type TextAlign = 'left' | 'center' | 'right'

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
}

export type TranslationPayload = {
  selection: SelectionRect | null
  sourceLanguage: string
  targetLanguage: string
  detectedSource: string | null
  capturedAt: string | null
  unchanged: boolean
  blocks: OverlayBlock[]
}

export type RuntimeSnapshot = {
  running: boolean
  status: RuntimeStatus
  statusDetail: string
  sourceLanguage: string
  targetLanguage: string
  panelPinned: boolean
  selection: SelectionRect | null
  selectorBounds: SelectionRect | null
  copyMode: boolean
  endpoint: string
  model: string
  blockCount: number
  lastUpdated: string | null
  lastDetectedSource: string | null
  lastError: string | null
}