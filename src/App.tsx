import {
  startTransition,
  useDeferredValue,
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
} from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { call, isTauri, watchEvent } from './bridge'
import { SOURCE_LANGUAGES, TARGET_LANGUAGES } from './languages'
import { CompactSelect } from './components/CompactSelect'
import type {
  RuntimeSnapshot,
  RuntimeStatus,
  SelectionRect,
  TranslationPayload,
} from './types'

const PREVIEW_SNAPSHOT: RuntimeSnapshot = {
  running: false,
  status: 'ready',
  statusDetail: 'Preview',
  sourceLanguage: 'auto',
  targetLanguage: 'zh-TW',
  selection: { x: 280, y: 180, width: 764, height: 312 },
  selectorBounds: { x: 0, y: 0, width: 1280, height: 720 },
  copyMode: false,
  endpoint: 'https://lacresha-posological-steven.ngrok-free.dev',
  model: 'discovering',
  blockCount: 3,
  lastUpdated: null,
  lastDetectedSource: 'ja-JP',
  lastError: null,
}

const PREVIEW_TRANSLATION: TranslationPayload = {
  selection: PREVIEW_SNAPSHOT.selection,
  sourceLanguage: 'auto',
  targetLanguage: 'zh-TW',
  detectedSource: 'ja-JP',
  capturedAt: null,
  unchanged: false,
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
    },
  ],
}

const DEBUG_EVENT = 'selector-debug'

type DebugPayload = {
  scope: string
  message: string
  detail: unknown
  timestamp: string
}

type AppView = 'panel' | 'selector' | 'overlay'

