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
import { call, isTauri, watchEvent } from '../bridge'
import {
  PANEL_RESIZE_HANDLES,
  PREVIEW_SNAPSHOT,
  PREVIEW_TRANSLATION,
  type ResizeDirection,
} from '../app/constants'
import { formatUnknown, writeLocalDebug } from '../app/debug'
import {
  mergeTranslationPartial,
  readWindowScale,
  sameSelection,
  shouldIgnoreWindowDrag,
  toLogicalPixels,
  withAlpha,
} from '../app/overlay'
import type {
  RuntimeSnapshot,
  SelectionRect,
  TranslationPartialPayload,
  TranslationPayload,
} from '../types'

export function OverlayView() {
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot>(PREVIEW_SNAPSHOT)
  const [translation, setTranslation] = useState<TranslationPayload>(PREVIEW_TRANSLATION)
  const [overlayScale, setOverlayScale] = useState(1)
  const deferredBlocks = useDeferredValue(translation.blocks)
  const overlayWindow = useMemo(() => (isTauri() ? getCurrentWindow() : null), [])
  const boundsRef = useRef<SelectionRect | null>(PREVIEW_SNAPSHOT.selection)
  const overlayBoundsSyncArmedRef = useRef(false)

  const syncSnapshot = useEffectEvent((next: RuntimeSnapshot) => {
    startTransition(() => {
      setSnapshot(next)
    })
  })

  const syncTranslation = useEffectEvent((next: TranslationPayload) => {
    startTransition(() => {
      setTranslation((current) =>
        next.generation >= current.generation ? next : current,
      )
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
    if (!overlayWindow || !isTauri() || !snapshot.copyMode) {
      return
    }

    try {
      const [position, size, scaleFactor] = await Promise.all([
        overlayWindow.innerPosition(),
        overlayWindow.innerSize(),
        overlayWindow.scaleFactor(),
      ])

      setOverlayScale(scaleFactor)

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
        const requests: Array<Promise<unknown>> = [
          call<RuntimeSnapshot>('get_runtime_snapshot'),
          call<TranslationPayload>('get_latest_translation'),
        ]

        if (overlayWindow) {
          requests.push(readWindowScale(overlayWindow))
        }

        const [nextSnapshot, nextTranslation, nextScale] = await Promise.all(requests)

        if (!stopped) {
          setSnapshot(nextSnapshot as RuntimeSnapshot)
          setTranslation(nextTranslation as TranslationPayload)
          if (typeof nextScale === 'number') {
            setOverlayScale(nextScale)
          }
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
  }, [])

  useEffect(() => {
    if (!overlayWindow || !isTauri() || !snapshot.copyMode) {
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
  }, [overlayWindow, snapshot.copyMode, syncOverlayBounds])

  const startOverlayDrag = async (event: React.PointerEvent<HTMLElement>) => {
    if (!overlayWindow || event.button !== 0 || !snapshot.copyMode) {
      return
    }

    const target = event.target as HTMLElement
    if (shouldIgnoreWindowDrag(target)) {
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
    if (!overlayWindow || event.button !== 0 || !snapshot.copyMode) {
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
      className={`overlay-page ${snapshot.copyMode ? 'overlay-interactive overlay-copy-mode' : 'overlay-passive'}`}
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
        <span className={`overlay-mode ${snapshot.copyMode ? 'overlay-mode-edit' : 'overlay-mode-pass'}`}>
          {snapshot.copyMode ? 'EDIT' : 'PASS'}
        </span>
      </div>

      {deferredBlocks.map((block) => (
        <article
          key={block.id}
          className={`overlay-block overlay-block-${block.align} ${block.streaming ? 'overlay-block-streaming' : ''}`}
          data-no-drag="true"
          style={{
            left: toLogicalPixels(block.x, overlayScale),
            top: toLogicalPixels(block.y, overlayScale),
            width: toLogicalPixels(block.width, overlayScale),
            height: Math.max(1, toLogicalPixels(block.height, overlayScale)),
            color: block.foreground,
            background: withAlpha(block.background, 0.76),
            fontSize: Math.max(10, block.fontSize / Math.max(overlayScale, 1)),
          }}
        >
          {block.translatedText || block.sourceText}
        </article>
      ))}
    </div>
  )
}
