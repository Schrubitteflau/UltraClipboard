// Historique des transformations (vue « Historique ») : commande `history`,
// événement `history-changed` à chaque nouvelle entrée.

import { call, subscribe } from '.'
import type { UnlistenFn } from '@tauri-apps/api/event'

/** Ce qu'a donné le Pipeline. */
export type HistoryStatus =
  /** Le presse-papier n'est écrit que si `result` diffère de l'Original. */
  | { status: 'success'; result: string }
  /**
   * Presse-papier pas touché. `step` : Transformation en cause, `null` quand
   * c'est le résultat entier qui est refusé (résultat vide). `stderr` peut être vide.
   */
  | { status: 'failure'; step: string | null; message: string; stderr: string }
  /** Une copie plus récente est arrivée avant la fin : résultat jeté. */
  | { status: 'cancelled' }

/** Une copie qui a déclenché un Pipeline. */
export type HistoryEntry = {
  /** Heure de la copie, en millisecondes depuis l'époque Unix. */
  at: number
} & UntimedEntry

/** Une entrée sans son heure. */
export type UntimedEntry = {
  /** Motif de la Règle, tel qu'écrit. */
  rule: string
  pipeline: string
  original: string
} & HistoryStatus

export const HISTORY_CHANGED = 'history-changed'

/** Dernières transformations (50 au plus), la plus récente en tête. */
export function history(): Promise<HistoryEntry[]> {
  return call<HistoryEntry[]>('history')
}

export function onHistoryChanged(handler: (entries: HistoryEntry[]) => void): Promise<UnlistenFn> {
  return subscribe<HistoryEntry[]>(HISTORY_CHANGED, handler)
}
