import path from 'node:path'
import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'

// Servi tel quel à la fenêtre Tauri (devUrl) ; port fixe pour que Tauri le trouve.
export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  resolve: {
    alias: { $lib: path.resolve('./src/lib') },
  },
  clearScreen: false,
  server: { port: 5173, strictPort: true },
})