type RouteInfo = {
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

type ResizeDirection =
  | 'East'
  | 'North'
  | 'NorthEast'
  | 'NorthWest'
  | 'South'
  | 'SouthEast'
  | 'SouthWest'
  | 'West'

const PANEL_RESIZE_HANDLES: Array<{
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

function App() {
  const route = useMemo(() => resolveRouteInfo(), [])

  useEffect(() => {
    writeLocalDebug('app-router', 'boot', route)
  }, [route])

  useEffect(() => {
    document.documentElement.dataset.appView = route.view
    document.body.dataset.appView = route.view

    const root = document.getElementById('root')
    if (root) {
      root.dataset.appView = route.view
    }
  }, [route.view])

  useEffect(() => {
    if (route.view !== 'selector') {
      return
    }

    let moveLogged = false

    const onPointerDown = (event: PointerEvent) => {
      writeLocalDebug('selector-capture', 'window pointerdown', {
        pointerId: event.pointerId,
        button: event.button,
        buttons: event.buttons,
        point: { x: event.clientX, y: event.clientY },
        target: describeTarget(event.target),
      })
    }

    const onPointerMove = (event: PointerEvent) => {
      if (moveLogged) {
        return
      }
      moveLogged = true
      writeLocalDebug('selector-capture', 'window pointermove', {
        pointerId: event.pointerId,
        buttons: event.buttons,
        point: { x: event.clientX, y: event.clientY },
      })
    }

    const onPointerUp = (event: PointerEvent) => {
      moveLogged = false
      writeLocalDebug('selector-capture', 'window pointerup', {
        pointerId: event.pointerId,
        button: event.button,
        buttons: event.buttons,
        point: { x: event.clientX, y: event.clientY },
      })
    }

    const onMouseDown = (event: MouseEvent) => {
      writeLocalDebug('selector-capture', 'window mousedown', {
        button: event.button,
        buttons: event.buttons,
        point: { x: event.clientX, y: event.clientY },
      })
    }

    const onKeyDown = (event: KeyboardEvent) => {
      writeLocalDebug('selector-capture', 'window keydown', {
        key: event.key,
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
        metaKey: event.metaKey,
      })
    }

    window.addEventListener('pointerdown', onPointerDown, true)
    window.addEventListener('pointermove', onPointerMove, true)
    window.addEventListener('pointerup', onPointerUp, true)
    window.addEventListener('mousedown', onMouseDown, true)
    window.addEventListener('keydown', onKeyDown, true)

    return () => {
      window.removeEventListener('pointerdown', onPointerDown, true)
      window.removeEventListener('pointermove', onPointerMove, true)
      window.removeEventListener('pointerup', onPointerUp, true)
      window.removeEventListener('mousedown', onMouseDown, true)
      window.removeEventListener('keydown', onKeyDown, true)
    }
  }, [route])

  if (route.view === 'selector') {
    return <SelectorView />
  }

  if (route.view === 'overlay') {
    return <OverlayView />
  }

  return <PanelView />
}

function PanelView() {
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot>(PREVIEW_SNAPSHOT)
  const [sourceLanguage, setSourceLanguage] = useState('auto')
  const [targetLanguage, setTargetLanguage] = useState('zh-TW')
  const [busy, setBusy] = useState(false)
  const panelWindow = useMemo(() => (isTauri() ? getCurrentWindow() : null), [])

  const applySnapshot = useEffectEvent((next: RuntimeSnapshot) => {
    startTransition(() => {
      setSnapshot(next)
    })
  })

  useEffect(() => {
    let cancelled = false
    const bootstrap = async () => {
      if (!isTauri()) {
        return
      }

      try {
        const next = await call<RuntimeSnapshot>('get_runtime_snapshot')
        if (!cancelled) {
          setSnapshot(next)
          setSourceLanguage(next.sourceLanguage)
          setTargetLanguage(next.targetLanguage)
        }
      } catch {
        // Browser preview keeps the embedded sample state.
      }
    }

    void bootstrap()

    let detach = () => {}
    void watchEvent<RuntimeSnapshot>('runtime-snapshot', applySnapshot).then(
      (unlisten) => {
        detach = unlisten
      },
    )

    return () => {
      cancelled = true
      detach()
    }
  }, [])

  useEffect(() => {
    if (!isTauri()) {
      return
    }

    let detach = () => {}
    void watchEvent<DebugPayload>(DEBUG_EVENT, (payload) => {
      logDebugPayload(payload)
    }).then((unlisten) => {
      detach = unlisten
    })

    return () => {
      detach()
    }
  }, [])

  const selectionLabel = snapshot.selection
    ? `${snapshot.selection.width} x ${snapshot.selection.height}`
    : 'No region'
  const statusTone = toneForStatus(snapshot.status)

  const runCommand = async (action: () => Promise<void>) => {
    if (!isTauri()) {
      return
    }

    setBusy(true)
    try {
      await action()
    } finally {
      setBusy(false)
    }
  }

  const startDrag = async (event: React.PointerEvent<HTMLElement>) => {
    if (!panelWindow || event.button !== 0) {
      return
    }

    const target = event.target as HTMLElement
    if (shouldIgnoreWindowDrag(target)) {
      return
    }

    try {
      await panelWindow.startDragging()
    } catch {
      // Ignore drag errors triggered by secondary interactions.
    }
  }

  const startResize = (direction: ResizeDirection) => async (
    event: React.PointerEvent<HTMLButtonElement>,
  ) => {
    if (!panelWindow || event.button !== 0) {
      return
    }

    event.preventDefault()
    event.stopPropagation()
    try {
      await panelWindow.startResizeDragging(direction)
    } catch {
      // Ignore resize errors if the host rejects the gesture.
    }
  }

  const handleHotkey = useEffectEvent((event: KeyboardEvent) => {
    if (busy || !isTauri() || event.repeat) {
      return
    }

    const key = event.key.toLowerCase()
    if ((event.ctrlKey || event.metaKey) && event.shiftKey && key === 's') {
      event.preventDefault()
      void runCommand(() => call('open_selector_window'))
      return
    }

    if ((event.ctrlKey || event.metaKey) && event.shiftKey && key === 'c') {
      if (snapshot.blockCount === 0) {
        return
      }
      event.preventDefault()
      void runCommand(() =>
        call('toggle_copy_mode', { enabled: !snapshot.copyMode }),
      )
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 'enter') {
      event.preventDefault()
      void runCommand(() =>
        call(snapshot.running ? 'stop_pipeline' : 'start_pipeline', {
          settings: { sourceLanguage, targetLanguage },
        }),
      )
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 'backspace' && snapshot.selection) {
      event.preventDefault()
      void runCommand(() => call('clear_selection'))
    }
  })

  useEffect(() => {
    window.addEventListener('keydown', handleHotkey)
    return () => {
      window.removeEventListener('keydown', handleHotkey)
    }
  }, [])

  return (
    <main className="panel-window" onPointerDown={startDrag}>
      {PANEL_RESIZE_HANDLES.map((handle) => (
        <button
          key={handle.direction}
          type="button"
          className={`resize-handle ${handle.className}`}
          aria-label={`Resize ${handle.direction}`}
          onPointerDown={startResize(handle.direction)}
        ></button>
      ))}

      <header className="panel-header">
        <div className="brand-lockup" data-tauri-drag-region>
          <span className="brand-mark" aria-hidden="true">
            <IconScan />
          </span>
          <div className="brand-copy">
            <strong>RangeTranslator</strong>
            <span>live</span>
          </div>
        </div>

        <div className="panel-drag-spacer" data-tauri-drag-region aria-hidden="true"></div>

        <div className="panel-actions" data-no-drag="true">
          <button
            type="button"
            className={`panel-icon-button panel-icon-button-primary ${
              snapshot.running ? 'panel-icon-button-active' : ''
            }`}
            title={snapshot.running ? 'Stop live translation' : 'Start live translation'}
            aria-label={snapshot.running ? 'Stop live translation' : 'Start live translation'}
            disabled={busy || (!snapshot.running && !snapshot.selection)}
            onClick={() =>
              runCommand(() =>
                call(snapshot.running ? 'stop_pipeline' : 'start_pipeline', {
                  settings: { sourceLanguage, targetLanguage },
                }),
              )
            }
          >
            {snapshot.running ? <IconPause /> : <IconPlay />}
          </button>

          <button
            type="button"
            className="panel-icon-button"
            title="Select screen region"
            aria-label="Select screen region"
            disabled={busy}
            onClick={() => runCommand(() => call('open_selector_window'))}
          >
            <IconCrop />
          </button>

          <span className="panel-actions-divider" aria-hidden="true"></span>

          <button
            type="button"
            className="window-button"
            aria-label="Minimize"
            onClick={() => runCommand(() => call('panel_minimize'))}
          >
            <IconMinimize />
          </button>
          <button
            type="button"
            className="window-button window-button-danger"
            aria-label="Close"
            onClick={() => runCommand(() => call('panel_close'))}
          >
            <IconClose />
          </button>
        </div>
      </header>

      <section className="panel-fields">
        <CompactSelect
          label="Source"
          icon={<IconWave />}
          value={sourceLanguage}
          disabled={snapshot.running || busy}
          options={SOURCE_LANGUAGES}
          onChange={setSourceLanguage}
          menuSide="bottom"
        />
        <CompactSelect
          label="Target"
          icon={<IconTranslate />}
          value={targetLanguage}
          disabled={snapshot.running || busy}
          options={TARGET_LANGUAGES}
          onChange={setTargetLanguage}
          menuSide="top"
        />
      </section>

      <footer className="panel-footer">
        <div
          className={`panel-status-rail panel-status-rail-${statusTone}`}
          data-tauri-drag-region
        >
          <span className="status-dot"></span>
          <span className="panel-status-copy">{labelForStatus(snapshot.status)}</span>
          <span className="panel-status-divider" aria-hidden="true"></span>
          <span
            className={`panel-region-copy ${
              snapshot.selection ? 'panel-region-copy-active' : ''
            }`}
          >
            {selectionLabel}
          </span>
        </div>

        <div className="footer-tools" data-no-drag="true">
          <button
            type="button"
            className={`footer-icon ${snapshot.copyMode ? 'footer-icon-active' : ''}`}
            disabled={busy || snapshot.blockCount === 0}
            aria-label="Toggle copy mode"
            onClick={() =>
              runCommand(() =>
                call('toggle_copy_mode', { enabled: !snapshot.copyMode }),
              )
            }
          >
            <IconCopy />
          </button>
          <button
            type="button"
            className="footer-icon"
            disabled={busy || !snapshot.selection}
            aria-label="Clear selection"
            onClick={() => runCommand(() => call('clear_selection'))}
          >
            <IconErase />
          </button>
        </div>
      </footer>

      {snapshot.lastError ? (
        <div className="panel-error">{snapshot.lastError}</div>
      ) : null}
    </main>
  )
}

function SelectorView() {
  const [origin, setOrigin] = useState(() => selectorOrigin())
  const [anchor, setAnchor] = useState<{ x: number; y: number } | null>(null)
  const [current, setCurrent] = useState<{ x: number; y: number } | null>(null)
  const [lastDebugLine, setLastDebugLine] = useState('Selector mounted')
  const anchorRef = useRef<{ x: number; y: number } | null>(null)
  const dragMoveLoggedRef = useRef(false)
  const lifecycleBusyRef = useRef(false)
  const selectorWindow = useMemo(() => (isTauri() ? getCurrentWindow() : null), [])
  const currentWindowLabel = isTauri() ? getCurrentWindow().label : 'browser'
  const injectedView = readInjectedView() ?? 'none'

  const selection = useMemo(() => {
    if (!anchor || !current) {
      return null
    }

    const left = Math.min(anchor.x, current.x)
    const top = Math.min(anchor.y, current.y)
    return {
      x: Math.round(left),
      y: Math.round(top),
      width: Math.round(Math.abs(current.x - anchor.x)),
      height: Math.round(Math.abs(current.y - anchor.y)),
    }
  }, [anchor, current])

  const requestSelectorCancel = useEffectEvent(async (reason: string) => {
    if (lifecycleBusyRef.current) {
      return
    }

    lifecycleBusyRef.current = true
    writeLocalDebug('selector-ui', 'cancel requested', { reason })

    try {
      await call('close_selector_window')
    } catch (error) {
      lifecycleBusyRef.current = false
      writeLocalDebug('selector-ui', 'cancel request failed', {
        reason,
        error: formatUnknown(error),
      })
    }
  })

  useEffect(() => {
    let detach = () => {}
    if (isTauri()) {
      void watchEvent<DebugPayload>(DEBUG_EVENT, (payload) => {
        const line = formatDebugLine(payload)
        setLastDebugLine(line)
        logDebugPayload(payload)
      }).then((unlisten) => {
        detach = unlisten
      })
    }

    let detachSnapshot = () => {}
    if (isTauri()) {
      void call<RuntimeSnapshot>('get_runtime_snapshot')
        .then((snapshot) => {
          if (snapshot.selectorBounds) {
            setOrigin({
              x: snapshot.selectorBounds.x,
              y: snapshot.selectorBounds.y,
            })
            writeLocalDebug('selector-ui', 'loaded selector bounds from runtime', {
              selectorBounds: snapshot.selectorBounds,
            })
          } else {
            writeLocalDebug('selector-ui', 'runtime snapshot missing selector bounds', {
              snapshot,
            })
          }
        })
        .catch((error) => {
          writeLocalDebug('selector-ui', 'failed to load runtime snapshot', {
            error: formatUnknown(error),
          })
        })

      void watchEvent<RuntimeSnapshot>('runtime-snapshot', (snapshot) => {
        if (!snapshot.selectorBounds) {
          return
        }

        setOrigin({
          x: snapshot.selectorBounds.x,
          y: snapshot.selectorBounds.y,
        })
        writeLocalDebug('selector-ui', 'runtime snapshot updated selector bounds', {
          selectorBounds: snapshot.selectorBounds,
        })
      }).then((unlisten) => {
        detachSnapshot = unlisten
      })
    }

    writeLocalDebug('selector-ui', 'mounted', {
      origin,
      hash: window.location.hash,
      search: window.location.search,
      viewport: {
        width: window.innerWidth,
        height: window.innerHeight,
      },
      tauri: isTauri(),
      currentWindowLabel: isTauri() ? getCurrentWindow().label : null,
    })
    if (selectorWindow) {
      void selectorWindow.setFocus().catch(() => {
        window.focus()
      })
    } else {
      window.focus()
    }

    const onFocus = () => {
      writeLocalDebug('selector-ui', 'window focus', {
        hasFocus: document.hasFocus(),
        visibilityState: document.visibilityState,
      })
    }

    const onBlur = () => {
      writeLocalDebug('selector-ui', 'window blur', {
        hasFocus: document.hasFocus(),
        visibilityState: document.visibilityState,
      })
    }

    const onVisibilityChange = () => {
      writeLocalDebug('selector-ui', 'visibility change', {
        visibilityState: document.visibilityState,
      })
    }

    const onError = (event: ErrorEvent) => {
      writeLocalDebug('selector-ui', 'window error', {
        message: event.message,
        filename: event.filename,
        line: event.lineno,
        column: event.colno,
      })
    }

    const onUnhandledRejection = (event: PromiseRejectionEvent) => {
      writeLocalDebug('selector-ui', 'unhandled rejection', {
        reason: formatUnknown(event.reason),
      })
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        writeLocalDebug('selector-ui', 'escape pressed', {
          selectionActive: Boolean(anchorRef.current),
        })
        void requestSelectorCancel('escape')
      }
    }

    window.addEventListener('focus', onFocus)
    window.addEventListener('blur', onBlur)
    window.addEventListener('visibilitychange', onVisibilityChange)
    window.addEventListener('error', onError)
    window.addEventListener('unhandledrejection', onUnhandledRejection)
    window.addEventListener('keydown', onKeyDown, true)
    document.addEventListener('keydown', onKeyDown, true)
    onFocus()

    return () => {
      detach()
      detachSnapshot()
      writeLocalDebug('selector-ui', 'unmounted')
      window.removeEventListener('focus', onFocus)
      window.removeEventListener('blur', onBlur)
      window.removeEventListener('visibilitychange', onVisibilityChange)
      window.removeEventListener('error', onError)
      window.removeEventListener('unhandledrejection', onUnhandledRejection)
      window.removeEventListener('keydown', onKeyDown, true)
      document.removeEventListener('keydown', onKeyDown, true)
    }
  }, [origin, requestSelectorCancel, selectorWindow])

  const submitSelection = async (nextSelection: SelectionRect | null) => {
    if (!nextSelection || nextSelection.width < 24 || nextSelection.height < 24) {
      writeLocalDebug('selector-ui', 'selection ignored', {
        selection: nextSelection,
        reason: 'selection too small',
      })
      return
    }

    if (lifecycleBusyRef.current) {
      return
    }

    lifecycleBusyRef.current = true

    const absoluteSelection = {
      x: origin.x + nextSelection.x,
      y: origin.y + nextSelection.y,
      width: nextSelection.width,
      height: nextSelection.height,
    } satisfies SelectionRect

    writeLocalDebug('selector-ui', 'submitting selection', {
      localSelection: nextSelection,
      absoluteSelection,
    })

    try {
      await call('submit_selection', {
        selection: absoluteSelection,
      })
      writeLocalDebug('selector-ui', 'submit selection completed', {
        absoluteSelection,
      })
    } catch (error) {
      lifecycleBusyRef.current = false
      writeLocalDebug('selector-ui', 'submit selection failed', {
        absoluteSelection,
        error: formatUnknown(error),
      })
    }
  }

  return (
    <div
      className="selector-page selector-ready"
      onPointerDown={(event) => {
        if (event.button !== 0) {
          writeLocalDebug('selector-ui', 'pointerdown ignored', {
            button: event.button,
            pointerId: event.pointerId,
            target: describeTarget(event.target),
          })
          return
        }
        event.preventDefault()
        const point = { x: event.clientX, y: event.clientY }
        dragMoveLoggedRef.current = false
        anchorRef.current = point
        setAnchor(point)
        setCurrent(point)
        if (selectorWindow) {
          void selectorWindow.setFocus().catch(() => {
            window.focus()
          })
        }
        event.currentTarget.setPointerCapture(event.pointerId)
        writeLocalDebug('selector-ui', 'pointerdown accepted', {
          point,
          pointerId: event.pointerId,
          buttons: event.buttons,
          target: describeTarget(event.target),
        })
      }}
      onPointerMove={(event) => {
        if (!anchorRef.current) {
          return
        }
        if (!dragMoveLoggedRef.current) {
          dragMoveLoggedRef.current = true
          writeLocalDebug('selector-ui', 'pointermove during drag', {
            from: anchorRef.current,
            to: { x: event.clientX, y: event.clientY },
            pointerId: event.pointerId,
          })
        }
        setCurrent({ x: event.clientX, y: event.clientY })
      }}
      onPointerUp={(event) => {
        if (!anchorRef.current) {
          return
        }

        const nextSelection = normalizeSelection(anchorRef.current, {
          x: event.clientX,
          y: event.clientY,
        })

        const hadCapture = event.currentTarget.hasPointerCapture(event.pointerId)
        writeLocalDebug('selector-ui', 'pointerup', {
          pointerId: event.pointerId,
          point: { x: event.clientX, y: event.clientY },
          hadCapture,
          nextSelection,
        })

        if (hadCapture) {
          event.currentTarget.releasePointerCapture(event.pointerId)
        }

        dragMoveLoggedRef.current = false
        anchorRef.current = null
        setAnchor(null)
        setCurrent(null)
        void submitSelection(nextSelection)
      }}
      onPointerCancel={(event) => {
        const hadCapture = event.currentTarget.hasPointerCapture(event.pointerId)
        writeLocalDebug('selector-ui', 'pointercancel', {
          pointerId: event.pointerId,
          hadCapture,
        })
        if (hadCapture) {
          event.currentTarget.releasePointerCapture(event.pointerId)
        }
        dragMoveLoggedRef.current = false
        anchorRef.current = null
        setAnchor(null)
        setCurrent(null)
      }}
    >
      <div className="selector-grid"></div>
      <div className="selector-hud">
        <span className="selector-chip selector-chip-live">Drag</span>
        <span className="selector-chip">ESC</span>
      </div>

      <aside className="selector-debug-banner">
        <strong className="selector-debug-title">Selector Debug</strong>
        <span className="selector-debug-copy">
          window={currentWindowLabel} view={injectedView} origin={origin.x},{origin.y}
        </span>
        <span className="selector-debug-copy">{lastDebugLine}</span>
      </aside>

      {selection ? (
        <div
          className="selector-rect"
          style={{
            left: selection.x,
            top: selection.y,
            width: selection.width,
            height: selection.height,
          }}
        >
          <span className="selector-size">
            {selection.width} × {selection.height}
          </span>
        </div>
      ) : null}
    </div>
  )
}

function OverlayView() {
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot>(PREVIEW_SNAPSHOT)
  const [translation, setTranslation] = useState<TranslationPayload>(PREVIEW_TRANSLATION)
  const deferredBlocks = useDeferredValue(translation.blocks)
  const overlayWindow = useMemo(() => (isTauri() ? getCurrentWindow() : null), [])
  const boundsRef = useRef<SelectionRect | null>(PREVIEW_SNAPSHOT.selection)

  const syncSnapshot = useEffectEvent((next: RuntimeSnapshot) => {
    startTransition(() => {
      setSnapshot(next)
    })
  })

  const syncTranslation = useEffectEvent((next: TranslationPayload) => {
    startTransition(() => {
      setTranslation(next)
    })
  })

  useEffect(() => {
    boundsRef.current = snapshot.selection
  }, [snapshot.selection])

  const syncOverlayBounds = useEffectEvent(async () => {
    if (!overlayWindow || !isTauri()) {
      return
    }

    try {
      const [position, size] = await Promise.all([
        overlayWindow.innerPosition(),
        overlayWindow.innerSize(),
      ])

      const nextSelection = {
        x: Math.round(position.x),
        y: Math.round(position.y),
        width: Math.round(size.width),
        height: Math.round(size.height),
      } satisfies SelectionRect

      if (sameSelection(boundsRef.current, nextSelection)) {
        return
      }

      boundsRef.current = nextSelection
      await call('update_overlay_selection', { selection: nextSelection })
    } catch (error) {
      writeLocalDebug('overlay-ui', 'sync overlay bounds failed', {
        error: formatUnknown(error),
      })
    }
  })

  useEffect(() => {
    if (!isTauri()) {
      return
    }

    let stopped = false
    const bootstrap = async () => {
      try {
        const [nextSnapshot, nextTranslation] = await Promise.all([
          call<RuntimeSnapshot>('get_runtime_snapshot'),
          call<TranslationPayload>('get_latest_translation'),
        ])

        if (!stopped) {
          setSnapshot(nextSnapshot)
          setTranslation(nextTranslation)
        }
      } catch {
        // Overlay keeps the preview payload when the backend is not ready yet.
      }
    }

    void bootstrap()

    let detachSnapshot = () => {}
    let detachTranslation = () => {}
    void watchEvent<RuntimeSnapshot>('runtime-snapshot', syncSnapshot).then(
      (unlisten) => {
        detachSnapshot = unlisten
      },
    )
    void watchEvent<TranslationPayload>('translation-update', syncTranslation).then(
      (unlisten) => {
        detachTranslation = unlisten
      },
    )

    return () => {
      stopped = true
      detachSnapshot()
      detachTranslation()
    }
  }, [])

  useEffect(() => {
    if (!overlayWindow || !isTauri()) {
      return
    }

    let detachMoved = () => {}
    let detachResized = () => {}
    let syncTimer: number | null = null

    const queueSync = () => {
      if (syncTimer !== null) {
        window.clearTimeout(syncTimer)
      }

      syncTimer = window.setTimeout(() => {
        void syncOverlayBounds()
      }, 120)
    }

    void overlayWindow.onMoved(() => {
      queueSync()
    }).then((unlisten) => {
      detachMoved = unlisten
    })

    void overlayWindow.onResized(() => {
      queueSync()
    }).then((unlisten) => {
      detachResized = unlisten
    })

    return () => {
      if (syncTimer !== null) {
        window.clearTimeout(syncTimer)
      }
      detachMoved()
      detachResized()
    }
  }, [overlayWindow, syncOverlayBounds])

  const startOverlayDrag = async (event: React.PointerEvent<HTMLElement>) => {
    if (!overlayWindow || event.button !== 0 || snapshot.copyMode) {
      return
    }

    const target = event.target as HTMLElement
    if (shouldIgnoreWindowDrag(target)) {
      return
    }

    try {
      await overlayWindow.startDragging()
    } catch {
      // Ignore drag rejections from edge cases like rapid resize gestures.
    }
  }

  const startOverlayResize = (direction: ResizeDirection) => async (
    event: React.PointerEvent<HTMLButtonElement>,
  ) => {
    if (!overlayWindow || event.button !== 0) {
      return
    }

    event.preventDefault()
    event.stopPropagation()

    try {
      await overlayWindow.startResizeDragging(direction)
    } catch {
      // Ignore resize rejections from unsupported host edge cases.
    }
  }

  return (
    <div
      className={`overlay-page overlay-interactive ${
        snapshot.copyMode ? 'overlay-copy-mode' : ''
      }`}
      onPointerDown={startOverlayDrag}
    >
      {PANEL_RESIZE_HANDLES.map((handle) => (
        <button
          key={handle.direction}
          type="button"
          className={`resize-handle ${handle.className}`}
          aria-label={`Resize ${handle.direction}`}
          data-no-drag="true"
          onPointerDown={startOverlayResize(handle.direction)}
        ></button>
      ))}

      <div className="overlay-frame" aria-hidden="true">
        <span className="overlay-corner overlay-corner-tl"></span>
        <span className="overlay-corner overlay-corner-tr"></span>
        <span className="overlay-corner overlay-corner-bl"></span>
        <span className="overlay-corner overlay-corner-br"></span>
      </div>

      <div className="overlay-meta">
        <span className={`overlay-mode ${snapshot.copyMode ? 'overlay-mode-live' : ''}`}>
          {snapshot.copyMode ? 'COPY' : 'LIVE'}
        </span>
      </div>

      {deferredBlocks.map((block) => (
        <article
          key={block.id}
          className={`overlay-block overlay-block-${block.align}`}
          style={{
            left: block.x,
            top: block.y,
            width: block.width,
            minHeight: block.height,
            color: block.foreground,
            background: withAlpha(block.background, 0.76),
            fontSize: block.fontSize,
          }}
        >
          {block.translatedText}
        </article>
      ))}
    </div>
  )
}

function selectorOrigin() {
  const url = new URL(window.location.href)
  const params = new URLSearchParams(url.search)

  if (!params.has('ox') && !params.has('oy')) {
    const hashQuery = window.location.hash.split('?')[1] ?? ''
    const hashParams = new URLSearchParams(hashQuery)
    return {
      x: Number(hashParams.get('ox') ?? 0),
      y: Number(hashParams.get('oy') ?? 0),
    }
  }

  return {
    x: Number(params.get('ox') ?? 0),
    y: Number(params.get('oy') ?? 0),
  }
}

function readInjectedView() {
  if (typeof window === 'undefined') {
    return null
  }

  const value = (window as Window & {
    __RANGE_TRANSLATOR_VIEW__?: unknown
  }).__RANGE_TRANSLATOR_VIEW__

  return typeof value === 'string' ? value : null
}

function resolveRouteInfo(): RouteInfo {
  const url = new URL(window.location.href)
  const scriptView = readInjectedView()
  const currentWindowLabel = isTauri() ? getCurrentWindow().label : null
  const queryView = url.searchParams.get('view')
  const hashView = extractHashView(url.hash)
  const pathView = extractPathView(url.pathname)

  return {
    view:
      normalizeView(scriptView) ??
      normalizeView(currentWindowLabel) ??
      normalizeView(queryView) ??
      normalizeView(hashView) ??
      normalizeView(pathView) ??
      'panel',
    scriptView,
    currentWindowLabel,
    href: url.href,
    pathname: url.pathname,
    search: url.search,
    hash: url.hash,
    queryView,
    hashView,
    pathView,
  }
}

function normalizeView(value: string | null | undefined): AppView | null {
  if (value === 'selector' || value === 'overlay' || value === 'panel') {
    return value
  }

  return null
}

function extractHashView(hash: string) {
  if (!hash) {
    return null
  }

  const trimmed = hash.replace(/^#\/?/, '')
  const view = trimmed.split(/[/?&]/)[0]
  return view || null
}

function extractPathView(pathname: string) {
  const segments = pathname
    .split('/')
    .map((segment) => segment.trim())
    .filter(Boolean)
    .filter((segment) => segment !== 'index.html')

  return segments.at(-1) ?? null
}

function normalizeSelection(
  start: { x: number; y: number },
  end: { x: number; y: number },
): SelectionRect {
  const left = Math.min(start.x, end.x)
  const top = Math.min(start.y, end.y)
  return {
    x: Math.round(left),
    y: Math.round(top),
    width: Math.round(Math.abs(end.x - start.x)),
    height: Math.round(Math.abs(end.y - start.y)),
  }
}

function sameSelection(
  left: SelectionRect | null | undefined,
  right: SelectionRect | null | undefined,
) {
  if (!left || !right) {
    return false
  }

  return (
    left.x === right.x &&
    left.y === right.y &&
    left.width === right.width &&
    left.height === right.height
  )
}

function shouldIgnoreWindowDrag(target: HTMLElement) {
  return Boolean(target.closest('button, select, option, [data-no-drag="true"]'))
}

function toneForStatus(status: RuntimeStatus) {
  switch (status) {
    case 'capturing':
    case 'recognizing':
    case 'translating':
      return 'live'
    case 'error':
      return 'danger'
    case 'ready':
      return 'ready'
    case 'selecting':
      return 'selecting'
    default:
      return 'idle'
  }
}

function labelForStatus(status: RuntimeStatus) {
  switch (status) {
    case 'capturing':
      return 'Live'
    case 'recognizing':
      return 'OCR'
    case 'translating':
      return 'Wait'
    case 'selecting':
      return 'Pick'
    case 'ready':
      return 'Ready'
    case 'error':
      return 'Error'
    default:
      return 'Idle'
  }
}

function withAlpha(color: string, alpha: number) {
  const normalized = color.replace('#', '')
  if (normalized.length !== 6) {
    return color
  }

  const suffix = Math.round(Math.max(0, Math.min(1, alpha)) * 255)
    .toString(16)
    .padStart(2, '0')
  return `#${normalized}${suffix}`
}

function logDebugPayload(payload: DebugPayload) {
  const title = `[RangeTranslator:${payload.scope}] ${payload.message} @ ${payload.timestamp}`
  if (payload.detail == null) {
    console.info(title)
    return
  }

  console.groupCollapsed(title)
  console.log(payload.detail)
  console.groupEnd()
}

function writeLocalDebug(scope: string, message: string, detail?: unknown) {
  logDebugPayload({
    scope,
    message,
    detail: detail ?? null,
    timestamp: new Date().toISOString(),
  })
}

function formatDebugLine(payload: DebugPayload) {
  const detail = payload.detail == null ? '' : ` ${formatUnknown(payload.detail)}`
  return `${payload.scope}: ${payload.message}${detail}`.trim()
}

function formatUnknown(value: unknown): string {
  if (typeof value === 'string') {
    return value
  }

  if (value instanceof Error) {
    return value.message
  }

  try {
    return JSON.stringify(value)
  } catch {
    return String(value)
  }
}

function describeTarget(target: EventTarget | null) {
  if (!(target instanceof Element)) {
    return null
  }

  return {
    tag: target.tagName.toLowerCase(),
    id: target.id || null,
    className: target.getAttribute('class'),
  }
}

function IconScan() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M5 7.5V5h2.5M19 7.5V5h-2.5M5 16.5V19h2.5M19 16.5V19h-2.5" />
      <path d="M4.5 12h15" />
    </svg>
  )
}

function IconMinimize() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M6 12.5h12" />
    </svg>
  )
}

function IconClose() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M7 7l10 10M17 7 7 17" />
    </svg>
  )
}

function IconWave() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4 12c2 0 2-4 4-4s2 8 4 8 2-8 4-8 2 4 4 4" />
    </svg>
  )
}

function IconTranslate() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4 6h10M9 6c0 6-2 10-5 12M9 6c1 4 3 6 6 8" />
      <path d="M15 10h5M17.5 10v8M14.5 18h6" />
    </svg>
  )
}

function IconCrop() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M8 4v12a2 2 0 0 0 2 2h10" />
      <path d="M4 8h12a2 2 0 0 1 2 2v10" />
    </svg>
  )
}

function IconErase() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="m6 15 6-8 6 8" />
      <path d="M7.5 15h9" />
      <path d="M5 19h14" />
    </svg>
  )
}

function IconPlay() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="m9 7 8 5-8 5Z" />
    </svg>
  )
}

function IconPause() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M9 7v10M15 7v10" />
    </svg>
  )
}

function IconCopy() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M9 9h9v11H9z" />
      <path d="M6 15H5V4h9v2" />
    </svg>
  )
}

export default App
