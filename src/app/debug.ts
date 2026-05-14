import type { DebugPayload } from './constants'

export function logDebugPayload(payload: DebugPayload) {
  const title = `[RangeTranslator:${payload.scope}] ${payload.message} @ ${payload.timestamp}`
  if (payload.detail == null) {
    console.info(title)
    return
  }

  console.groupCollapsed(title)
  console.log(payload.detail)
  console.groupEnd()
}

export function writeLocalDebug(scope: string, message: string, detail?: unknown) {
  logDebugPayload({
    scope,
    message,
    detail: detail ?? null,
    timestamp: new Date().toISOString(),
  })
}

export function formatDebugLine(payload: DebugPayload) {
  const detail = payload.detail == null ? '' : ` ${formatUnknown(payload.detail)}`
  return `${payload.scope}: ${payload.message}${detail}`.trim()
}

export function formatUnknown(value: unknown): string {
  if (typeof value === 'string') {
    return value
  }

  if (value instanceof Error) {
    return value.message
  }

  try {
    return JSON.stringify(value)
  } catch {
    return String(value)
  }
}

export function describeTarget(target: EventTarget | null) {
  if (!(target instanceof Element)) {
    return null
  }

  return {
    tag: target.tagName.toLowerCase(),
    id: target.id || null,
    className: target.getAttribute('class'),
  }
}
