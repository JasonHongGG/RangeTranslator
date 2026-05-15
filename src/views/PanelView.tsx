import {
  startTransition,
  useEffect,
  useEffectEvent,
  useMemo,
  useState,
} from 'react'
import { call, currentTauriWindow, isTauri, watchEvent } from '../bridge'
import { SOURCE_LANGUAGES, TARGET_LANGUAGES } from '../languages'
import { CompactSelect } from '../components/CompactSelect'
import { DEBUG_EVENT, PANEL_RESIZE_HANDLES, PREVIEW_SNAPSHOT, type DebugPayload, type ResizeDirection } from '../app/constants'
import { logDebugPayload } from '../app/debug'
import { labelForStatus, shouldIgnoreWindowDrag, toneForStatus } from '../app/overlay'
import {
  IconClose,
  IconCrop,
  IconErase,
  IconMinimize,
  IconPause,
  IconPin,
  IconPlay,
  IconPointer,
  IconScan,
  IconTranslate,
  IconWave,
} from '../ui/icons'
import type { RuntimeSnapshot } from '../types'

export function PanelView() {
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot>(PREVIEW_SNAPSHOT)
  const [sourceLanguage, setSourceLanguage] = useState('auto')
  const [targetLanguage, setTargetLanguage] = useState('zh-TW')
  const [busy, setBusy] = useState(false)
  const panelWindow = useMemo(() => currentTauriWindow(), [])

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
  const statusDetail = snapshot.lastError ?? snapshot.statusDetail

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
      if (!snapshot.selection) {
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

          <button
            type="button"
            className={`panel-icon-button ${snapshot.panelPinned ? 'panel-icon-button-active' : ''}`}
            title={snapshot.panelPinned ? 'Disable always on top' : 'Keep panel on top'}
            aria-label={snapshot.panelPinned ? 'Disable always on top' : 'Keep panel on top'}
            aria-pressed={snapshot.panelPinned}
            disabled={busy}
            onClick={() =>
              runCommand(() =>
                call('toggle_panel_pin', { enabled: !snapshot.panelPinned }),
              )
            }
          >
            <IconPin />
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
          <span className="panel-status-divider" aria-hidden="true"></span>
          <span className="panel-status-detail">{statusDetail}</span>
        </div>

        <div className="footer-tools" data-no-drag="true">
          <button
            type="button"
            className={`footer-icon ${snapshot.copyMode ? 'footer-icon-active' : ''}`}
            disabled={busy || !snapshot.selection}
            title={snapshot.copyMode ? 'Enable mouse passthrough' : 'Disable mouse passthrough'}
            aria-label={
              snapshot.copyMode
                ? 'Enable mouse passthrough'
                : 'Disable mouse passthrough and allow overlay editing'
            }
            aria-pressed={snapshot.copyMode}
            onClick={() =>
              runCommand(() =>
                call('toggle_copy_mode', { enabled: !snapshot.copyMode }),
              )
            }
          >
            <IconPointer />
          </button>
          <button
            type="button"
            className="footer-icon"
            disabled={busy || !snapshot.selection}
            title="Clear current region"
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
