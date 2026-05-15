import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow, type Window } from '@tauri-apps/api/window'

export function isTauri() {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export function currentTauriWindow(): Window | null {
  if (!isTauri()) {
    return null
  }

  try {
    return getCurrentWindow()
  } catch {
    return null
  }
}

export function currentTauriWindowLabel() {
  return currentTauriWindow()?.label ?? null
}

export async function call<T = void>(
  command: string,
  args?: Record<string, unknown>,
) {
  return invoke<T>(command, args)
}

export async function watchEvent<T>(
  name: string,
  onPayload: (payload: T) => void,
) {
  if (!isTauri()) {
    return () => {}
  }

  return listen<T>(name, (event) => onPayload(event.payload))
}