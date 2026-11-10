// Passerelle vers l'appli : les vues lisent par commandes Tauri et se mettent à
// jour sur événements. Seul endroit du front qui parle à Tauri.

import { invoke, type InvokeArgs } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

/** Appelle une commande de l'appli et rend sa réponse. */
export function call<T>(command: string, args?: InvokeArgs): Promise<T> {
  return invoke<T>(command, args)
}

/** Écoute un événement poussé par l'appli ; rend la fonction qui arrête l'écoute. */
export function subscribe<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload))
}
