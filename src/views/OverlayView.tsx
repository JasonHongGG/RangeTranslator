import {
  startTransition,
  useDeferredValue,
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
} from 'react'
import { call, currentTauriWindow, isTauri, watchEvent } from '../bridge'
import {
  PANEL_RESIZE_HANDLES,
  PREVIEW_SNAPSHOT,
  PREVIEW_TRANSLATION,
  type ResizeDirection,
} from '../app/constants'
import { formatUnknown, writeLocalDebug } from '../app/debug'
import {
  mergeTranslationPartial,
  mergeTranslationUpdate,
  sameSelection,
  shouldIgnoreWindowDrag,
} from '../app/overlay'
import { buildOverlayRenderModel } from '../app/overlay-view-model'
import type {
  OverlayInteractionMode,
  RuntimeSnapshot,
  SelectionRect,
  TranslationPartialPayload,
  TranslationPayload,
} from '../types'
import { OverlayDebugLayer } from './overlay/DebugLayer'
import { OverlayMaskLayer } from './overlay/MaskLayer'
import { OverlayTextLayer } from './overlay/TextLayer'

export function OverlayView() {
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot>(PREVIEW_SNAPSHOT)
  const [translation, setTranslation] = useState<TranslationPayload>(PREVIEW_TRANSLATION)
  const deferredTranslation = useDeferredValue(translation)
  const overlayWindow = useMemo(() => currentTauriWindow(), [])
  const boundsRef = useRef<SelectionRect | null>(PREVIEW_SNAPSHOT.selection)
  const overlayBoundsSyncArmedRef = useRef(false)
  const overlayViewport = {
    width: Math.max(window.innerWidth, 1),
    height: Math.max(window.innerHeight, 1),
  }
  const renderModel = buildOverlayRenderModel(snapshot, deferredTranslation, overlayViewport)
  const { allowsTextSelection, geometryContext, isInteractive, sourceUnits, translationBySourceId, visibleLayer } = renderModel

  const syncSnapshot = useEffectEvent((next: RuntimeSnapshot) => {
    startTransition(() => {
      setSnapshot(next)
    })
  })

  const syncTranslation = useEffectEvent((next: TranslationPayload) => {
    startTransition(() => {
      setTranslation((current) => mergeTranslationUpdate(current, next))
    })
  })

  const syncPartialTranslation = useEffectEvent((next: TranslationPartialPayload) => {
    startTransition(() => {
      setTranslation((current) => mergeTranslationPartial(current, next))
    })
  })

  useEffect(() => {
    boundsRef.current = snapshot.selection
  }, [snapshot.selection])

  const syncOverlayBounds = useEffectEvent(async () => {
    if (!overlayWindow || !isTauri() || !isInteractive) {
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
    let detachPartial = () => {}
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
    void watchEvent<TranslationPartialPayload>('translation-partial', syncPartialTranslation).then(
      (unlisten) => {
        detachPartial = unlisten
      },
    )

    return () => {
      stopped = true
      detachSnapshot()
      detachTranslation()
      detachPartial()
    }
  }, [overlayWindow])

  useEffect(() => {
    if (!overlayWindow || !isTauri() || !isInteractive) {
      overlayBoundsSyncArmedRef.current = false
      return
    }

    let detachMoved = () => {}
    let detachResized = () => {}
    let syncTimer: number | null = null
    let settleTimer: number | null = null

    const queueSync = () => {
      if (!overlayBoundsSyncArmedRef.current) {
        return
      }

      if (syncTimer !== null) {
        window.clearTimeout(syncTimer)
      }

      if (settleTimer !== null) {
        window.clearTimeout(settleTimer)
      }

      syncTimer = window.setTimeout(() => {
        void syncOverlayBounds()
      }, 80)

      settleTimer = window.setTimeout(() => {
        overlayBoundsSyncArmedRef.current = false
      }, 260)
    }

    void overlayWindow
      .onMoved(() => {
        queueSync()
      })
      .then((unlisten) => {
        detachMoved = unlisten
      })

    void overlayWindow
      .onResized(() => {
        queueSync()
      })
      .then((unlisten) => {
        detachResized = unlisten
      })

    return () => {
      if (syncTimer !== null) {
        window.clearTimeout(syncTimer)
      }
      if (settleTimer !== null) {
        window.clearTimeout(settleTimer)
      }
      detachMoved()
      detachResized()
      overlayBoundsSyncArmedRef.current = false
    }
  }, [overlayWindow, isInteractive])

  const startOverlayDrag = async (event: React.PointerEvent<HTMLElement>) => {
    if (!overlayWindow || event.button !== 0 || !isInteractive) {
      return
    }

    const target = event.target as HTMLElement
    if (allowsTextSelection && shouldIgnoreWindowDrag(target)) {
      return
    }

    try {
      overlayBoundsSyncArmedRef.current = true
      await overlayWindow.startDragging()
    } catch {
      // Ignore drag rejections from edge cases like rapid resize gestures.
    }
  }

  const startOverlayResize = (direction: ResizeDirection) => async (
    event: React.PointerEvent<HTMLButtonElement>,
  ) => {
    if (!overlayWindow || event.button !== 0 || !isInteractive) {
      return
    }

    event.preventDefault()
    event.stopPropagation()

    try {
      overlayBoundsSyncArmedRef.current = true
      await overlayWindow.startResizeDragging(direction)
    } catch {
      // Ignore resize rejections from unsupported host edge cases.
    }
  }

  return (
    <div
      className={`overlay-page ${isInteractive ? 'overlay-interactive' : 'overlay-passive'} ${overlayModeClass(snapshot.overlayMode)}`}
      data-tauri-drag-region={snapshot.overlayMode === 'dragWindow' ? 'true' : undefined}
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

      <OverlayMaskLayer
        visible={visibleLayer !== 'none'}
        sourceUnits={sourceUnits}
        geometryContext={geometryContext}
      />

      <OverlayTextLayer
        sourceUnits={sourceUnits}
        translationBySourceId={translationBySourceId}
        geometryContext={geometryContext}
        visibleLayer={visibleLayer}
        allowsTextSelection={allowsTextSelection}
      />

      <OverlayDebugLayer
        sourceUnits={sourceUnits}
        geometryContext={geometryContext}
      />
    </div>
  )
}

function overlayModeClass(mode: OverlayInteractionMode) {
  switch (mode) {
    case 'selectText':
      return 'overlay-mode-select-text'
    case 'dragWindow':
      return 'overlay-mode-drag-window'
    default:
      return 'overlay-mode-pass-through'
  }
}
