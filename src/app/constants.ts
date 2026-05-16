import type { RuntimeSnapshot, TranslationPayload } from '../types'

export const PREVIEW_SNAPSHOT: RuntimeSnapshot = {
  running: false,
  status: 'ready',
  statusDetail: 'Preview',
  sourceLanguage: 'auto',
  targetLanguage: 'zh-TW',
  ocrProvider: 'paddleocr',
  aiProvider: 'ollama',
  promptProfile: 'translation.ui_overlay.default',
  aiTranslationEnabled: true,
  panelPinned: true,
  debugScreenshotMode: false,
  selection: { x: 280, y: 180, width: 764, height: 312 },
  selectorBounds: { x: 0, y: 0, width: 1280, height: 720 },
  overlayMode: 'selectText',
  endpoint: 'https://lacresha-posological-steven.ngrok-free.dev',
  model: 'discovering',
  generation: 0,
  visibleLayer: 'translation',
  blockCount: 3,
  lastUpdated: null,
  lastDetectedSource: 'ja-JP',
  lastError: null,
}

export const PREVIEW_TRANSLATION: TranslationPayload = {
  generation: 0,
  selection: PREVIEW_SNAPSHOT.selection,
  sourceLanguage: 'auto',
  targetLanguage: 'zh-TW',
  detectedSource: 'ja-JP',
  capturedAt: null,
  unchanged: false,
  visibleLayer: 'translation',
  provider: 'ollama',
  promptProfile: 'translation.ui_overlay.default',
  blocks: [
    {
      id: 'preview-1',
      sourceText: 'スキャン開始',
      translatedText: '開始掃描',
      x: 46,
      y: 40,
      width: 178,
      height: 40,
      fontSize: 24,
      confidence: 0.92,
      foreground: '#F6E7C8',
      background: '#1C2229',
      align: 'left',
      streaming: false,
    },
    {
      id: 'preview-2',
      sourceText: '読み込み中',
      translatedText: '載入中',
      x: 402,
      y: 116,
      width: 146,
      height: 34,
      fontSize: 20,
      confidence: 0.88,
      foreground: '#F7F5F2',
      background: '#113A45',
      align: 'center',
      streaming: false,
    },
    {
      id: 'preview-3',
      sourceText: '設定を保存',
      translatedText: '儲存設定',
      x: 512,
      y: 232,
      width: 176,
      height: 38,
      fontSize: 23,
      confidence: 0.9,
      foreground: '#172127',
      background: '#F9DFC6',
      align: 'left',
      streaming: false,
    },
  ],
}

export const DEBUG_EVENT = 'selector-debug'

export type DebugPayload = {
  scope: string
  message: string
  detail: unknown
  timestamp: string
}

export type AppView = 'panel' | 'selector' | 'overlay'

export type RouteInfo = {
  view: AppView
  scriptView: string | null
  currentWindowLabel: string | null
  href: string
  pathname: string
  search: string
  hash: string
  queryView: string | null
  hashView: string | null
  pathView: string | null
}

export type ResizeDirection =
  | 'East'
  | 'North'
  | 'NorthEast'
  | 'NorthWest'
  | 'South'
  | 'SouthEast'
  | 'SouthWest'
  | 'West'

export const PANEL_RESIZE_HANDLES: Array<{
  direction: ResizeDirection
  className: string
}> = [
  { direction: 'North', className: 'resize-n' },
  { direction: 'South', className: 'resize-s' },
  { direction: 'East', className: 'resize-e' },
  { direction: 'West', className: 'resize-w' },
  { direction: 'NorthEast', className: 'resize-ne' },
  { direction: 'NorthWest', className: 'resize-nw' },
  { direction: 'SouthEast', className: 'resize-se' },
  { direction: 'SouthWest', className: 'resize-sw' },
]
