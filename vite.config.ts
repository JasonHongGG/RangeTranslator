import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    watch: {
      ignored: [
        '**/src-tauri/target/**',
        '**/range-translator-runtime/.runtime/**',
        '**/range-translator-runtime/.venv/**',
      ],
    },
  },
})
