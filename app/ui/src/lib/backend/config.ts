// Config chargée par l'appli (vue « Règles ») : commandes `config_state` et
// `config_path`, événement `config-changed` à chaque chargement.

import { call, subscribe } from '.'
import type { UnlistenFn } from '@tauri-apps/api/event'

/** Une Règle de la config active. */
export type RuleState = { pattern: string; pipeline: string }

/** Une étape d'un Pipeline : la Transformation et ses paramètres sur une ligne, tels qu'écrits. */
export type Step = { transformation: string; params: string }

export type NamedPipeline = { name: string; steps: Step[] }

/**
 * Refus du dernier chargement : fichier absent ou illisible (`unreadable`) ou
 * contenu invalide (`invalid`). `field` est vide quand c'est le fichier entier ;
 * `reason` peut tenir sur plusieurs lignes.
 */
export type Refusal = { kind: 'unreadable' | 'invalid'; field: string; reason: string }

/** Config active (dans l'ordre du fichier) et refus du dernier chargement. */
export type ConfigState = {
  rules: RuleState[]
  pipelines: NamedPipeline[]
  /** Faux tant qu'aucune config n'a été acceptée : aucune Règle n'est active. */
  loaded: boolean
  error: Refusal | null
}

export const CONFIG_CHANGED = 'config-changed'

export function configState(): Promise<ConfigState> {
  return call<ConfigState>('config_state')
}

/** Chemin du fichier de config ; `null` si l'appli n'en a pas. */
export function configPath(): Promise<string | null> {
  return call<string | null>('config_path')
}

export function onConfigChanged(handler: (state: ConfigState) => void): Promise<UnlistenFn> {
  return subscribe<ConfigState>(CONFIG_CHANGED, handler)
}
