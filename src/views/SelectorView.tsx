import {
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
} from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { call, isTauri, watchEvent } from '../bridge'
import { DEBUG_EVENT, type DebugPayload } from '../app/constants'
import {
  describeTarget,
  formatDebugLine,
  formatUnknown,
  logDebugPayload,
  writeLocalDebug,
} from '../app/debug'
import { normalizeSelection } from '../app/overlay'
import { readInjectedView, selectorOrigin } from '../app/routing'
import type { RuntimeSnapshot, SelectionRect } from '../types'

export function SelectorView() {
  const [selectorBounds, setSelectorBounds] = useState<SelectionRect>(() => {
    const origin = selectorOrigin()
    return {
      x: origin.x,
      y: origin.y,
      width: Math.max(window.innerWidth, 1),
      height: Math.max(window.innerHeight, 1),
    }
  })
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
            setSelectorBounds(snapshot.selectorBounds)
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

        setSelectorBounds(snapshot.selectorBounds)
        writeLocalDebug('selector-ui', 'runtime snapshot updated selector bounds', {
          selectorBounds: snapshot.selectorBounds,
        })
      }).then((unlisten) => {
        detachSnapshot = unlisten
      })
    }

    writeLocalDebug('selector-ui', 'mounted', {
      selectorBounds,
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
  }, [requestSelectorCancel, selectorBounds, selectorWindow])

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

    const viewportWidth = Math.max(window.innerWidth, 1)
    const viewportHeight = Math.max(window.innerHeight, 1)
    const scaleX = selectorBounds.width / viewportWidth
    const scaleY = selectorBounds.height / viewportHeight

    const absoluteSelection = {
      x: selectorBounds.x + Math.round(nextSelection.x * scaleX),
      y: selectorBounds.y + Math.round(nextSelection.y * scaleY),
      width: Math.round(nextSelection.width * scaleX),
      height: Math.round(nextSelection.height * scaleY),
    } satisfies SelectionRect

    writeLocalDebug('selector-ui', 'submitting selection', {
      localSelection: nextSelection,
      absoluteSelection,
      scaleX,
      scaleY,
      selectorBounds,
      viewportWidth,
      viewportHeight,
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
          window={currentWindowLabel} view={injectedView} origin={selectorBounds.x},
          {selectorBounds.y}
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
