import {
  startTransition,
  useEffect,
  useEffectEvent,
  useMemo,
  useState,
} from 'react'
import { call, currentTauriWindow, isTauri, watchEvent } from '../bridge'
import { DEBUG_EVENT, PANEL_RESIZE_HANDLES, PREVIEW_SNAPSHOT, type DebugPayload, type ResizeDirection } from '../app/constants'
import { logDebugPayload } from '../app/debug'
import { shouldIgnoreWindowDrag } from '../app/overlay'

import { FiX, FiMousePointer, FiCamera, FiEye, FiMove } from "react-icons/fi"

import type { OverlayInteractionMode, RuntimeSnapshot } from '../types'

export function SettingsView() {
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot>(PREVIEW_SNAPSHOT)
  const [busy, setBusy] = useState(false)
  const settingsWindow = useMemo(() => currentTauriWindow(), [])

  const applySnapshot = useEffectEvent((next: RuntimeSnapshot) => {
    startTransition(() => {
      setSnapshot(next)
    })
  })

  useEffect(() => {
    let cancelled = false
    const bootstrap = async () => {
      if (!isTauri()) return

      try {
        const next = await call<RuntimeSnapshot>('get_runtime_snapshot')
        if (!cancelled) setSnapshot(next)
      } catch {
        // Browser preview
      }
    }

    void bootstrap()

    let detach = () => {}
    void watchEvent<RuntimeSnapshot>('runtime-snapshot', applySnapshot).then(
      (unlisten) => { detach = unlisten },
    )

    return () => {
      cancelled = true
      detach()
    }
  }, [])

  useEffect(() => {
    if (!isTauri()) return

    let detach = () => {}
    void watchEvent<DebugPayload>(DEBUG_EVENT, (payload) => {
      logDebugPayload(payload)
    }).then((unlisten) => { detach = unlisten })

    return () => { detach() }
  }, [])

  const runCommand = async (action: () => Promise<void>) => {
    if (!isTauri()) return

    setBusy(true)
    try {
      await action()
    } finally {
      setBusy(false)
    }
  }

  const startDrag = async (event: React.PointerEvent<HTMLElement>) => {
    if (!settingsWindow || event.button !== 0) return

    const target = event.target as HTMLElement
    if (shouldIgnoreWindowDrag(target)) return

    try {
      await settingsWindow.startDragging()
    } catch {
      // Ignore drag errors
    }
  }

  const startResize = (direction: ResizeDirection) => async (
    event: React.PointerEvent<HTMLButtonElement>,
  ) => {
    if (!settingsWindow || event.button !== 0) return

    event.preventDefault()
    event.stopPropagation()
    try {
      await settingsWindow.startResizeDragging(direction)
    } catch {
      // Ignore resize errors
    }
  }

  return (
    <main className="panel-window settings-window" onPointerDown={startDrag} data-tauri-drag-region>
      {PANEL_RESIZE_HANDLES.map((handle) => (
        <button
          key={handle.direction}
          type="button"
          className={`resize-handle ${handle.className}`}
          aria-label={`Resize ${handle.direction}`}
          onPointerDown={startResize(handle.direction)}
        ></button>
      ))}

      <header className="settings-header">
        <h2 
          data-no-drag="true" 
          style={{ margin: 0, fontSize: '20px', fontWeight: 700, color: 'var(--ink-strong)' }}
        >
          Settings
        </h2>

        <div className="window-controls" data-no-drag="true">
          <button
            type="button"
            className="window-btn danger"
            title="Close"
            onClick={() => {
              if (settingsWindow) {
                settingsWindow.close()
              }
            }}
          >
            <FiX size={14} />
          </button>
        </div>
      </header>

      <section className="settings-content" data-tauri-drag-region>
        <div className="setting-segmented" data-no-drag="true">
          <div className="setting-segmented-header">
            <FiMove size={18} />
            <span className="setting-toggle-label">Overlay Mode</span>
          </div>
          <div className="segmented-control">
            <button
              type="button"
              className={`segment-btn ${snapshot.overlayMode === 'passThrough' ? 'active' : ''}`}
              disabled={busy || !snapshot.selection}
              onClick={() => runCommand(() => call('set_overlay_interaction_mode', { mode: 'passThrough' as OverlayInteractionMode }))}
            >
              <FiEye size={14} /> View
            </button>
            <button
              type="button"
              className={`segment-btn ${snapshot.overlayMode === 'selectText' ? 'active' : ''}`}
              disabled={busy || !snapshot.selection}
              onClick={() => runCommand(() => call('set_overlay_interaction_mode', { mode: 'selectText' as OverlayInteractionMode }))}
            >
              <FiMousePointer size={14} /> Select
            </button>
            <button
              type="button"
              className={`segment-btn ${snapshot.overlayMode === 'dragWindow' ? 'active' : ''}`}
              disabled={busy || !snapshot.selection}
              onClick={() => runCommand(() => call('set_overlay_interaction_mode', { mode: 'dragWindow' as OverlayInteractionMode }))}
            >
              <FiMove size={14} /> Drag
            </button>
          </div>
        </div>

        <div
          className="setting-toggle"
          data-no-drag="true"
          onClick={() => {
            if (busy) return;
            runCommand(() =>
              call('toggle_debug_screenshot_mode', {
                enabled: !snapshot.debugScreenshotMode,
              }),
            )
          }}
        >
          <div className="setting-toggle-info">
            <div className="setting-toggle-icon">
              <FiCamera size={18} />
            </div>
            <span className="setting-toggle-label">Allow Screenshots</span>
          </div>
          <div className={`switch ${snapshot.debugScreenshotMode ? 'active' : ''}`}>
            <div className="switch-thumb" />
          </div>
        </div>
      </section>
    </main>
  )
}
