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

const OVERLAY_MODE_ORDER: OverlayInteractionMode[] = ['passThrough', 'selectText', 'dragWindow']

function nextOverlayMode(mode: OverlayInteractionMode): OverlayInteractionMode {
  const currentIndex = OVERLAY_MODE_ORDER.indexOf(mode)
  return OVERLAY_MODE_ORDER[(currentIndex + 1 + OVERLAY_MODE_ORDER.length) % OVERLAY_MODE_ORDER.length]
}

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
      if (!isTauri()) {
        return
      }

      try {
        const next = await call<RuntimeSnapshot>('get_runtime_snapshot')
        if (!cancelled) {
          setSnapshot(next)
        }
      } catch {
        // Browser preview
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
    if (!settingsWindow || event.button !== 0) {
      return
    }

    const target = event.target as HTMLElement
    if (shouldIgnoreWindowDrag(target)) {
      return
    }

    try {
      await settingsWindow.startDragging()
    } catch {
      // Ignore drag errors
    }
  }

  const startResize = (direction: ResizeDirection) => async (
    event: React.PointerEvent<HTMLButtonElement>,
  ) => {
    if (!settingsWindow || event.button !== 0) {
      return
    }

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

      <header className="panel-header">
        <div className="status-indicator" data-no-drag="true">
          <span style={{ fontWeight: 'bold' }}>Settings</span>
        </div>

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
        <div className="setting-group" data-no-drag="true">
          <h3>Overlay Mode</h3>
          <p>Controls how you interact with the overlay window.</p>
          <button
            type="button"
            className={`setting-btn ${snapshot.overlayMode !== 'passThrough' ? 'active' : ''}`}
            disabled={busy || !snapshot.selection}
            onClick={() =>
              runCommand(() =>
                call('set_overlay_interaction_mode', {
                  mode: nextOverlayMode(snapshot.overlayMode),
                }),
              )
            }
          >
            {snapshot.overlayMode === 'passThrough' && <><FiEye /> <span>View Only</span></>}
            {snapshot.overlayMode === 'selectText' && <><FiMousePointer /> <span>Select Text</span></>}
            {snapshot.overlayMode === 'dragWindow' && <><FiMove /> <span>Drag Window</span></>}
          </button>
        </div>

        <div className="setting-group" data-no-drag="true">
          <h3>Debug Screenshot Mode</h3>
          <p>Allows screenshots for debugging. Content is usually protected.</p>
          <button
            type="button"
            className={`setting-btn ${snapshot.debugScreenshotMode ? 'active' : ''}`}
            disabled={busy}
            onClick={() =>
              runCommand(() =>
                call('toggle_debug_screenshot_mode', {
                  enabled: !snapshot.debugScreenshotMode,
                }),
              )
            }
          >
            <FiCamera />
            <span>{snapshot.debugScreenshotMode ? 'Enabled' : 'Disabled'}</span>
          </button>
        </div>
      </section>
    </main>
  )
}
