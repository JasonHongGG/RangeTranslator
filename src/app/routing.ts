import { currentTauriWindowLabel } from '../bridge'
import type { AppView, RouteInfo } from './constants'

export function selectorOrigin() {
  const url = new URL(window.location.href)
  const params = new URLSearchParams(url.search)

  if (!params.has('ox') && !params.has('oy')) {
    const hashQuery = window.location.hash.split('?')[1] ?? ''
    const hashParams = new URLSearchParams(hashQuery)
    return {
      x: Number(hashParams.get('ox') ?? 0),
      y: Number(hashParams.get('oy') ?? 0),
    }
  }

  return {
    x: Number(params.get('ox') ?? 0),
    y: Number(params.get('oy') ?? 0),
  }
}

export function readInjectedView() {
  if (typeof window === 'undefined') {
    return null
  }

  const value = (window as Window & {
    __RANGE_TRANSLATOR_VIEW__?: unknown
  }).__RANGE_TRANSLATOR_VIEW__

  return typeof value === 'string' ? value : null
}

export function resolveRouteInfo(): RouteInfo {
  const url = new URL(window.location.href)
  const scriptView = readInjectedView()
  const currentWindowLabel = currentTauriWindowLabel()
  const queryView = url.searchParams.get('view')
  const hashView = extractHashView(url.hash)
  const pathView = extractPathView(url.pathname)

  return {
    view:
      normalizeView(scriptView) ??
      normalizeView(currentWindowLabel) ??
      normalizeView(queryView) ??
      normalizeView(hashView) ??
      normalizeView(pathView) ??
      'panel',
    scriptView,
    currentWindowLabel,
    href: url.href,
    pathname: url.pathname,
    search: url.search,
    hash: url.hash,
    queryView,
    hashView,
    pathView,
  }
}

function normalizeView(value: string | null | undefined): AppView | null {
  if (value === 'selector' || value === 'overlay' || value === 'panel' || value === 'settings') {
    return value
  }

  return null
}

function extractHashView(hash: string) {
  if (!hash) {
    return null
  }

  const trimmed = hash.replace(/^#\/?/, '')
  const view = trimmed.split(/[/?&]/)[0]
  return view || null
}

function extractPathView(pathname: string) {
  const segments = pathname
    .split('/')
    .map((segment) => segment.trim())
    .filter(Boolean)
    .filter((segment) => segment !== 'index.html')

  return segments.at(-1) ?? null
}
