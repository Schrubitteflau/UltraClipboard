---
stepsCompleted: [step-01-init, step-02-context, step-03-starter, step-04-decisions, step-05-patterns, step-06-structure, step-07-validation, step-08-complete]
lastStep: 8
status: complete
completedAt: '2026-03-07'
inputDocuments:
  - '_bmad-output/planning-artifacts/prd.md'
  - '_bmad-output/planning-artifacts/product-brief-UltraClipboard-2026-03-05.md'
  - '_bmad-output/planning-artifacts/ux-design-specification.md'
workflowType: 'architecture'
project_name: 'UltraClipboard'
user_name: 'Grochonnou'
date: '2026-03-07'
---

# Architecture Decision Document

_This document builds collaboratively through step-by-step discovery. Sections are appended as we work through each architectural decision together._

## Project Context Analysis

### Requirements Overview

**Functional Requirements (29 FRs, 8 catégories) :**

| Catégorie | FRs | Implications architecturales |
|---|---|---|
| Clipboard Management | FR1–FR4 | OS-level monitor, in-RAM original storage, global shortcut Ctrl+V+V |
| Pipeline Engine | FR5–FR9 | Async executor, timeout management, fallback, dual execution mode |
| Module System | FR10–FR13 | Module loader, boa JS runtime, manifest parser (module.yaml), builtin modules |
| Playground | FR14–FR17 | Svelte UI avec step debugger, Tauri events pour live updates, "Clipboard comme Négatif" |
| Configuration | FR18–FR21 | YAML parser, file watcher hot-reload, trigger evaluator (regex) |
| System Integration | FR22–FR25 | Tauri system tray, window management, daemon toggle |
| Observability & Metrics | FR26–FR28 | Execution log (métadonnées only), cumulative time_saved counter |
| Onboarding | FR29 | DriverJS-style tour dans le playground |

**Non-Functional Requirements :**

| NFR | Contrainte | Implication architecturale |
|---|---|---|
| NFR1 | Daemon idle ≈ 0 CPU/RAM perceptible | Clipboard monitor en mode poll ou event-driven natif OS ; pas de loop active |
| NFR2 | Modules sync : latence imperceptible | Pipeline sync dans le thread principal Rust ou worker léger |
| NFR3 | Cold start < 3s | Pas de chargement lourd au démarrage ; modules chargés on-demand |
| NFR4 | Contenu clipboard jamais sur disque | In-RAM uniquement, lifetime bounded par le prochain Ctrl+C |
| NFR5 | Log = métadonnées uniquement | Séparation stricte payload / metadata dans le log |
| NFR6 | Sandbox JS (pas de fs/network non déclaré) | boa isolé, sans accès aux APIs Tauri ou système depuis les modules |
| NFR7 | Exception module ≠ crash daemon | Panic isolation autour de l'exécution boa |
| NFR8 | Crash daemon → signal tray visible | Watchdog ou health check tray |
| NFR9 | Timeout module async configurable | Tokio timeout wrapper autour de chaque module execution |

**Scale & Complexity :**

- **Primary domain :** Application desktop native (Tauri/Rust + Svelte)
- **Complexity level :** Basse–Moyenne (périmètre maîtrisé ; complexité concentrée sur 3 points techniques)
- **Estimated architectural components :** 9

### Technical Constraints & Dependencies

- **Tauri** comme runtime natif : bridge Rust↔Svelte via commandes et events IPC
- **boa** (JS engine Rust) : couverture ES async partielle — prototype obligatoire avant architecture finale du module runtime
- **YAML** (serde_yaml probable) : parsing config pipelines et module.yaml
- **Svelte** : UI playground et sidebar, communique avec le backend via `invoke()` et `listen()`
- **shadcn-svelte** : design system, composants copy-paste, tokens CSS
- **Projet solo** : zero overhead de coordination d'équipe, API surface minimaliste

### Cross-Cutting Concerns Identified

1. **State machine (idle / processing / error)** — propagée du Pipeline Engine vers le System Tray et le Playground en temps réel via Tauri events
2. **Error containment & fallback** — chaque composant doit garantir la préservation du clipboard original en cas d'échec
3. **Async concurrence** — gestion du cas "Ctrl+V pendant module async en cours" (FR8)
4. **Platform abstraction** — interception clipboard OS-spécifique (X11/Wayland, CGEventTap, Win32)
5. **Security boundary** — clipboard content ne quitte jamais le processus Rust vers le disque ; les modules JS opèrent dans un contexte boa isolé

## Starter Template Evaluation

### Primary Technology Domain

Application desktop native — Tauri 2 (Rust backend) + Svelte (Webview frontend)

### Starter Options Considered

| Option | Pour | Contre |
|---|---|---|
| `create-tauri-app` (svelte-ts) | Officiel, Tauri 2, Svelte standard + Vite, TypeScript | — |
| `create-tauri-app` (sveltekit) | Routing, SSR, adapter-static | Complexité inutile pour mono-fenêtre |
| Template tiers (tauri-sveltekit) | Pré-configuré | Non officiel, maintenance incertaine |

