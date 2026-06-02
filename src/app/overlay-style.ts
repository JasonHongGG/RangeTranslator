import type { OverlaySourceUnit } from '../types.ts'

const LIGHT_INK = '#F4EFE5'
const DARK_INK = '#121A21'
const LIGHT_SURFACE = '#F1ECE4'
const DARK_SURFACE = '#1E242B'

export type OverlayResolvedStyle = {
  foreground: string
  background: string
}

export function resolveOverlayStyle(
  unit: Pick<OverlaySourceUnit, 'foreground' | 'background' | 'styleConfidence'>,
): OverlayResolvedStyle {
  const confidence = clamp(unit.styleConfidence, 0, 1)
  const background = resolveBackdropColor(unit.background, confidence)
  const foreground = resolveForegroundColor(unit.foreground, background, confidence)

  return {
    foreground,
    background,
  }
}

function resolveBackdropColor(background: string, confidence: number) {
  const normalized = normalizeHex(background)
  if (!normalized) {
    return background
  }

  if (confidence >= 0.6) {
    return normalized
  }

  const anchor = luminance(normalized) > 148 ? LIGHT_SURFACE : DARK_SURFACE
  const mixAmount = ((0.6 - confidence) / 0.6) * 0.22
  return mixColors(normalized, anchor, mixAmount)
}

function resolveForegroundColor(
  foreground: string,
  background: string,
  confidence: number,
) {
  const normalizedBackground = normalizeHex(background)
  const fallback =
    normalizedBackground && luminance(normalizedBackground) > 148 ? DARK_INK : LIGHT_INK
  const normalizedForeground = normalizeHex(foreground)

  if (!normalizedBackground || !normalizedForeground) {
    return normalizedForeground ?? fallback
  }

  const distance = colorDistance(normalizedForeground, normalizedBackground)
  if (confidence >= 0.6 && distance >= 34) {
    return normalizedForeground
  }

  if (confidence >= 0.4 && distance >= 52) {
    return normalizedForeground
  }

  return fallback
}

function normalizeHex(color: string) {
  const normalized = color.trim().replace('#', '')
  if (!/^[0-9a-fA-F]{6}$/.test(normalized)) {
    return null
  }

  return `#${normalized.toUpperCase()}`
}

function mixColors(left: string, right: string, amount: number) {
  const a = parseHex(left)
  const b = parseHex(right)
  const weight = clamp(amount, 0, 1)

  return toHex({
    red: Math.round(a.red + (b.red - a.red) * weight),
    green: Math.round(a.green + (b.green - a.green) * weight),
    blue: Math.round(a.blue + (b.blue - a.blue) * weight),
  })
}

function parseHex(color: string) {
  const normalized = normalizeHex(color) ?? '#202020'
  return {
    red: Number.parseInt(normalized.slice(1, 3), 16),
    green: Number.parseInt(normalized.slice(3, 5), 16),
    blue: Number.parseInt(normalized.slice(5, 7), 16),
  }
}

function toHex(color: { red: number; green: number; blue: number }) {
  return `#${toChannel(color.red)}${toChannel(color.green)}${toChannel(color.blue)}`
}

function toChannel(value: number) {
  return clamp(Math.round(value), 0, 255).toString(16).padStart(2, '0').toUpperCase()
}

function colorDistance(left: string, right: string) {
  const a = parseHex(left)
  const b = parseHex(right)
  const red = a.red - b.red
  const green = a.green - b.green
  const blue = a.blue - b.blue

  return Math.sqrt(red * red + green * green + blue * blue)
}

function luminance(color: string) {
  const rgb = parseHex(color)
  return 0.2126 * rgb.red + 0.7152 * rgb.green + 0.0722 * rgb.blue
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max)
}