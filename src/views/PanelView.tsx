import {
  startTransition,
  useEffect,
  useEffectEvent,
  useMemo,
  useState,
} from 'react'
import { call, currentTauriWindow, isTauri, watchEvent } from '../bridge'
import { Tooltip } from '../components/Tooltip'
import { DEBUG_EVENT, PANEL_RESIZE_HANDLES, PREVIEW_SNAPSHOT, type DebugPayload, type ResizeDirection } from '../app/constants'
import { logDebugPayload } from '../app/debug'
import { labelForStatus, shouldIgnoreWindowDrag, toneForStatus } from '../app/overlay'

import { FiX, FiMinus, FiPlay, FiPause, FiCrop, FiSettings } from "react-icons/fi";
import { RiPushpinLine, RiPushpinFill } from "react-icons/ri";

import type { RuntimeSnapshot } from '../types'
import { useNotification } from '../components/NotificationProvider'
import { motion, AnimatePresence } from 'framer-motion';

import './PanelView.css';

export function PanelView() {
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot>(PREVIEW_SNAPSHOT)
  const [busy, setBusy] = useState(false)
  const panelWindow = useMemo(() => currentTauriWindow(), [])
  const { showNotification } = useNotification()
  const [isHoveredSelect, setIsHoveredSelect] = useState(false);

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

  const hasSelection = !!snapshot.selection;
  const statusTone = toneForStatus(snapshot.status)

  const runCommand = async (action: () => Promise<void>) => {
    if (!isTauri()) {
      return
    }

    setBusy(true)
    try {
      await action()
    } catch (error) {
      showNotification({
        type: 'error',
        message: error instanceof Error ? error.message : String(error)
      })
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
          settings: { sourceLanguage: snapshot.sourceLanguage, targetLanguage: snapshot.targetLanguage },
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
    <main className="panel-v2-window" onPointerDown={startDrag} data-tauri-drag-region>
      {PANEL_RESIZE_HANDLES.map((handle) => (
        <button
          key={handle.direction}
          type="button"
          className={`resize-handle ${handle.className}`}
          aria-label={`Resize ${handle.direction}`}
          onPointerDown={startResize(handle.direction)}
        ></button>
      ))}

      {/* Elegant Header with Glassmorphism */}
      <header className="panel-v2-header">
        <div className="v2-status-indicator" data-no-drag="true">
          <div className={`v2-status-dot ${statusTone === 'danger' ? 'danger' : ''}`}></div>
          <span className="v2-status-text">{labelForStatus(snapshot.status)}</span>
        </div>

        <div className="v2-window-controls" data-no-drag="true">
          <Tooltip content="Settings" position="bottom">
            <button
              type="button"
              className="v2-window-btn"
              onClick={() => runCommand(() => call('open_settings_window'))}
            >
              <FiSettings size={14} />
            </button>
          </Tooltip>
          <Tooltip content={snapshot.panelPinned ? "Unpin Window" : "Pin Window"} position="bottom">
            <button
              type="button"
              className={`v2-window-btn ${snapshot.panelPinned ? 'active' : ''}`}
              disabled={busy}
              onClick={() =>
                runCommand(() =>
                  call('toggle_panel_pin', { enabled: !snapshot.panelPinned }),
                )
              }
            >
              {snapshot.panelPinned ? (
                <RiPushpinFill size={14} style={{ strokeWidth: 0, fill: 'currentColor' }} />
              ) : (
                <RiPushpinLine size={14} style={{ strokeWidth: 0, fill: 'currentColor' }} />
              )}
            </button>
          </Tooltip>
          <Tooltip content="Minimize" position="bottom">
            <button
              type="button"
              className="v2-window-btn"
              onClick={() => runCommand(() => call('panel_minimize'))}
            >
              <FiMinus size={14} />
            </button>
          </Tooltip>
          <Tooltip content="Close" position="bottom">
            <button
              type="button"
              className="v2-window-btn danger"
              onClick={() => runCommand(() => call('panel_close'))}
            >
              <FiX size={14} />
            </button>
          </Tooltip>
        </div>
      </header>

      {/* Center Stage with Framer Motion */}
      <section className="v2-center-stage" data-tauri-drag-region>
        <AnimatePresence mode="wait">
          {!hasSelection ? (
            <motion.div
              key="select-mode"
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.9 }}
              transition={{ duration: 0.3 }}
              className="v2-select-mode"
            >
              {/* Elegant pulsing aura for select action */}
              <div 
                className="v2-select-portal"
                onMouseEnter={() => setIsHoveredSelect(true)}
                onMouseLeave={() => setIsHoveredSelect(false)}
                data-no-drag="true"
              >
                {/* Rotating gradient background */}
                <motion.div 
                  animate={{ rotate: 360, scale: isHoveredSelect ? 1.1 : 1, opacity: isHoveredSelect ? 0.7 : 0.3 }}
                  transition={{ rotate: { duration: 15, repeat: Infinity, ease: "linear" }, scale: { duration: 0.5 }, opacity: { duration: 0.5 } }}
                  className="v2-aura-layer-1"
                />
                <motion.div 
                  animate={{ rotate: -360, scale: isHoveredSelect ? 1.05 : 0.9, opacity: isHoveredSelect ? 0.5 : 0.2 }}
                  transition={{ rotate: { duration: 20, repeat: Infinity, ease: "linear" }, scale: { duration: 0.5 }, opacity: { duration: 0.5 } }}
                  className="v2-aura-layer-2"
                />
                
                {/* Main Button */}
                <motion.button
                  className="v2-select-btn"
                  onClick={() => runCommand(() => call('open_selector_window'))}
                  animate={{
                    scale: isHoveredSelect ? 1.05 : 1,
                    boxShadow: isHoveredSelect 
                      ? '0 10px 30px rgba(59, 130, 246, 0.3), inset 0 0 0 1px rgba(255, 255, 255, 0.4)' 
                      : '0 8px 24px rgba(0, 0, 0, 0.05), inset 0 0 0 1px rgba(255, 255, 255, 0.2)'
                  }}
                  whileTap={{ scale: 0.95 }}
                >
                  <FiCrop size={32} className="v2-select-icon" />
                </motion.button>
              </div>
            </motion.div>
          ) : (
            <motion.div
              key="play-mode"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              transition={{ duration: 0.4, type: "spring", stiffness: 300, damping: 25 }}
              className="v2-play-mode"
            >
              {/* Central Play/Pause Action */}
              <motion.button
                type="button"
                className={`v2-play-action-btn ${snapshot.running ? 'running' : ''}`}
                data-no-drag="true"
                disabled={busy}
                onClick={() =>
                  runCommand(() =>
                    call(snapshot.running ? 'stop_pipeline' : 'start_pipeline', {
                      settings: { sourceLanguage: snapshot.sourceLanguage, targetLanguage: snapshot.targetLanguage },
                    }),
                  )
                }
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
              >
                {snapshot.running ? <FiPause size={36} /> : <FiPlay size={36} />}
              </motion.button>

              {/* Selection Pill */}
              <motion.div 
                className="v2-selection-pill"
                data-no-drag="true"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2, duration: 0.3 }}
              >
                <Tooltip content="Reselect Region" position="top">
                  <button 
                    type="button"
                    className="v2-pill-reselect" 
                    onClick={() => runCommand(() => call('open_selector_window'))}
                  >
                    <FiCrop size={14} className="v2-pill-icon" />
                    <span className="v2-pill-text">{snapshot.selection?.width} x {snapshot.selection?.height}</span>
                  </button>
                </Tooltip>
                
                <div className="v2-pill-divider" />
                
                <Tooltip content="Clear Selection" position="top">
                  <button 
                    type="button"
                    className="v2-pill-clear" 
                    onClick={() => runCommand(() => call('clear_selection'))}
                  >
                    <FiX size={14} />
                  </button>
                </Tooltip>
              </motion.div>
            </motion.div>
          )}
        </AnimatePresence>
      </section>
    </main>
  )
}