### Selected Starter: `create-tauri-app` — template `svelte-ts`

**Rationale :**
UltraClipboard est une app mono-fenêtre (playground + sidebar) avec un daemon backend Rust. SvelteKit est conçu pour les apps multi-pages avec routing — inutile ici. Le template `svelte-ts` donne exactement ce qui est nécessaire : Vite + Svelte 5 + TypeScript + structure `src-tauri/`.

**Initialization Command:**

```bash
npm create tauri-app@latest ultraclipboard -- --template svelte-ts
```

ou interactif :

```bash
npm create tauri-app@latest
# → nom : ultraclipboard
# → frontend language : TypeScript
# → framework : Svelte
```

**Architectural Decisions Provided by Starter:**

**Language & Runtime:**
- TypeScript strict mode (Svelte frontend)
- Rust (Tauri backend, `src-tauri/`)
- Tauri 2.x — IPC via `invoke()` (commandes) et `listen()` (events)

**Build Tooling:**
- Vite comme bundler frontend (HMR, build rapide)
- Cargo pour le backend Rust
- `tauri dev` pour le développement combiné

**Code Organization:**
```
ultraclipboard/
├── src/                    # Svelte components, stores, UI
│   ├── lib/                # Composants réutilisables
│   └── App.svelte          # Point d'entrée UI
├── src-tauri/              # Backend Rust
│   ├── src/
│   │   └── main.rs         # Point d'entrée Tauri
│   ├── Cargo.toml
│   └── tauri.conf.json     # Config Tauri (tray, window, permissions)
└── package.json
```

**Development Experience:**
- HMR Vite (rechargement UI instantané)
- Compilation Rust incrémentale
- Tauri DevTools pour debug WebView

**Note:** L'initialisation du projet via cette commande sera la première story d'implémentation.

## Core Architectural Decisions

### Decision Priority Analysis

