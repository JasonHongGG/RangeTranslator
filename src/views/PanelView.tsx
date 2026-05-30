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

import { FiX, FiMinus, FiPlay, FiPause, FiCrop, FiMousePointer, FiCamera, FiGlobe, FiArrowRight, FiEye, FiMove } from "react-icons/fi";
import { RiPushpinLine, RiPushpinFill } from "react-icons/ri";

import type { OverlayInteractionMode, RuntimeSnapshot } from '../types'

const OVERLAY_MODE_ORDER: OverlayInteractionMode[] = ['passThrough', 'selectText', 'dragWindow']

function nextOverlayMode(mode: OverlayInteractionMode): OverlayInteractionMode {
  const currentIndex = OVERLAY_MODE_ORDER.indexOf(mode)
  return OVERLAY_MODE_ORDER[(currentIndex + 1 + OVERLAY_MODE_ORDER.length) % OVERLAY_MODE_ORDER.length]
}

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
    : 'Set Region'
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
        call('set_overlay_interaction_mode', {
          mode: nextOverlayMode(snapshot.overlayMode),
        }),
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
    <main className="panel-window" onPointerDown={startDrag} data-tauri-drag-region>
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
        <div className="status-indicator" data-no-drag="true">
          <div className={`status-dot ${statusTone === 'danger' ? 'danger' : ''}`}></div>
          <span>{labelForStatus(snapshot.status)}</span>
        </div>

        <div className="window-controls" data-no-drag="true">
          <button
            type="button"
            className={`window-btn ${snapshot.panelPinned ? 'active' : ''}`}
            disabled={busy}
            title="Pin Window"
            onClick={() =>
              runCommand(() =>
                call('toggle_panel_pin', { enabled: !snapshot.panelPinned }),
              )
            }
          >
            {snapshot.panelPinned ? <RiPushpinFill size={14} /> : <RiPushpinLine size={14} />}
          </button>
          <button
            type="button"
            className="window-btn"
            title="Minimize"
            onClick={() => runCommand(() => call('panel_minimize'))}
          >
            <FiMinus size={14} />
          </button>
          <button
            type="button"
            className="window-btn danger"
            title="Close"
            onClick={() => runCommand(() => call('panel_close'))}
          >
            <FiX size={14} />
          </button>
        </div>
      </header>

      <section className="center-stage" data-tauri-drag-region>
        <div className="lang-capsule" data-no-drag="true">
          <CompactSelect
            value={sourceLanguage}
            disabled={snapshot.running || busy}
            options={SOURCE_LANGUAGES}
            onChange={setSourceLanguage}
            menuSide="bottom"
          />
          <FiArrowRight className="lang-arrow" />
          <CompactSelect
            value={targetLanguage}
            disabled={snapshot.running || busy}
            options={TARGET_LANGUAGES}
            onChange={setTargetLanguage}
            menuSide="bottom"
          />
        </div>

        <div className="core-action-group">
          <button
            type="button"
            className={`play-btn ${snapshot.running ? 'running' : ''}`}
            data-no-drag="true"
            disabled={busy || (!snapshot.running && !snapshot.selection)}
            onClick={() =>
              runCommand(() =>
                call(snapshot.running ? 'stop_pipeline' : 'start_pipeline', {
                  settings: { sourceLanguage, targetLanguage },
                }),
              )
            }
          >
            {snapshot.running ? <FiPause /> : <FiPlay />}
          </button>

          <button
            type="button"
            className={`region-pill ${snapshot.selection ? 'active' : ''}`}
            data-no-drag="true"
            disabled={busy}
            onClick={() => {
              if (snapshot.selection) {
                runCommand(() => call('clear_selection'))
              } else {
                runCommand(() => call('open_selector_window'))
              }
            }}
          >
            {snapshot.selection ? <FiX size={14} /> : <FiCrop size={14} />}
            <span>{selectionLabel}</span>
          </button>
        </div>
      </section>

      <footer className="dock-container">
        <div className="mac-dock" data-no-drag="true">
          <button
            type="button"
            className={`dock-icon ${snapshot.aiTranslationEnabled ? 'active' : ''}`}
            disabled={busy || !snapshot.selection}
            title="AI Translation"
            onClick={() =>
              runCommand(() =>
                call('toggle_ai_translation', { enabled: !snapshot.aiTranslationEnabled }),
              )
            }
          >
            <FiGlobe />
          </button>
          <button
            type="button"
            className={`dock-icon ${snapshot.overlayMode !== 'passThrough' ? 'active' : ''}`}
            disabled={busy || !snapshot.selection}
            title={snapshot.overlayMode === 'passThrough' ? 'Overlay Mode: View Only' : snapshot.overlayMode === 'selectText' ? 'Overlay Mode: Select Text' : 'Overlay Mode: Drag Window'}
            onClick={() =>
              runCommand(() =>
                call('set_overlay_interaction_mode', {
                  mode: nextOverlayMode(snapshot.overlayMode),
                }),
              )
            }
          >
            {snapshot.overlayMode === 'passThrough' && <FiEye />}
            {snapshot.overlayMode === 'selectText' && <FiMousePointer />}
            {snapshot.overlayMode === 'dragWindow' && <FiMove />}
          </button>
          <button
            type="button"
            className={`dock-icon ${snapshot.debugScreenshotMode ? 'active' : ''}`}
            disabled={busy}
            title="Debug Screenshot Mode"
            onClick={() =>
              runCommand(() =>
                call('toggle_debug_screenshot_mode', {
                  enabled: !snapshot.debugScreenshotMode,
                }),
              )
            }
          >
            <FiCamera />
          </button>
        </div>
      </footer>
    </main>
  )
}
