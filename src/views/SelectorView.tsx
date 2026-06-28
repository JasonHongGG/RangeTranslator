import {
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
} from 'react'
import { call, currentTauriWindow, currentTauriWindowLabel, isTauri, watchEvent } from '../bridge'
import { DEBUG_EVENT, type DebugPayload } from '../app/constants'
import {
  describeTarget,
  formatUnknown,
  logDebugPayload,
  writeLocalDebug,
} from '../app/debug'
import { normalizeSelection } from '../app/overlay'
import { selectorOrigin } from '../app/routing'
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
  const [pointerPos, setPointerPos] = useState({ x: 0, y: 0 })
  const [showMagnifier, setShowMagnifier] = useState(false)
  const [zoom, setZoom] = useState(3)
  const [magnifierImage, setMagnifierImage] = useState<string | null>(null)

  const anchorRef = useRef<{ x: number; y: number } | null>(null)
  const dragMoveLoggedRef = useRef(false)
  const lifecycleBusyRef = useRef(false)
  const isFetchingMagRef = useRef(false)
  const selectorWindow = useMemo(() => currentTauriWindow(), [])


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
      lifecycleBusyRef.current = false
      setAnchor(null)
      setCurrent(null)
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
      currentWindowLabel: currentTauriWindowLabel(),
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
      if (document.visibilityState === 'visible') {
        lifecycleBusyRef.current = false
        setAnchor(null)
        setCurrent(null)
      }
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
  }, [selectorBounds, selectorWindow])

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
      lifecycleBusyRef.current = false
      setAnchor(null)
      setCurrent(null)
    } catch (error) {
      lifecycleBusyRef.current = false
      writeLocalDebug('selector-ui', 'submit selection failed', {
        absoluteSelection,
        error: formatUnknown(error),
      })
    }
  }

  const isSelecting = Boolean(anchor)

  const MAGNIFIER_WIDTH = 160
  const MAGNIFIER_HEIGHT = 160
  const OFFSET = 24

  let magX = pointerPos.x + OFFSET
  let magY = pointerPos.y + OFFSET

  if (magX + MAGNIFIER_WIDTH > window.innerWidth) {
    magX = pointerPos.x - MAGNIFIER_WIDTH - OFFSET
  }
  if (magY + MAGNIFIER_HEIGHT > window.innerHeight) {
    magY = pointerPos.y - MAGNIFIER_HEIGHT - OFFSET
  }
  if (magX < 0) magX = 0
  if (magY < 0) magY = 0

  return (
    <div
      className="selector-page selector-ready"
      style={{ backgroundColor: 'transparent' }}
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
        event.currentTarget.setPointerCapture(event.pointerId)
        writeLocalDebug('selector-ui', 'pointerdown accepted', {
          point,
          pointerId: event.pointerId,
          buttons: event.buttons,
          target: describeTarget(event.target),
        })
      }}
      onPointerMove={(event) => {
        const x = event.clientX
        const y = event.clientY
        setPointerPos({ x, y })
        setShowMagnifier(true)

        if (!isFetchingMagRef.current) {
          isFetchingMagRef.current = true
          const dpr = window.devicePixelRatio || 1
          const size = Math.ceil((MAGNIFIER_WIDTH / zoom) * dpr)
          
          const viewportWidth = Math.max(window.innerWidth, 1)
          const viewportHeight = Math.max(window.innerHeight, 1)
          const scaleX = selectorBounds.width / viewportWidth
          const scaleY = selectorBounds.height / viewportHeight
          const physX = selectorBounds.x + Math.round(x * scaleX)
          const physY = selectorBounds.y + Math.round(y * scaleY)
          
          call<string>('get_magnifier_region', { x: physX, y: physY, size }).then(dataUrl => {
            setMagnifierImage(dataUrl)
            isFetchingMagRef.current = false
          }).catch((err) => {
            console.error(err)
            isFetchingMagRef.current = false
          })
        }

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
      onPointerLeave={() => {
        setShowMagnifier(false)
      }}
      onWheel={(e) => {
        let newZoom = zoom
        if (e.deltaY < 0) {
          newZoom = Math.min(zoom + 1, 20)
        } else {
          newZoom = Math.max(zoom - 1, 2)
        }
        setZoom(newZoom)
        
        if (!isFetchingMagRef.current) {
           isFetchingMagRef.current = true
           const dpr = window.devicePixelRatio || 1
           const size = Math.ceil((MAGNIFIER_WIDTH / newZoom) * dpr)
           
           const viewportWidth = Math.max(window.innerWidth, 1)
           const viewportHeight = Math.max(window.innerHeight, 1)
           const scaleX = selectorBounds.width / viewportWidth
           const scaleY = selectorBounds.height / viewportHeight
           const physX = selectorBounds.x + Math.round(pointerPos.x * scaleX)
           const physY = selectorBounds.y + Math.round(pointerPos.y * scaleY)
           
           call<string>('get_magnifier_region', { x: physX, y: physY, size }).then(dataUrl => {
              setMagnifierImage(dataUrl)
              isFetchingMagRef.current = false
           }).catch(() => {
              isFetchingMagRef.current = false
           })
        }
      }}
    >
      <div 
        style={{
          position: 'absolute',
          inset: 0,
          backgroundColor: 'rgba(0, 0, 0, 0.3)',
          pointerEvents: 'none',
          opacity: isSelecting ? 0 : 1
        }} 
      />



      {selection ? (
        <div
          style={{
            position: 'absolute',
            border: '1px solid rgba(255, 255, 255, 0.3)',
            boxShadow: '0 0 0 9999px rgba(0, 0, 0, 0.3)',
            left: selection.x,
            top: selection.y,
            width: selection.width,
            height: selection.height,
            zIndex: 2,
          }}
        >
          <div style={{ position: 'absolute', top: '-1px', left: '-1px', width: '12px', height: '12px', borderTop: '2px solid #60a5fa', borderLeft: '2px solid #60a5fa' }} />
          <div style={{ position: 'absolute', top: '-1px', right: '-1px', width: '12px', height: '12px', borderTop: '2px solid #60a5fa', borderRight: '2px solid #60a5fa' }} />
          <div style={{ position: 'absolute', bottom: '-1px', left: '-1px', width: '12px', height: '12px', borderBottom: '2px solid #60a5fa', borderLeft: '2px solid #60a5fa' }} />
          <div style={{ position: 'absolute', bottom: '-1px', right: '-1px', width: '12px', height: '12px', borderBottom: '2px solid #60a5fa', borderRight: '2px solid #60a5fa' }} />

          <div style={{
            position: 'absolute',
            top: '-28px',
            left: '0',
            backgroundColor: 'rgba(0, 0, 0, 0.7)',
            backdropFilter: 'blur(4px)',
            color: 'white',
            fontSize: '10px',
            padding: '2px 8px',
            borderRadius: '4px',
            fontFamily: 'monospace',
            letterSpacing: '0.05em',
            border: '1px solid rgba(255, 255, 255, 0.1)',
            opacity: 0.8,
            whiteSpace: 'nowrap'
          }}>
            {selection.width} × {selection.height}
          </div>
        </div>
      ) : null}

      {/* HUD Viewfinder Magnifier */}
      {showMagnifier && magnifierImage && (
        <div
          style={{
            position: 'absolute',
            pointerEvents: 'none',
            zIndex: 50,
            backgroundColor: '#111',
            border: '1px solid #444',
            boxShadow: '0 8px 32px rgba(0,0,0,0.8)',
            overflow: 'hidden',
            display: 'flex',
            flexDirection: 'column',
            left: magX,
            top: magY,
            width: MAGNIFIER_WIDTH,
            height: MAGNIFIER_HEIGHT,
          }}
        >
          {/* Header Bar */}
          <div style={{ height: '20px', backgroundColor: '#222', borderBottom: '1px solid #444', display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '0 8px', flexShrink: 0 }}>
             <span style={{ color: '#888', fontSize: '9px', fontFamily: 'monospace', letterSpacing: '0.05em' }}>HUD_VIEW</span>
             <span style={{ color: '#ccc', fontSize: '9px', fontFamily: 'monospace', letterSpacing: '0.05em' }}>{zoom}X</span>
          </div>

          {/* Image Container */}
          <div style={{ position: 'relative', flex: 1, backgroundColor: '#111', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <div
              style={{
                position: 'absolute',
                inset: 0,
                backgroundImage: `url(${magnifierImage})`,
                backgroundSize: '100% 100%',
                backgroundPosition: 'center',
                imageRendering: 'pixelated',
              }}
            />

            {/* Central Hollow Crosshair */}
            <div style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
               <div 
                 style={{ 
                   width: zoom, 
                   height: zoom, 
                   boxShadow: '0 0 0 1px rgba(255,255,255,0.9), 0 0 0 2px rgba(0,0,0,0.6)' 
                 }} 
               />
               
               {/* Minimalist HUD Cross lines */}
               <div style={{ position: 'absolute', width: '1px', height: '8px', backgroundColor: 'rgba(255,255,255,0.5)', top: 0, left: '50%', transform: 'translateX(-50%)' }} />
               <div style={{ position: 'absolute', width: '1px', height: '8px', backgroundColor: 'rgba(255,255,255,0.5)', bottom: 0, left: '50%', transform: 'translateX(-50%)' }} />
               <div style={{ position: 'absolute', height: '1px', width: '8px', backgroundColor: 'rgba(255,255,255,0.5)', left: 0, top: '50%', transform: 'translateY(-50%)' }} />
               <div style={{ position: 'absolute', height: '1px', width: '8px', backgroundColor: 'rgba(255,255,255,0.5)', right: 0, top: '50%', transform: 'translateY(-50%)' }} />
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
