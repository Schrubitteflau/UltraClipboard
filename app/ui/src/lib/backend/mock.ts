// Appli simulée, pour charger la fenêtre dans un navigateur ordinaire (`bun run dev`).
// Chaque commande ajoutée côté Rust reçoit ici une réponse plausible.

import { mockIPC } from '@tauri-apps/api/mocks'

const commands: Record<string, (args?: unknown) => unknown> = {}

export function mockBackend() {
  mockIPC(
    (command, args) => {
      const handler = commands[command]
      if (!handler) throw new Error(`commande simulée inconnue : ${command}`)
      return handler(args)
    },
    { shouldMockEvents: true },
  )
}
