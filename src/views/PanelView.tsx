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

import { FiX, FiMinus, FiPlay, FiPause, FiCrop, FiMousePointer, FiCamera, FiGlobe, FiType, FiArrowRight } from "react-icons/fi";
import { RiPushpinLine } from "react-icons/ri";

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

    if ((event.ctrlKey || event.metaKey) && event.shiftKey && key === 'd') {
      event.preventDefault()
      void runCommand(() =>
        call('toggle_debug_screenshot_mode', {
          enabled: !snapshot.debugScreenshotMode,
        }),
      )
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 'enter') {
      if (snapshot.debugScreenshotMode) {
        return
      }
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
    <main className="panel-window main-layout" onPointerDown={startDrag}>
      {PANEL_RESIZE_HANDLES.map((handle) => (
        <button
          key={handle.direction}
          type="button"
          className={`resize-handle ${handle.className}`}
          aria-label={`Resize ${handle.direction}`}
          onPointerDown={startResize(handle.direction)}
        ></button>
      ))}

      <header className={`panel-header panel-status-rail-${statusTone}`} data-tauri-drag-region>
        <div className="header-status-area">
          <span className="status-dot"></span>
          <span>{labelForStatus(snapshot.status)}</span>
        </div>

        <div className="panel-drag-spacer" aria-hidden="true"></div>

        <div className="panel-actions" data-no-drag="true">
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
            <RiPushpinLine style={{ width: 14, height: 14 }} />
          </button>

          <span className="panel-actions-divider" aria-hidden="true"></span>

          <button
            type="button"
            className="window-button"
            aria-label="Minimize"
            onClick={() => runCommand(() => call('panel_minimize'))}
          >
            <FiMinus style={{ width: 14, height: 14 }} />
          </button>
          <button
            type="button"
            className="window-button window-button-danger"
            aria-label="Close"
            onClick={() => runCommand(() => call('panel_close'))}
          >
            <FiX style={{ width: 14, height: 14 }} />
          </button>
        </div>
      </header>

      <section className="language-switch-area" data-tauri-drag-region>
        <CompactSelect
          label=""
          icon={<FiType style={{ width: 14, height: 14 }} />}
          value={sourceLanguage}
          disabled={snapshot.running || busy}
          options={SOURCE_LANGUAGES}
          onChange={setSourceLanguage}
          menuSide="bottom"
        />
        <FiArrowRight className="language-arrow" style={{ width: 14, height: 14, flexShrink: 0 }} />
        <CompactSelect
          label=""
          icon={<FiGlobe style={{ width: 14, height: 14 }} />}
          value={targetLanguage}
          disabled={snapshot.running || busy}
          options={TARGET_LANGUAGES}
          onChange={setTargetLanguage}
          menuSide="bottom"
        />
      </section>

      <section className="core-action-area" data-tauri-drag-region>
        <button
          type="button"
          className={`main-action-btn ${snapshot.running ? 'active' : ''}`}
          data-no-drag="true"
          title={snapshot.debugScreenshotMode
            ? 'Disable debug screenshot mode to start live translation'
            : snapshot.running
              ? 'Stop live translation'
              : 'Start live translation'}
          disabled={busy || snapshot.debugScreenshotMode || (!snapshot.running && !snapshot.selection)}
          onClick={() =>
            runCommand(() =>
              call(snapshot.running ? 'stop_pipeline' : 'start_pipeline', {
                settings: { sourceLanguage, targetLanguage },
              }),
            )
          }
        >
          {snapshot.running ? <FiPause size={36} /> : <FiPlay size={36} style={{ marginLeft: 4 }} />}
        </button>

        <div className="region-setup-area" data-no-drag="true">
          {!snapshot.selection ? (
             <button
              type="button"
              className="crop-main-btn"
              title="Select screen region"
              disabled={busy}
              onClick={() => runCommand(() => call('open_selector_window'))}
            >
              <FiCrop size={16} /> Set Region
            </button>
          ) : (
            <div className="region-chip">
              <FiCrop size={12} style={{ opacity: 0.5, marginTop: '2px' }} />
              <span>{selectionLabel}</span>
              <button
                className="region-chip-clear"
                title="Clear current region"
                onClick={() => runCommand(() => call('clear_selection'))}
                disabled={busy}
              >
                <FiX size={14} />
              </button>
            </div>
          )}
        </div>
      </section>

      <footer className="advanced-footer" data-tauri-drag-region>
        <div data-no-drag="true" style={{ display: 'flex', gap: '8px' }}>
          <button
            type="button"
            className={`ghost-icon-btn ${snapshot.copyMode ? 'active' : ''}`}
            disabled={busy || !snapshot.selection}
            title={snapshot.copyMode ? 'Enable mouse passthrough' : 'Disable mouse passthrough'}
            onClick={() =>
              runCommand(() =>
                call('toggle_copy_mode', { enabled: !snapshot.copyMode }),
              )
            }
          >
            <FiMousePointer size={18} />
          </button>
          <button
            type="button"
            className={`ghost-icon-btn ${snapshot.debugScreenshotMode ? 'active' : ''}`}
            disabled={busy}
            title={snapshot.debugScreenshotMode
              ? 'Disable debug screenshot mode (Translation Pipeline Paused)'
              : 'Enable debug screenshot mode'}
            onClick={() =>
              runCommand(() =>
                call('toggle_debug_screenshot_mode', {
                  enabled: !snapshot.debugScreenshotMode,
                }),
              )
            }
          >
            <FiCamera size={18} />
          </button>
        </div>
      </footer>

      {snapshot.lastError ? (
        <div className="panel-error">{snapshot.lastError}</div>
      ) : null}
    </main>
  )
}