**Décisions critiques (bloquent l'implémentation) :**
- Stratégie de persistance locale (config, log, modules)
- Schéma IPC Tauri (invoke vs events)
- Crates Rust pour clipboard, file watcher, global shortcut
- Granularité des events step-by-step

**Décisions importantes (façonnent l'architecture) :**
- Svelte 5 Runes comme système de réactivité
- Structure des stores frontend
- Routing par conditional rendering typé

**Décisions différées (post-MVP) :**
- Auto-update (`tauri-plugin-updater`) — Phase 2
- SQLite si le log JSONL devient trop limité — Phase 3
- Sandboxing avancé des modules JS — Phase 3

### Data Architecture

**Config & Pipelines (FR18–FR21) :**
- Stockage : répertoire OS standard via `app_config_dir()` (Tauri)
  - Linux : `~/.config/ultraclipboard/`
  - macOS : `~/Library/Application Support/ultraclipboard/`
  - Windows : `%APPDATA%\ultraclipboard\`
- Format : fichiers YAML (pipelines, triggers, settings)
- Hot-reload : crate `notify` surveille les changements, recharge sans redémarrage

**Traceability Log (FR26–FR28) :**
- Format : JSONL (JSON Lines, append-only) dans `app_data_dir()/logs/execution.jsonl`
- Contenu : métadonnées uniquement (timestamp, pipeline_name, outcome, time_saved_ms) — jamais le contenu clipboard (NFR5)
- Rotation : non gérée pour MVP

**Module Storage (FR10–FR13) :**
- Modules builtin (`trim`, `uppercase`, `fetch-youtube-title`) : compilés et embarqués dans le binaire Rust, chargés sans accès filesystem
- Modules externes : `app_data_dir()/modules/{module-name}/` avec `module.yaml` + `index.js`
- Manifest parsing : `serde_yaml` côté Rust

### IPC & Communication Patterns

**Règle de séparation invoke/events :**
- `invoke()` : toute action initiée par l'UI (lancer pipeline, toggle daemon, charger config, Ctrl+V+V recall)
- `emit()` / `listen()` : tout état propagé par le backend (daemon state, pipeline step results, erreurs)

**Events schema :**

| Event | Direction | Payload |
|---|---|---|
| `daemon:state-changed` | Rust → Svelte | `{ state: 'idle' \| 'processing' \| 'error' }` |
| `pipeline:step-complete` | Rust → Svelte | `{ step: number, module: string, output: string, status: 'done' \| 'error' }` |
| `pipeline:complete` | Rust → Svelte | `{ success: boolean, time_saved_ms: number }` |
| `clipboard:captured` | Rust → Svelte | `{ preview: string }` (30 chars max, pas le contenu complet) |

**Crates Rust :**

| Composant | Crate | Raison |
|---|---|---|
| Clipboard read/write | `tauri-plugin-clipboard-manager` | Plugin officiel Tauri 2 |
| Clipboard interception OS | Code natif OS-spécifique (à prototyper) | X11/Wayland/CGEventTap/Win32 — risque technique n°1 |
| File watcher (hot-reload) | `notify` | Standard Rust, async, cross-platform |
| Global shortcut (Ctrl+V+V) | `tauri-plugin-global-shortcut` | Plugin officiel Tauri 2 |
| YAML parsing | `serde_yaml` + `serde` | Standard Rust |
| Async runtime | `tokio` | Intégré à Tauri |
| Module timeout | `tokio::time::timeout` | Wrapper autour de l'exécution boa (NFR9) |

### Frontend Architecture

**Système de réactivité : Svelte 5 Runes**
- `$state` pour l'état mutable local et global
- `$derived` pour les valeurs calculées
- `$effect` pour les side-effects (écoute Tauri events)
- Pas de Svelte 4 stores — projet greenfield, partir sur la base actuelle

**Structure des stores (src/lib/stores/) :**
```
src/lib/stores/
├── daemon.svelte.ts        # $state: 'idle' | 'processing' | 'error'
├── pipeline-run.svelte.ts  # $state: étapes en cours, résultats step-by-step
├── pipelines.svelte.ts     # $state: liste des pipelines configurés
└── log.svelte.ts           # $state: entrées JSONL chargées
```
Chaque store initialise ses `listen()` Tauri au montage et expose des états réactifs aux composants.

**Routing interne — Conditional rendering typé :**
```typescript
type Section = 'playground' | 'pipelines' | 'modules' | 'log' | 'settings'
// activeSection: $state<Section> = 'playground' dans App.svelte
// AppSidebar met à jour activeSection
// App.svelte switche sur activeSection → affiche le composant de section
```
Zéro magic string dispersé, navigation typée, un composant dédié par section.

### Infrastructure & Deployment

**CI/CD — GitHub Actions :**
- Trigger : tag git `v*` (ex. `v0.1.0`)
- Runners : `ubuntu-latest` (.AppImage), `macos-latest` (.dmg), `windows-latest` (.exe/.msi)
- Output : upload automatique sur GitHub Releases
- Gratuit pour repos publics open source

**Développement local :**
- `npm run tauri dev` — dev avec HMR Vite + recompilation Rust incrémentale
- `npm run tauri build` — build release local pour validation

**Versioning :**
- SemVer : `MAJOR.MINOR.PATCH`
- Tags git : `v0.1.0`, `v0.2.0`...
- Changelog : `CHANGELOG.md` (format Keep a Changelog)
- Premier release `v0.1.0` : critère = workflow YouTube → Obsidian fonctionnel

### Decision Impact Analysis

**Séquence d'implémentation recommandée :**
1. Initialiser projet (`create-tauri-app`)
2. **Prototype clipboard interception** — risque n°1, valider avant tout le reste
3. Prototype `boa` bridge — risque n°2, valider `async transform()` end-to-end
4. Pipeline engine (Rust) avec les 3 modules builtin
5. Playground UI (Svelte) + stores + Tauri events
6. Config YAML + hot-reload (`notify`)
7. System tray + daemon toggle + global shortcut
8. Log JSONL + vue Log
9. GitHub Actions CI/CD
10. Onboarding DriverJS

**Dépendances transversales :**
- La state machine `idle/processing/error` est le fil conducteur entre Pipeline Engine, System Tray et Playground — à définir en Rust avant d'implémenter les UI
- Le clipboard interception conditionne toute la mécanique full-auto — si `boa` ou l'interception bloquent, le scope MVP doit être réévalué
## Implementation Patterns & Consistency Rules

### Critical Conflict Points Identified

7 zones où des agents AI pourraient faire des choix différents et incompatibles :
nommage IPC, convention fichiers Svelte, structure Rust, format erreurs,
nommage events, organisation state Svelte, format YAML config.

### Naming Patterns

**Règle fondamentale — Frontière Rust/TypeScript :**
Rust utilise `snake_case`, TypeScript utilise `camelCase`.
La sérialisation Tauri/serde gère la conversion automatiquement via `#[serde(rename_all = "camelCase")]`.
→ **Chaque struct Rust exposée via IPC DOIT avoir `#[serde(rename_all = "camelCase")]`.**

**Tauri Commands (invoke) :**
- Rust : `snake_case` — `#[tauri::command] fn run_pipeline(...)`
- TypeScript : `camelCase` — `invoke('run_pipeline', ...)` (Tauri utilise le nom Rust tel quel)
- Convention : verbe + nom — `run_pipeline`, `toggle_daemon`, `load_config`, `recall_original`

**Tauri Events (emit/listen) :**
- Format : `domain:action` en `kebab-case` — séparateur `:` entre domaine et action
- Exemples corrects : `daemon:state-changed`, `pipeline:step-complete`, `pipeline:complete`, `clipboard:captured`
- Anti-pattern : `daemonStateChanged`, `DAEMON_STATE_CHANGED`, `daemon_state_changed`

**Code TypeScript/Svelte :**
- Variables & fonctions : `camelCase` — `activeSection`, `runPipeline()`
- Types & interfaces : `PascalCase` — `Section`, `PipelineStep`, `DaemonState`
- Constantes : `SCREAMING_SNAKE_CASE` — `MAX_TIMEOUT_MS`
- Fichiers composants Svelte : `PascalCase.svelte` — `AppSidebar.svelte`, `FlowNode.svelte`
- Fichiers TypeScript non-composants : `kebab-case.ts` — `pipeline-run.svelte.ts`, `daemon.svelte.ts`

**Code Rust :**
- Tout en `snake_case` : structs, fonctions, modules, fichiers
- Modules Rust : `clipboard_monitor`, `pipeline_engine`, `module_loader`, `config_manager`

**YAML config (fichiers utilisateur) :**
- Clés en `snake_case` : `pipeline_name`, `content_regex`, `time_saved_seconds`
- Cohérent avec les struct Rust qui parsent ces fichiers

### Structure Patterns

**Organisation des composants Svelte :**
Par type (non par feature) — l'app est trop petite pour justifier une organisation par feature.

```
src/
├── lib/
│   ├── components/          # Composants UI réutilisables
│   │   ├── AppSidebar.svelte
│   │   ├── FlowNode.svelte
│   │   ├── PipelineFlowCanvas.svelte
│   │   ├── StepOutputPanel.svelte
│   │   └── LogEntry.svelte
│   ├── stores/              # État global Svelte 5 Runes
│   │   ├── daemon.svelte.ts
│   │   ├── pipeline-run.svelte.ts
│   │   ├── pipelines.svelte.ts
│   │   └── log.svelte.ts
│   └── utils/               # Fonctions utilitaires pures (pas de state)
│       └── format.ts
├── sections/                # Un composant par section de l'app
│   ├── PlaygroundSection.svelte
│   ├── PipelinesSection.svelte
│   ├── ModulesSection.svelte
│   ├── LogSection.svelte
│   └── SettingsSection.svelte
└── App.svelte               # Root : activeSection + routing conditionnel
```

**Organisation Rust (`src-tauri/src/`) :**
```
src-tauri/src/
├── main.rs                  # Point d'entrée Tauri, setup plugins
├── commands/                # Handlers #[tauri::command] — un fichier par domaine
│   ├── mod.rs
│   ├── pipeline.rs          # run_pipeline, stop_pipeline
│   ├── daemon.rs            # toggle_daemon, get_daemon_state
│   ├── clipboard.rs         # recall_original
│   └── config.rs            # load_config, reload_config
├── clipboard_monitor/       # Interception OS-spécifique
│   ├── mod.rs
│   ├── linux.rs
│   ├── macos.rs
│   └── windows.rs
├── pipeline_engine/         # Exécuteur de pipelines
│   └── mod.rs
├── module_loader/           # Chargement et exécution boa
│   └── mod.rs
└── config_manager/          # YAML parsing + hot-reload
    └── mod.rs
```

**Tests :**
- TypeScript : fichiers co-localisés `Component.test.ts` à côté du composant
- Rust : modules `#[cfg(test)]` inline dans le fichier source (convention Rust)

### Format Patterns

**Tauri Command Response (invoke) :**
Toujours `Result<T, String>` côté Rust — Tauri sérialise automatiquement :
- Succès → valeur directe côté TypeScript
- Erreur → exception catchable avec le message String

```rust
// Rust — pattern correct
#[tauri::command]
async fn run_pipeline(input: String, pipeline_name: String) -> Result<PipelineResult, String> {
    engine.run(&input, &pipeline_name).await.map_err(|e| e.to_string())
}
```

```typescript
// TypeScript — pattern correct
try {
  const result = await invoke<PipelineResult>('run_pipeline', { input, pipelineName })
} catch (error: unknown) {
  // error est un string (le message Rust)
}
```

**Payload des Events Tauri :**
Toujours un objet typé (jamais une valeur primitive seule) :

```typescript
// Correct
{ state: 'idle' | 'processing' | 'error' }
{ step: number, module: string, output: string, status: 'done' | 'error' }

// Anti-pattern
'idle'   // string primitif — non extensible
```

**Format JSONL Log :**
Une entrée par ligne, structure fixe :
```json
{"timestamp":"2026-03-07T09:14:00Z","pipeline":"fetch-youtube-title","outcome":"success","time_saved_ms":4000}
{"timestamp":"2026-03-07T09:15:12Z","pipeline":"fetch-youtube-title","outcome":"error","error":"API timeout"}
```
Jamais de contenu clipboard dans le log (NFR5).

### Communication Patterns

**Pattern d'écoute Tauri dans les stores Svelte :**
Chaque store initialise ses listeners dans un `$effect` au niveau module, pas dans les composants.

```typescript
// daemon.svelte.ts — pattern correct
import { listen } from '@tauri-apps/api/event'

let state = $state<'idle' | 'processing' | 'error'>('idle')

$effect(() => {
  const unlisten = listen<{ state: typeof state }>('daemon:state-changed', (event) => {
    state = event.payload.state
  })
  return () => { unlisten.then(fn => fn()) }
})

export { state as daemonState }
```

**Pattern d'invocation depuis les composants :**
Les composants appellent `invoke()` directement — pas de wrapper intermédiaire pour le MVP.
Les erreurs sont catchées localement dans le composant qui déclenche l'action.

### Process Patterns

**Error Handling — Règle de séparation :**
- **Erreurs récupérables** (module timeout, API indisponible) : propagées via event `pipeline:step-complete` avec `status: 'error'` → UI met à jour FlowNode en état error, icône tray change, log enregistre
- **Erreurs fatales daemon** (panic Rust non catchée) : tray change d'état, redémarrage manuel requis
- **Jamais** de modal bloquant pour une erreur — toujours un signal visuel discret (UX spec)

**Loading States :**
- État `processing` global dans `daemon.svelte.ts` — source de vérité unique pour l'icône tray
- État de run local dans `pipeline-run.svelte.ts` — pour animer les FlowNodes dans le playground
- Les composants ne gèrent pas leur propre état de loading — ils lisent les stores

**Clipboard Original — Règle de lifetime :**
Le contenu original du dernier Ctrl+C est stocké dans un `Mutex<Option<String>>` côté Rust.
Il est écrasé à chaque nouvel événement de copie, jamais écrit sur disque (NFR4).
Le recall (Ctrl+V+V) appelle `invoke('recall_original')` qui restaure le clipboard système.

### Enforcement Guidelines

**Tous les agents AI DOIVENT :**
- Nommer les structs Rust IPC avec `#[serde(rename_all = "camelCase")]`
- Nommer les events Tauri au format `domain:action` en kebab-case
- Retourner `Result<T, String>` depuis tous les `#[tauri::command]`
- Placer les listeners Tauri dans les stores (`src/lib/stores/`), pas dans les composants
- Ne jamais écrire le contenu clipboard dans un fichier ou log
- Utiliser `$state` / `$derived` / `$effect` (Svelte 5 Runes) — pas de `writable()` stores

**Anti-patterns à rejeter systématiquement :**
- Event nommé en camelCase (`daemonStateChanged`) → doit être `daemon:state-changed`
- Struct Rust sans `#[serde(rename_all = "camelCase")]` exposée via IPC
- Contenu clipboard loggué dans execution.jsonl
- Store Svelte 4 (`writable`, `readable`) dans un nouveau fichier
- Command Tauri retournant `Result<T, MyError>` sans `impl ToString for MyError`
- Listener `listen()` instancié dans un composant Svelte (doit être dans le store)

## Project Structure & Boundaries

### Complete Project Directory Structure

```
ultraclipboard/
├── README.md
├── CHANGELOG.md
├── package.json
├── tsconfig.json
├── vite.config.ts
├── .gitignore
├── .github/
│   └── workflows/
│       └── release.yml          # CI/CD : build cross-platform sur tag v*
│
├── src/                         # Frontend Svelte
│   ├── App.svelte               # Root : activeSection $state + routing conditionnel
│   ├── app.css                  # CSS global + tokens shadcn-svelte
│   ├── main.ts                  # Point d'entrée Vite
│   │
│   ├── lib/
│   │   ├── components/          # Composants UI réutilisables
│   │   │   ├── AppSidebar.svelte        # Navigation + daemon toggle
│   │   │   ├── FlowNode.svelte          # Nœud d'étape pipeline (custom)
│   │   │   ├── PipelineFlowCanvas.svelte # Rangée de FlowNodes connectés
│   │   │   ├── StepOutputPanel.svelte   # Sortie de l'étape sélectionnée
│   │   │   ├── LogEntry.svelte          # Ligne du traceability log
│   │   │   └── Onboarding.svelte        # Tour guidé DriverJS (FR29)
│   │   │
│   │   ├── stores/              # État global Svelte 5 Runes
│   │   │   ├── daemon.svelte.ts         # daemonState: idle|processing|error
│   │   │   ├── pipeline-run.svelte.ts   # Étapes en cours, résultats step-by-step
│   │   │   ├── pipelines.svelte.ts      # Liste des pipelines configurés
│   │   │   └── log.svelte.ts            # Entrées JSONL chargées
│   │   │
│   │   └── utils/
│   │       └── format.ts                # Formatage timestamps, preview strings
│   │
│   └── sections/                # Un composant par section sidebar
│       ├── PlaygroundSection.svelte     # FR14–FR17 : input, pipeline, step debugger
│       ├── PipelinesSection.svelte      # FR18–FR21 : éditeur YAML, triggers
│       ├── ModulesSection.svelte        # FR12 : liste modules installés
│       ├── LogSection.svelte            # FR26–FR28 : traceability log + métriques
│       └── SettingsSection.svelte       # Daemon mode, timeout, préférences
│
└── src-tauri/                   # Backend Rust
    ├── Cargo.toml
    ├── tauri.conf.json          # Config Tauri : tray, window, permissions, minSize
    ├── capabilities/            # Tauri 2 permissions (tray, shortcuts, fs, clipboard)
    │   └── default.json
    ├── icons/                   # Icônes app (idle / processing / error)
    │   ├── icon.png
    │   ├── icon-processing.png
    │   └── icon-error.png
    │
    └── src/
        ├── main.rs              # Setup Tauri, plugins, tray, global shortcuts
        │
        ├── commands/            # Handlers #[tauri::command] — un fichier par domaine
        │   ├── mod.rs
        │   ├── pipeline.rs      # run_pipeline, stop_pipeline
        │   ├── daemon.rs        # toggle_daemon, get_daemon_state
        │   ├── clipboard.rs     # recall_original (FR3–FR4)
        │   └── config.rs        # load_config, reload_config
        │
        ├── clipboard_monitor/   # Interception clipboard OS-level (FR1) — risque n°1
        │   ├── mod.rs           # Trait ClipboardMonitor + dispatch
        │   ├── linux.rs         # X11 / Wayland
        │   ├── macos.rs         # CGEventTap
        │   └── windows.rs       # SetClipboardViewer / AddClipboardFormatListener
        │
        ├── pipeline_engine/     # Exécuteur de pipelines (FR5–FR9)
        │   ├── mod.rs           # PipelineEngine : run(), state machine
        │   └── trigger.rs       # Évaluation content_regex / recipient_app_regex
        │
        ├── module_loader/       # Chargement et exécution boa (FR10–FR13) — risque n°2
        │   ├── mod.rs           # ModuleLoader : load(), execute()
        │   ├── builtin/         # Modules builtin compilés (pas de filesystem)
        │   │   ├── trim.rs
        │   │   ├── uppercase.rs
        │   │   └── fetch_youtube_title.rs
        │   └── sandbox.rs       # Contexte boa isolé, timeout wrapper
        │
        ├── config_manager/      # YAML parsing + hot-reload (FR18–FR21)
        │   ├── mod.rs           # ConfigManager : load(), watch()
        │   ├── pipeline_config.rs # Structs Pipeline, Module, Trigger
        │   └── module_manifest.rs # Struct ModuleManifest (module.yaml)
        │
        └── log_manager/         # Traceability log JSONL (FR26–FR28)
            └── mod.rs           # LogManager : append(), load_recent()
```

### Architectural Boundaries

**Boundary 1 — IPC Rust ↔ Svelte :**
La frontière principale de l'application. Toute communication passe par :
- `invoke()` : actions UI → Rust (commands/)
- `emit()` / `listen()` : état Rust → UI (events définis dans pipeline_engine et clipboard_monitor)
Aucune logique métier dans le frontend — le Svelte ne fait que présenter l'état et déclencher des commandes.

**Boundary 2 — Pipeline Engine ↔ Module Loader :**
Le pipeline engine orchestre l'exécution et délègue chaque étape au module loader.
Interface : `ModuleLoader::execute(input: &str, module_name: &str, config: &Value) -> Result<String, String>`
Le module loader est la seule couche qui touche à boa.

**Boundary 3 — Clipboard Monitor ↔ Pipeline Engine :**
Le monitor détecte les événements de copie et notifie le pipeline engine via un channel tokio.
Interface : `mpsc::Sender<ClipboardEvent>` — découplage total entre détection et traitement.

**Boundary 4 — Config Manager ↔ Pipeline Engine :**
Le config manager maintient la configuration en mémoire et notifie le pipeline engine des changements via `notify`.
Interface : `Arc<RwLock<AppConfig>>` partagé — le pipeline engine lit toujours la config courante.

### Requirements to Structure Mapping

| FR | Fichier(s) principal(aux) |
|---|---|
| FR1 — Interception clipboard | `clipboard_monitor/mod.rs` + `linux/macos/windows.rs` |
| FR2 — Stockage original en RAM | `commands/clipboard.rs` : `Mutex<Option<String>>` |
| FR3/FR4 — Ctrl+V+V recall | `commands/clipboard.rs` + `main.rs` (global shortcut) |
| FR5–FR7 — Pipeline engine + modes | `pipeline_engine/mod.rs` |
| FR8 — Async en cours au Ctrl+V | `pipeline_engine/mod.rs` : gestion état pending |
| FR9 — Fallback sur échec | `pipeline_engine/mod.rs` : Result chain |
| FR10/FR11 — Module manifest + contrat | `config_manager/module_manifest.rs` + `module_loader/mod.rs` |
| FR12 — Installation module externe | `module_loader/mod.rs` : chargement depuis `app_data_dir/modules/` |
| FR13 — Modules builtin | `module_loader/builtin/` (trim, uppercase, fetch_youtube_title) |
| FR14–FR17 — Playground + step debugger | `src/sections/PlaygroundSection.svelte` + `pipeline-run.svelte.ts` |
| FR18–FR21 — Config YAML + hot-reload | `config_manager/mod.rs` + `src/sections/PipelinesSection.svelte` |
| FR22–FR25 — System tray + daemon | `main.rs` (tray setup, window management) |
| FR26–FR28 — Log + métriques | `log_manager/mod.rs` + `src/sections/LogSection.svelte` |
| FR29 — Onboarding | `src/lib/components/Onboarding.svelte` |

### Integration Points

**Data Flow — Clipboard Full-Auto :**
```
OS clipboard event
  → clipboard_monitor (OS-specific)
  → mpsc channel
  → pipeline_engine::run()
    → trigger.rs : évalue content_regex / recipient_app_regex
    → module_loader::execute() × N modules (boa)
      → emit('pipeline:step-complete') après chaque module
    → clipboard write (transformed result)
    → emit('pipeline:complete')
    → log_manager::append()
  → tray icon : state update
```

**Data Flow — Playground manual run :**
```
UI → invoke('run_pipeline', { input, pipelineName })
  → pipeline_engine::run_interactive()
    → module_loader::execute() × N modules
      → emit('pipeline:step-complete') après chaque module
    → return Ok(final_output)
  → UI updates FlowNodes via listen('pipeline:step-complete')
```

**External Integrations :**
- `fetch-youtube-title` module : appel HTTP vers YouTube oEmbed API (réseau requis, géré dans boa avec timeout)
- GitHub : distribution des releases via GitHub Actions upload
- Modules externes : téléchargés manuellement par l'utilisateur depuis GitHub, placés dans `app_data_dir/modules/`

### File Organization Patterns

**Configuration files (runtime, écrits par l'utilisateur) :**
```
app_config_dir()/ultraclipboard/
├── pipelines/
│   ├── youtube-to-obsidian.yaml
│   └── normalize-number.yaml
└── settings.yaml
```

**Data files (écrits par l'app) :**
```
app_data_dir()/ultraclipboard/
├── modules/
│   └── ultraclipboard-module-normalize-number/
│       ├── module.yaml
│       └── index.js
└── logs/
    └── execution.jsonl
```

**GitHub Actions :**
```
.github/workflows/release.yml
  trigger: push tag v*
  jobs:
    - build-linux   → ubuntu-latest  → .AppImage
    - build-macos   → macos-latest   → .dmg
    - build-windows → windows-latest → .exe + .msi
  → upload artifacts to GitHub Release
```

## Architecture Validation Results

### Coherence Validation ✅

**Decision Compatibility :**
Toutes les décisions technologiques sont compatibles : Tauri 2 + Svelte 5 + Vite + TypeScript
forment la stack officielle create-tauri-app. Les crates Rust (boa, tokio, serde_yaml, notify)
sont indépendantes et sans conflit. shadcn-svelte supporte Svelte 5 nativement.

**Pattern Consistency :**
Les patterns de nommage (serde rename_all camelCase, event domain:action) sont cohérents
avec les choix technologiques. La séparation invoke/events correspond aux capacités Tauri 2.
Les Svelte 5 Runes sont le système natif du starter sélectionné.

**Correction appliquée — $effect module-level :**
Le pattern `$effect()` au niveau module dans les fichiers `.svelte.ts` requiert `$effect.root()`
en Svelte 5 (contrairement à `$effect()` qui ne fonctionne qu'à l'intérieur d'un composant).
Pattern corrigé pour les stores :

```typescript
// daemon.svelte.ts — pattern correct Svelte 5
import { listen } from '@tauri-apps/api/event'

let state = $state<'idle' | 'processing' | 'error'>('idle')

export function initDaemonStore() {
  $effect.root(() => {
    const unlisten = listen<{ state: typeof state }>('daemon:state-changed', (event) => {
      state = event.payload.state
    })
    return () => { unlisten.then(fn => fn()) }
  })
}

export { state as daemonState }
// initDaemonStore() appelé une fois depuis App.svelte au montage
```

**Structure Alignment :**
La structure de projet reflète exactement les boundaries architecturaux :
`clipboard_monitor/` ↔ Pipeline Engine via mpsc, `module_loader/` comme seule couche boa,
`commands/` comme seule exposition IPC. Les boundaries sont bien physiquement séparés.

### Requirements Coverage Validation ✅

**Functional Requirements (29/29 couverts) :**

| Catégorie | FRs | Couverture |
|---|---|---|
| Clipboard Management | FR1–FR4 | ✅ clipboard_monitor + commands/clipboard.rs |
| Pipeline Engine | FR5–FR9 | ✅ pipeline_engine/ + state machine |
| Module System | FR10–FR13 | ✅ module_loader/ + builtin/ |
| Playground | FR14–FR17 | ✅ PlaygroundSection + pipeline-run.svelte.ts |
| Configuration | FR18–FR21 | ✅ config_manager/ + notify hot-reload |
| System Integration | FR22–FR25 | ✅ main.rs tray + global shortcuts |
| Observability | FR26–FR28 | ✅ log_manager/ + JSONL |
| Onboarding | FR29 | ✅ Onboarding.svelte (DriverJS) |

**Non-Functional Requirements (9/9 couverts) :**

| NFR | Mécanisme architectural |
|---|---|
| NFR1 — Idle CPU/RAM minimal | Clipboard monitor event-driven (pas de poll loop) |
| NFR2 — Sync latence imperceptible | Pipeline engine synchrone dans thread tokio dédié |
| NFR3 — Cold start < 3s | Modules chargés on-demand, pas de heavy init |
| NFR4 — Contenu jamais sur disque | `Mutex<Option<String>>` RAM-only, lifetime bounded |
| NFR5 — Log = métadonnées only | JSONL structure fixe sans payload, documenté dans patterns |
| NFR6 — Sandbox JS | `sandbox.rs` avec contexte boa isolé |
| NFR7 — Exception module ≠ crash | `Result<T,String>` chain, panic isolation |
| NFR8 — Crash daemon → tray signal | State machine propagée au tray via events |
| NFR9 — Timeout module configurable | `tokio::time::timeout` dans sandbox.rs |

### Implementation Readiness Validation ✅

**Decision Completeness :** Toutes les décisions critiques sont documentées avec rationale.
Les versions sont spécifiées (Tauri 2.x, Svelte 5). Les crates Rust sont nommées.

**Structure Completeness :** Arbre de fichiers complet, spécifique, sans placeholder.
Chaque FR est mappé à un fichier source précis.

**Pattern Completeness :** 7 zones de conflit potentiel identifiées et adressées.
Exemples de code concrets pour les patterns clés (IPC, stores, error handling).
Anti-patterns explicites documentés.

### Gap Analysis Results

**Gaps critiques :** Aucun.

**Gaps importants :**
- La compatibilité exacte de `boa` avec `async/await` JS et les Promises reste à valider
  par prototype — documenté comme risque n°2, première story après clipboard interception.
- Le mécanisme exact d'interception clipboard sur Linux Wayland (vs X11) est à déterminer
  au moment du prototype — les deux sont listés dans `linux.rs` mais l'approche diffère.

**Gaps nice-to-have :**
- Version de `boa` à épingler dans Cargo.toml non spécifiée (à déterminer au setup)
- Stratégie de test unitaire Rust peu détaillée — acceptable pour un MVP solo

### Architecture Completeness Checklist

**✅ Requirements Analysis**
- [x] 29 FRs analysés avec implications architecturales
- [x] 9 NFRs mappés à des mécanismes concrets
- [x] Complexité évaluée (Basse–Moyenne, 9 composants)
- [x] Concerns transversaux identifiés (state machine, error containment, async, platform, security)

**✅ Architectural Decisions**
- [x] Stack complète décidée (Tauri 2 + Svelte 5 + TS + Vite)
- [x] Persistance locale définie (OS dirs, YAML, JSONL, hybrid modules)
- [x] IPC schéma défini (invoke vs events, event schema complet)
- [x] Crates Rust sélectionnées
- [x] CI/CD défini (GitHub Actions, 3 OS runners)
- [x] Versioning défini (SemVer, Keep a Changelog)

**✅ Implementation Patterns**
- [x] 7 zones de conflit adressées
- [x] Conventions de nommage complètes (Rust, TypeScript, YAML, events)
- [x] Patterns IPC avec exemples de code
- [x] Patterns stores Svelte 5 avec correction $effect.root()
- [x] Pattern error handling avec séparation récupérable/fatal
- [x] Anti-patterns explicites documentés

**✅ Project Structure**
- [x] Arbre complet avec tous les fichiers nommés
- [x] 4 boundaries architecturaux définis avec interfaces
- [x] Tous les 29 FRs mappés à des fichiers précis
- [x] Data flows documentés (full-auto + playground)
- [x] File organization patterns (config, data, CI)

### Architecture Readiness Assessment

**Overall Status : READY FOR IMPLEMENTATION**

**Confidence Level : High**

**Points forts de cette architecture :**
- Séparation claire des responsabilités (un module Rust = une responsabilité)
- Boundaries IPC explicites qui empêchent la logique métier de fuiter dans le frontend
- Risques techniques identifiés et séquencés en priorité d'implémentation
- Patterns de consistance suffisamment précis pour guider des agents AI indépendants
- Zéro over-engineering — chaque décision est justifiée par un FR ou NFR concret

**Axes d'évolution future (post-MVP) :**
- SQLite si le log JSONL devient insuffisant (Phase 3)
- Sandboxing boa plus granulaire avec déclaration explicite des permissions réseau (Phase 3)
- `tauri-plugin-updater` pour auto-update (Phase 2)
- Index communautaire de modules (Phase 3)

### Implementation Handoff

**Directives pour les agents AI :**
- Suivre exactement les conventions de nommage du document (section Implementation Patterns)
- Ne jamais écrire de logique métier dans les composants Svelte — tout passe par invoke() ou les stores
- Le `sandbox.rs` est la seule interface avec boa — aucun autre fichier ne doit instancier boa
- Commencer par les prototypes (clipboard interception, boa bridge) avant toute UI

**Première priorité d'implémentation :**
```bash
npm create tauri-app@latest ultraclipboard -- --template svelte-ts
```
Puis : prototype `clipboard_monitor/` sur Linux (plateforme auteur) avant d'étendre aux autres OS.
