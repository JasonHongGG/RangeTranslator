import { useEffect, useMemo } from 'react'
import { describeTarget, writeLocalDebug } from './app/debug'
import { resolveRouteInfo } from './app/routing'
import { OverlayView } from './views/OverlayView'
import { PanelView } from './views/PanelView'
import { SelectorView } from './views/SelectorView'
import { SettingsView } from './views/SettingsView'

function App() {
  const route = useMemo(() => resolveRouteInfo(), [])

  useEffect(() => {
    writeLocalDebug('app-router', 'boot', route)
  }, [route])

  useEffect(() => {
    document.documentElement.dataset.appView = route.view
    document.body.dataset.appView = route.view

    const root = document.getElementById('root')
    if (root) {
      root.dataset.appView = route.view
    }
  }, [route.view])

  useEffect(() => {
    if (route.view !== 'selector') {
      return
    }

    let moveLogged = false

    const onPointerDown = (event: PointerEvent) => {
      writeLocalDebug('selector-capture', 'window pointerdown', {
        pointerId: event.pointerId,
        button: event.button,
        buttons: event.buttons,
        point: { x: event.clientX, y: event.clientY },
        target: describeTarget(event.target),
      })
    }

    const onPointerMove = (event: PointerEvent) => {
      if (moveLogged) {
        return
      }
      moveLogged = true
      writeLocalDebug('selector-capture', 'window pointermove', {
        pointerId: event.pointerId,
        buttons: event.buttons,
        point: { x: event.clientX, y: event.clientY },
      })
    }

    const onPointerUp = (event: PointerEvent) => {
      moveLogged = false
      writeLocalDebug('selector-capture', 'window pointerup', {
        pointerId: event.pointerId,
        button: event.button,
        buttons: event.buttons,
        point: { x: event.clientX, y: event.clientY },
      })
    }

    const onMouseDown = (event: MouseEvent) => {
      writeLocalDebug('selector-capture', 'window mousedown', {
        button: event.button,
        buttons: event.buttons,
        point: { x: event.clientX, y: event.clientY },
      })
    }

    const onKeyDown = (event: KeyboardEvent) => {
      writeLocalDebug('selector-capture', 'window keydown', {
        key: event.key,
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
        metaKey: event.metaKey,
      })
    }

    window.addEventListener('pointerdown', onPointerDown, true)
    window.addEventListener('pointermove', onPointerMove, true)
    window.addEventListener('pointerup', onPointerUp, true)
    window.addEventListener('mousedown', onMouseDown, true)
    window.addEventListener('keydown', onKeyDown, true)

    return () => {
      window.removeEventListener('pointerdown', onPointerDown, true)
      window.removeEventListener('pointermove', onPointerMove, true)
      window.removeEventListener('pointerup', onPointerUp, true)
      window.removeEventListener('mousedown', onMouseDown, true)
      window.removeEventListener('keydown', onKeyDown, true)
    }
  }, [route])

  if (route.view === 'selector') {
    return <SelectorView />
  }

  if (route.view === 'overlay') {
    return <OverlayView />
  }

  if (route.view === 'settings') {
    return <SettingsView />
  }

  return <PanelView />
}

export default App
