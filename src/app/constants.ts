import type { RuntimeSnapshot, TranslationPayload } from '../types'

export const EMPTY_SNAPSHOT: RuntimeSnapshot = {
  running: false,
  status: 'idle',
  statusDetail: 'Ready',
  sourceLanguage: 'auto',
  targetLanguage: 'zh-TW',
  ocrProvider: '',
  aiProvider: '',
  aiTranslationEnabled: true,
  panelPinned: false,
  debugScreenshotMode: false,
  selection: null,
  selectorBounds: null,
  overlayMode: 'selectText',
  generation: 0,
  visibleLayer: 'none',
  blockCount: 0,
  lastUpdated: null,
  lastDetectedSource: null,
  lastError: null,
}

export const EMPTY_TRANSLATION: TranslationPayload = {
  generation: 0,
  frameId: '',
  selection: null,
  capture: null,
  sourceLanguage: 'auto',
  targetLanguage: 'zh-TW',
  detectedSource: null,
  capturedAt: null,
  unchanged: false,
  visibleLayer: 'none',
  provider: '',
  sourceUnits: [],
  translationUnits: [],
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
