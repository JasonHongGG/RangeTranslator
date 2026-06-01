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
  frameId: 'preview:0',
  selection: PREVIEW_SNAPSHOT.selection,
  capture: null,
  sourceLanguage: 'auto',
  targetLanguage: 'zh-TW',
  detectedSource: 'ja-JP',
  capturedAt: null,
  unchanged: false,
  visibleLayer: 'translation',
  provider: 'ollama',
  promptProfile: 'translation.ui_overlay.default',
  sourceUnits: [
    {
      id: 'preview-1',
      frameId: 'preview:0',
      order: 0,
      sourceText: 'スキャン開始',
      sourceRect: { x: 46, y: 40, width: 178, height: 40 },
      renderRect: { x: 46, y: 40, width: 178, height: 40 },
      fontSize: 24,
      lineHeight: 28,
      confidence: 0.92,
      foreground: '#F6E7C8',
      background: '#1C2229',
      align: 'left',
    },
    {
      id: 'preview-2',
      frameId: 'preview:0',
      order: 1,
      sourceText: '読み込み中',
      sourceRect: { x: 402, y: 116, width: 146, height: 34 },
      renderRect: { x: 402, y: 116, width: 146, height: 34 },
      fontSize: 20,
      lineHeight: 24,
      confidence: 0.88,
      foreground: '#F7F5F2',
      background: '#113A45',
      align: 'center',
    },
    {
      id: 'preview-3',
      frameId: 'preview:0',
      order: 2,
      sourceText: '設定を保存',
      sourceRect: { x: 512, y: 232, width: 176, height: 38 },
      renderRect: { x: 512, y: 232, width: 176, height: 38 },
      fontSize: 23,
      lineHeight: 27,
      confidence: 0.9,
      foreground: '#172127',
      background: '#F9DFC6',
      align: 'left',
    },
  ],
  translationUnits: [
    {
      sourceId: 'preview-1',
      order: 0,
      text: '開始掃描',
      state: 'translated',
      confidence: 0.92,
      streaming: false,
    },
    {
      sourceId: 'preview-2',
      order: 1,
      text: '載入中',
      state: 'translated',
      confidence: 0.88,
      streaming: false,
    },
    {
      sourceId: 'preview-3',
      order: 2,
      text: '儲存設定',
      state: 'translated',
      confidence: 0.9,
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

export type AppView = 'panel' | 'selector' | 'overlay' | 'settings'

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
