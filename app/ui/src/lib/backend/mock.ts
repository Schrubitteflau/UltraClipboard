// Appli simulée, pour charger la fenêtre dans un navigateur ordinaire (`bun run dev`).
// Chaque commande ajoutée côté Rust reçoit ici une réponse plausible.
//
// Depuis la console du navigateur, `ucMock` simule ce que pousse l'appli :
// `ucMock.pushConfig(ucMock.configs.refused)` (ou une autre clé de `ucMock.configs`, ou
// un état écrit à la main).

import { mockIPC } from '@tauri-apps/api/mocks'
import { emit } from '@tauri-apps/api/event'
import { CONFIG_CHANGED, type ConfigState } from './config'

const valid: ConfigState = {
  rules: [
    { pattern: '^https?://(www\\.)?(youtube\\.com/watch|youtu\\.be/)', pipeline: 'youtube-markdown' },
    { pattern: '^\\s*\\d[\\d\\s.,]*$', pipeline: 'clean' },
  ],
  pipelines: [
    {
      name: 'youtube-markdown',
      steps: [
        {
          transformation: 'youtube',
          params: 'template: "[{title}]({url}) — [{author_name}]({author_url})"',
        },
      ],
    },
    {
      name: 'clean',
      steps: [
        { transformation: 'trim', params: '' },
        { transformation: 'command', params: '["python3", "~/scripts/fix-numbers.py"], timeout: 10' },
      ],
    },
  ],
  loaded: true,
  error: null,
}

/** Raison rendue par le moteur pour un YAML mal indenté (sur plusieurs lignes). */
const yamlError =
  'YAML invalide : error: line 5 column 5: while parsing a block mapping, did not find expected key\n --> <input>:5:5\n  |\n3 |   clean:\n4 |     - trim\n5 |   - upper\n  |     ^ while parsing a block mapping, did not find expected key\n6 | rules: []\n  |'

const configs = {
  valid,
  /** Enregistrement refusé : la config précédente reste active. */
  refused: {
    ...valid,
    error: { kind: 'invalid', field: 'rules[1].pipeline', reason: 'Pipeline inconnu « clen »' },
  },
  /** Enregistrement refusé, YAML illisible : erreur sur plusieurs lignes. */
  refusedYaml: { ...valid, error: { kind: 'invalid', field: '', reason: yamlError } },
  /** Fichier supprimé : la config chargée avant reste active. */
  unreadable: { ...valid, error: { kind: 'unreadable', field: '', reason: 'fichier absent' } },
  /** Config refusée dès le lancement : aucune Règle active. */
  firstLaunch: {
    rules: [],
    pipelines: [],
    loaded: false,
    error: { kind: 'invalid', field: '', reason: yamlError },
  },
} satisfies Record<string, ConfigState>

let config: ConfigState = configs.valid

const commands: Record<string, (args?: unknown) => unknown> = {
  config_state: () => config,
  config_path: () => '/home/moi/.config/ultraclipboard/config.yaml',
}

export function mockBackend() {
  mockIPC(
    (command, args) => {
      const handler = commands[command]
      if (!handler) throw new Error(`commande simulée inconnue : ${command}`)
      return handler(args)
    },
    { shouldMockEvents: true },
  )
  Object.assign(window, {
    ucMock: {
      configs,
      /** Simule un chargement de config : la commande rend cet état et l'événement le pousse. */
      pushConfig(state: ConfigState) {
        config = state
        return emit(CONFIG_CHANGED, state)
      },
    },
  })
}
