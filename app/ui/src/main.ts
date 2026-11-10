import { mount } from 'svelte'
import { isTauri } from '@tauri-apps/api/core'
import './app.css'
import App from './App.svelte'

async function start() {
  // Hors de Tauri (Vite dans un navigateur), les commandes et événements sont simulés.
  if (import.meta.env.DEV && !isTauri()) {
    const { mockBackend } = await import('$lib/backend/mock')
    mockBackend()
  }
  mount(App, { target: document.getElementById('app')! })
}

start()
