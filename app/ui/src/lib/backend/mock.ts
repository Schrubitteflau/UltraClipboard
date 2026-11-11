// Appli simulée, pour charger la fenêtre dans un navigateur ordinaire (`bun run dev`).
// Chaque commande ajoutée côté Rust reçoit ici une réponse plausible.
//
// Depuis la console du navigateur, `ucMock` simule ce que pousse l'appli :
// `ucMock.pushConfig(ucMock.configs.refused)` (ou une autre clé de `ucMock.configs`, ou
// un état écrit à la main) ; `ucMock.copy()` ajoute une entrée d'historique (ou
// `ucMock.copy(ucMock.entries.failure)`, ou une entrée écrite à la main).

import { mockIPC } from '@tauri-apps/api/mocks'
import { emit } from '@tauri-apps/api/event'
import { CONFIG_CHANGED, type ConfigState } from './config'
import { HISTORY_CHANGED, type HistoryEntry, type UntimedEntry } from './history'

const valid: ConfigState = {
  rules: [
    { pattern: '^https?://(www\\.)?(youtube\\.com/watch|youtu\\.be/)', pipeline: 'youtube-markdown' },
    { pattern: '^\\s*\\d[\\d\\s.,]*$', pipeline: 'clean' },
  ],
  pipelines: [
    {
      name: 'youtube-markdown',
      steps: [
        { transformation: 'trim', params: '' },
        {
          transformation: 'command',
          params:
            '["sh", "-c", "url=$(cat)\\ncurl -fsS --get --data-urlencode \\"url=$url\\" --data-urlencode \\"format=json\\" https://www.youtube.com/oembed | jq -er --arg url \\"$url\\" \'\\"[\\\\(.title)](\\\\($url)) — [\\\\(.author_name)](\\\\(.author_url))\\"\'\\n"]',
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

const youtubeRule = configs.valid.rules[0].pattern
const numberRule = configs.valid.rules[1].pattern
/** Il y a `minutes` minutes. */
const ago = (minutes: number) => Date.now() - minutes * 60_000

/** Une entrée de chaque sorte, sans l'heure. */
const entries = {
  success: {
    rule: youtubeRule,
    pipeline: 'youtube-markdown',
    original: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
    status: 'success',
    result:
      '[Faire son pain au levain](https://www.youtube.com/watch?v=dQw4w9WgXcQ) — [La Boulange](https://www.youtube.com/@laboulange)',
  },
  failure: {
    rule: numberRule,
    pipeline: 'clean',
    original: ' 1 234,50 ',
    status: 'failure',
    step: 'command',
    message: 'la commande a échoué (code de sortie 1)',
    stderr:
      'Traceback (most recent call last):\n  File "/home/moi/scripts/fix-numbers.py", line 4, in <module>\n    print(float(text))\nValueError: could not convert string to float: \'1 234,50\'\n',
  },
  empty: {
    rule: numberRule,
    pipeline: 'clean',
    original: '   ',
    status: 'failure',
    step: null,
    message: 'résultat vide',
    stderr: '',
  },
  unchanged: {
    rule: numberRule,
    pipeline: 'clean',
    original: '42',
    status: 'success',
    result: '42',
  },
  cancelled: {
    rule: youtubeRule,
    pipeline: 'youtube-markdown',
    original: 'https://youtu.be/abc123',
    status: 'cancelled',
  },
} satisfies Record<string, UntimedEntry>

/** La plus récente en tête. */
let historyEntries: HistoryEntry[] = [
  { ...entries.success, at: ago(1) },
  { ...entries.cancelled, at: ago(2) },
  { ...entries.failure, at: ago(5) },
  { ...entries.empty, at: ago(9) },
  { ...entries.unchanged, at: ago(12) },
  /** D'hier : la date s'affiche. */
  { ...entries.success, at: ago(26 * 60) },
]

const commands: Record<string, (args?: unknown) => unknown> = {
  config_state: () => config,
  config_path: () => '/home/moi/.config/ultraclipboard/config.yaml',
  history: () => historyEntries,
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
      entries,
      /** Simule une copie transformée : l'entrée (maintenant) passe en tête, 50 au plus. */
      copy(entry: UntimedEntry = entries.success) {
        historyEntries = [{ ...entry, at: Date.now() }, ...historyEntries].slice(0, 50)
        return emit(HISTORY_CHANGED, historyEntries)
      },
      /** Simule un lancement : historique vide. */
      clearHistory() {
        historyEntries = []
        return emit(HISTORY_CHANGED, historyEntries)
      },
    },
  })
}
