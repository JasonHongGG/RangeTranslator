const SHARED = [
  { code: 'en-US', label: 'English', nativeLabel: 'English' },
  { code: 'ja-JP', label: 'Japanese', nativeLabel: '日本語' },
  { code: 'ko-KR', label: 'Korean', nativeLabel: '한국어' },
  { code: 'zh-Hans', label: 'Chinese Simplified', nativeLabel: '简体中文' },
  { code: 'zh-TW', label: 'Chinese Traditional', nativeLabel: '繁體中文' },
  { code: 'fr-FR', label: 'French', nativeLabel: 'Français' },
  { code: 'de-DE', label: 'German', nativeLabel: 'Deutsch' },
  { code: 'es-ES', label: 'Spanish', nativeLabel: 'Español' },
  { code: 'ru-RU', label: 'Russian', nativeLabel: 'Русский' },
  { code: 'th-TH', label: 'Thai', nativeLabel: 'ไทย' },
  { code: 'vi-VN', label: 'Vietnamese', nativeLabel: 'Tiếng Việt' },
  { code: 'id-ID', label: 'Indonesian', nativeLabel: 'Bahasa Indonesia' },
] as const

export const SOURCE_LANGUAGES = [
  { code: 'auto', label: 'Auto detect', nativeLabel: 'Auto detect' },
  ...SHARED,
]

export const TARGET_LANGUAGES = SHARED

export function languageLabel(code: string) {
  return (
    SOURCE_LANGUAGES.find((option) => option.code === code)?.nativeLabel ?? code
  )
}