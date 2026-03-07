---
stepsCompleted: [step-01-validate-prerequisites, step-02-design-epics, step-03-create-stories, step-04-final-validation]
status: complete
completedAt: '2026-03-07'
inputDocuments:
  - '_bmad-output/planning-artifacts/prd.md'
  - '_bmad-output/planning-artifacts/architecture.md'
  - '_bmad-output/planning-artifacts/ux-design-specification.md'
---

# UltraClipboard - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for UltraClipboard, decomposing the requirements from the PRD, UX Design, and Architecture into implementable stories.

## Requirements Inventory

### Functional Requirements

**Clipboard Management**
FR1: Le système intercepte le contenu textuel du clipboard à chaque événement de copie (Ctrl+C)
FR2: Le système conserve le contenu clipboard original (pré-transformation) du dernier Ctrl+C en mémoire jusqu'au prochain événement de copie
FR3: L'utilisateur peut rappeler le contenu clipboard original via un raccourci clavier global (Ctrl+V+V)
FR4: Le système restaure le contenu clipboard original comme valeur active du clipboard sur demande

**Pipeline Engine**
FR5: Le système exécute un pipeline de modules sur le contenu clipboard intercepté
FR6: Le système exécute des pipelines automatiquement au moment de la copie (mode full auto)
FR7: Le système demande une confirmation à l'utilisateur avant d'appliquer un pipeline au moment du collage (mode validation)
FR8: Le système gère l'état où l'utilisateur déclenche un collage pendant qu'un module async est encore en cours d'exécution
FR9: Le système revient au contenu clipboard original en cas d'échec ou de timeout d'un module

**Module System**
FR10: Les auteurs de modules définissent un module via un manifest standardisé (module.yaml) déclarant : name, description, tags, version, time_saved_seconds, config_schema
FR11: Le système charge et exécute des modules JavaScript conformes au contrat async transform(input: string, config: object): Promise<string>
FR12: Les utilisateurs installent des modules depuis des sources externes (repos GitHub)
FR13: Le système inclut des modules builtin : trim, uppercase, fetch-youtube-title

**Playground**
FR14: Les utilisateurs saisissent un texte manuellement et exécutent un pipeline contre ce texte dans un environnement interactif
FR15: Les utilisateurs visualisent la string intermédiaire après chaque étape de module dans un pipeline (step debugger)
FR16: Les utilisateurs rechargent le dernier contenu clipboard intercepté dans le playground sans re-copier ("Clipboard comme Négatif")
FR17: Les utilisateurs sélectionnent quel pipeline exécuter dans le playground

**Configuration**
FR18: Les utilisateurs définissent des pipelines comme séquences ordonnées de modules en YAML
FR19: Les utilisateurs définissent des conditions de déclenchement basées sur le contenu clipboard (content_regex)
FR20: Les utilisateurs définissent des conditions de déclenchement basées sur l'application de destination (recipient_app_regex)
FR21: Le système recharge les modifications de configuration sans redémarrage de l'application

**System Integration**
FR22: L'application s'exécute en permanence comme processus du system tray
FR23: Les utilisateurs ouvrent le playground en cliquant sur l'icône du system tray
FR24: Les utilisateurs activent ou désactivent le daemon background depuis le system tray
FR25: L'icône du system tray communique l'état courant du daemon (idle, processing, error) via des indicateurs visuels distincts

**Observability & Metrics**
FR26: Le système maintient un journal de toutes les exécutions de pipeline avec horodatage, nom du pipeline et résultat (succès ou erreur)
FR27: Le système cumule le temps économisé basé sur les transformations effectivement collées, en utilisant time_saved_seconds du manifest de chaque module
FR28: Les utilisateurs consultent le journal d'exécution depuis l'application

**Onboarding**
FR29: Les nouveaux utilisateurs sont présentés avec un tour guidé interactif démontrant une transformation de pipeline en direct dans le playground

### NonFunctional Requirements

NFR1: Le daemon clipboard a une empreinte mémoire et CPU négligeable en état idle — imperceptible pour l'utilisateur dans le gestionnaire de tâches système.
NFR2: Les modules synchrones produisent un résultat sans latence perceptible entre le Ctrl+C et la disponibilité du contenu transformé dans le clipboard.
NFR3: Le démarrage de l'application (cold start) complète en moins de 3 secondes sur un matériel grand public.
NFR4: Le contenu clipboard intercepté n'est jamais écrit sur disque — le contenu original du dernier Ctrl+C est maintenu uniquement en RAM et écrasé au prochain événement de copie.
NFR5: Le traceability log persiste uniquement les métadonnées d'exécution (timestamp, pipeline name, outcome, temps économisé) — jamais le contenu clipboard.
NFR6: Les modules JS s'exécutent dans le contexte isolé du runtime boa, sans accès direct au filesystem ou aux APIs réseau non déclarées dans le manifest.
NFR7: Une exception non gérée dans un module n'affecte pas le daemon — le système reste opérationnel et traite les événements clipboard suivants normalement.
NFR8: Une terminaison inattendue du daemon est signalée via un changement d'état visible dans le system tray.
NFR9: Les modules async s'exécutent avec un timeout implicite configurable — un module non répondant ne bloque pas indéfiniment le clipboard ou le daemon.

### Additional Requirements

**Architecture — Starter Template (impacts Epic 1 Story 1):**
- Starter: `npm create tauri-app@latest ultraclipboard -- --template svelte-ts` (Tauri 2 + Svelte 5 + TypeScript + Vite)
- Generated structure: `src/` (Svelte), `src-tauri/` (Rust), `package.json`, `vite.config.ts`, `tsconfig.json`

**Architecture — Infrastructure & Deployment:**
- GitHub Actions CI/CD cross-platform on `v*` tag: ubuntu-latest (.AppImage), macos-latest (.dmg), windows-latest (.exe/.msi)
- SemVer versioning with `CHANGELOG.md` (Keep a Changelog format)
- `.github/workflows/release.yml` to create

**Architecture — Priority Prototypes (technical risks):**
- Risk #1: Clipboard interception OS-level prototype — validate before everything else
- Risk #2: boa bridge prototype — validate `async transform()` end-to-end before full implementation

**Architecture — Data & Persistence:**
- YAML config in `app_config_dir()/ultraclipboard/` (hot-reload via `notify` crate)
- JSONL append-only log in `app_data_dir()/ultraclipboard/logs/execution.jsonl` (metadata only)
- External modules in `app_data_dir()/ultraclipboard/modules/{module-name}/`
- Builtin modules compiled into the Rust binary

**Architecture — IPC & State Machine:**
- idle/processing/error state machine must be defined in Rust before UI
- Tauri events: `daemon:state-changed`, `pipeline:step-complete`, `pipeline:complete`, `clipboard:captured`
- invoke() for UI→Rust actions; emit()/listen() for Rust→UI state propagation
- Svelte 5 Runes with `$effect.root()` in stores (no `writable()`)

**UX — Design System & Components:**
- Design system: shadcn-svelte, Slate Dark (bg `#0f172a`, accent indigo `#6366f1`)
- 5 custom components: `AppSidebar`, `FlowNode`, `PipelineFlowCanvas`, `StepOutputPanel`, `LogEntry`
- Minimum window size: 500x400px (configure in `tauri.conf.json`)
- Sidebar collapse below 700px (icons only + tooltips)

**UX — Accessibility & Feedback:**
- WCAG AA accessibility (4.5:1 contrast minimum)
- No blocking modals — errors via discrete visual signals only
- Sync feedback: silent FlowNode state change
- Async feedback: tray animation + Sonner toast 3s auto-dismiss
- Full keyboard navigation (Tab + Enter/Space on FlowNodes)

### FR Coverage Map

FR1: Epic 2 - Interception clipboard OS-level
FR2: Epic 2 - Stockage original en RAM
FR3: Epic 2 - Raccourci global Ctrl+V+V
FR4: Epic 2 - Restauration clipboard original
FR5: Epic 1 - Exécution pipeline dans le playground
FR6: Epic 2 - Mode full auto (transform au Ctrl+C)
FR7: Epic 2 - Mode validation (confirmation au Ctrl+V)
FR8: Epic 2 - Gestion async en cours au Ctrl+V
FR9: Epic 2 - Fallback clipboard original sur échec/timeout
FR10: Epic 1 - Module manifest standardisé (module.yaml)
FR11: Epic 1 - Contrat async transform() via runtime boa
FR12: Epic 3 - Installation module externe depuis GitHub
FR13: Epic 1 - Modules builtin (trim, uppercase, fetch-youtube-title)
FR14: Epic 1 - Playground input manuel
FR15: Epic 1 - Step debugger (string intermédiaire visible)
FR16: Epic 2 - "Clipboard comme Négatif" (reload dernier clipboard)
FR17: Epic 1 - Sélection pipeline dans le playground
FR18: Epic 3 - Définition pipelines en YAML
FR19: Epic 3 - Trigger content_regex
FR20: Epic 3 - Trigger recipient_app_regex
FR21: Epic 3 - Hot-reload config sans redémarrage
FR22: Epic 1 - App comme processus system tray permanent
FR23: Epic 1 - Ouverture playground depuis l'icône tray
FR24: Epic 2 - Toggle daemon depuis le tray
FR25: Epic 2 - États visuels tray (idle/processing/error)
FR26: Epic 4 - Journal d'exécutions (timestamp, pipeline, outcome)
FR27: Epic 4 - Métriques temps économisé cumulé
FR28: Epic 4 - Vue log consultable depuis l'app
FR29: Epic 5 - Tour guidé interactif onboarding

## Epic List

### Epic 1: Core Playground & Project Foundation

Users can open the app, enter text in the playground, select a builtin pipeline, run it, and see the transformation step-by-step in the step debugger. The app runs as a persistent system tray process.

**FRs covered:** FR5, FR10, FR11, FR13, FR14, FR15, FR17, FR22, FR23
**NFRs addressed:** NFR3 (cold start < 3s), NFR6 (boa sandbox isolation), NFR7 (exception ≠ daemon crash), NFR9 (configurable module timeout)
**Architecture notes:** Starter template via `create-tauri-app svelte-ts`; boa bridge prototype (risk #2); shadcn-svelte design system; custom components AppSidebar, FlowNode, PipelineFlowCanvas, StepOutputPanel

### Epic 2: Clipboard Daemon — Automatic Background Transformation

Users can enable the background daemon to automatically transform clipboard content on Ctrl+C, see tray state feedback (idle/processing/error), and recover the original clipboard via Ctrl+V+V when the transformation fails or is unwanted.

**FRs covered:** FR1, FR2, FR3, FR4, FR6, FR7, FR8, FR9, FR16, FR24, FR25
**NFRs addressed:** NFR1 (idle CPU/RAM minimal), NFR2 (sync latency imperceptible), NFR4 (clipboard never to disk), NFR8 (crash → tray signal)
**Architecture notes:** Clipboard interception OS-level prototype (risk #1); Rust state machine idle/processing/error; 3 tray icons; global shortcut Ctrl+V+V via tauri-plugin-global-shortcut

### Epic 3: Custom Pipelines & Module Installation

Users can define custom pipelines in YAML, configure regex-based triggers (by content or destination app), install external modules from GitHub, and have configuration changes hot-reloaded without restarting.

**FRs covered:** FR12, FR18, FR19, FR20, FR21
**Architecture notes:** PipelinesSection.svelte; ModulesSection.svelte; YAML hot-reload via `notify` crate; external module loading from app_data_dir()

### Epic 4: Observability — Execution Log & Time Savings

Users can review the history of all pipeline executions (timestamp, pipeline name, outcome), see their cumulative time saved metric, and understand what succeeded or failed at a glance.

**FRs covered:** FR26, FR27, FR28
**NFRs addressed:** NFR5 (log = metadata only, never clipboard content)
**Architecture notes:** LogSection.svelte; LogEntry component; log_manager JSONL append-only

### Epic 5: Onboarding & Distribution

A new user is guided interactively through their first transformation in the playground. The app is distributed via GitHub Releases across all platforms on every version tag.

**FRs covered:** FR29
**Architecture notes:** DriverJS-style tour; Onboarding.svelte component; GitHub Actions CI/CD release.yml (ubuntu .AppImage, macos .dmg, windows .exe/.msi); CHANGELOG.md

---

## Epic 1: Core Playground & Project Foundation

Users can open the app, enter text in the playground, select a builtin pipeline, run it, and see the transformation step-by-step in the step debugger. The app runs as a persistent system tray process.

### Story 1.1: Project Initialization & App Shell

As a developer,
I want the project initialized with the Tauri + Svelte starter template and a basic app shell with system tray presence,
So that I have a working foundation with navigation structure, minimum window configuration, and shadcn-svelte design system ready for feature development.

**Acceptance Criteria:**

**Given** the developer runs `npm create tauri-app@latest ultraclipboard -- --template svelte-ts`
**When** the app is built and launched
**Then** a window opens with AppSidebar (5 sections: Playground, Pipelines, Modules, Log, Settings) and a system tray icon appears
**And** clicking the tray icon opens/focuses the window (FR22, FR23)
**And** the window enforces a minimum size of 500x400px via `tauri.conf.json`
**And** shadcn-svelte is initialized with the Slate Dark color palette (`--background: #0f172a`, `--primary: #6366f1`)
**And** the sidebar collapses to icons-only below 700px window width

### Story 1.2: boa JS Runtime Prototype — Module Execution Sandbox

As a developer,
I want the boa JS engine integrated in an isolated sandbox with async transform() contract support and configurable timeout,
So that I can confirm modules can be safely loaded and executed before building the full pipeline engine.

**Acceptance Criteria:**

**Given** a JavaScript string `async function transform(input, config) { return input.trim(); }` loaded into the boa sandbox
**When** `sandbox.execute(input, config)` is called with a text input
**Then** the function executes and returns the transformed string (FR11)
**And** the module runs in an isolated boa context with no access to filesystem or Tauri APIs (NFR6)
**And** a panic or JS exception inside the module is caught and returned as `Err(String)` without crashing the process (NFR7)
**And** execution is wrapped with `tokio::time::timeout` — a module exceeding the configured timeout returns `Err("timeout")` (NFR9)

### Story 1.3: Builtin Modules & Module Manifest Parser

As a developer (module author),
I want the three builtin modules (trim, uppercase, fetch-youtube-title) implemented with valid module.yaml manifests parsed by the module loader,
So that the system has working modules available for pipeline execution without requiring external files.

**Acceptance Criteria:**

**Given** the app starts
**When** the module loader initializes
**Then** builtin modules `trim`, `uppercase`, and `fetch-youtube-title` are available without reading from disk (FR13)
**And** each builtin module has a corresponding `module.yaml` manifest declaring `name`, `description`, `tags`, `version`, `time_saved_seconds`, `config_schema` (FR10)
**And** `serde_yaml` correctly parses a `module.yaml` file into a `ModuleManifest` struct
**And** `fetch-youtube-title` fetches the video title via YouTube oEmbed API and returns a Markdown link in the format `[title](url)`, with timeout handling for network failures

### Story 1.4: Pipeline Engine — Sequential Module Execution

As a user,
I want the pipeline engine to execute a sequence of modules on a text input and emit a step-complete event after each module,
So that text can be transformed through a configurable chain of modules with intermediate results visible.

**Acceptance Criteria:**

**Given** a pipeline defined as an ordered list of module names (e.g. `["trim", "uppercase"]`)
**When** `run_pipeline(input, pipeline_name)` is invoked
**Then** modules execute sequentially, each receiving the output of the previous module as input (FR5)
**And** after each module completes, a `pipeline:step-complete` event is emitted with `{ step, module, output, status: 'done' | 'error' }`
**And** if a module returns `Err`, execution stops and `pipeline:complete` is emitted with `{ success: false }`
**And** on success, `pipeline:complete` is emitted with `{ success: true, time_saved_ms }`
**And** cold start of the app completes in under 3 seconds on standard hardware (NFR3)

### Story 1.5: Playground UI — Input, Pipeline Selector & Step Debugger

As a user,
I want to enter text in the playground, select a pipeline, run it, and see the transformation step-by-step via animated flow nodes,
So that I can understand what each module does and validate the transformation before trusting the daemon.

**Acceptance Criteria:**

**Given** the user opens the Playground section
**When** they type text in the input field, select a pipeline from the dropdown, and click Run
**Then** the pipeline executes and `PipelineFlowCanvas` displays one `FlowNode` per module (FR14, FR17)
**And** each FlowNode animates through `idle → running → done` states as modules execute (FR15)
**And** clicking any FlowNode updates `StepOutputPanel` to display that step's output in monospace font
**And** the final output is displayed in the StepOutputPanel after the last module completes
**And** if a module errors, its FlowNode switches to `error` state and subsequent nodes remain `idle`

---

## Epic 2: Clipboard Daemon — Automatic Background Transformation

Users can enable the background daemon to automatically transform clipboard content on Ctrl+C, see tray state feedback (idle/processing/error), and recover the original clipboard via Ctrl+V+V when the transformation fails or is unwanted.

### Story 2.1: Clipboard Interception OS-Level Prototype

As a developer,
I want a working clipboard monitor that detects Ctrl+C events at the OS level and forwards the copied text to the pipeline engine via a tokio channel,
So that I can validate the highest-risk technical component before building the full daemon.

**Acceptance Criteria:**

**Given** the clipboard monitor is running on Linux (primary), macOS, or Windows
**When** the user copies text via Ctrl+C in any application
**Then** the monitor captures the text and sends a `ClipboardEvent` through the `mpsc::Sender` channel
**And** the monitor uses native OS APIs (X11/Wayland, CGEventTap, Win32) with no active polling loop (NFR1)
**And** the daemon's idle CPU/RAM usage is imperceptible in the system task manager (NFR1)
**And** captured clipboard content is never written to disk — stored only in RAM (NFR4)

### Story 2.2: Original Clipboard Storage & Ctrl+V+V Recall

As a user,
I want the system to silently preserve the original clipboard content after each Ctrl+C and let me recall it at any time via Ctrl+V+V,
So that I always have a safety net if a transformation produces an unwanted result.

**Acceptance Criteria:**

**Given** the daemon is active and the user copies text via Ctrl+C
**When** the clipboard monitor captures the event
**Then** the original (pre-transformation) text is stored in a `Mutex<Option<String>>` in RAM (FR2)
**And** the stored original is overwritten on the next Ctrl+C event
**And** when the user presses Ctrl+V+V (registered via `tauri-plugin-global-shortcut`)
**Then** the original text is restored as the active clipboard content (FR3, FR4)
**And** the recall works from any application without requiring the playground window to be open

### Story 2.3: Full Auto Mode — Automatic Clipboard Transformation

As a user,
I want the daemon to automatically replace my clipboard content with the transformed result immediately after Ctrl+C,
So that when I press Ctrl+V, the result is already ready without any additional action.

**Acceptance Criteria:**

**Given** the daemon is active in full auto mode with a pipeline configured
**When** the user copies text that matches the pipeline's trigger
**Then** the pipeline executes and the transformed result replaces the clipboard content (FR6)
**And** synchronous modules complete with no perceptible latency between Ctrl+C and the transformed content being available (NFR2)
**And** the tray icon is the only visual feedback — no popup or notification is shown

### Story 2.4: Validation Mode & Async Handling

As a user,
I want the option to confirm a transformation before it is applied (validation mode), and to always receive a safe fallback if a module is still running when I paste,
So that I have control over transformations and am never left without clipboard content.

**Acceptance Criteria:**

**Given** the daemon is active in validation mode
**When** the user copies text that matches a pipeline
**Then** a Sonner toast appears at paste time asking to confirm the transformation (FR7)
**And** the toast auto-dismisses after 3 seconds with no blocking modal
**Given** a module is still running asynchronously when the user presses Ctrl+V (FR8)
**When** the paste occurs before the module completes
**Then** the original clipboard content is pasted immediately without waiting
**Given** a module fails or times out (FR9)
**When** the pipeline returns an error
**Then** the original clipboard content is preserved and used, not the partial result

### Story 2.5: System Tray States & Daemon Toggle

As a user,
I want the tray icon to clearly reflect the daemon's current state and be able to toggle the daemon on/off from the tray,
So that I can see at a glance whether the daemon is active and disable it instantly if needed.

**Acceptance Criteria:**

**Given** the daemon is running
**When** it is idle, processing an async module, or in an error state
**Then** the tray icon displays the corresponding icon (idle/processing/error) with distinct visual differentiation not based solely on color (FR25)
**And** the state change is propagated via the `daemon:state-changed` event from Rust to the Svelte store
**When** the user clicks "Disable daemon" in the tray menu or the sidebar toggle
**Then** clipboard interception stops and the tray icon reflects the disabled state (FR24)
**And** if the daemon crashes unexpectedly, the tray icon changes to the error state (NFR8)

### Story 2.6: "Clipboard comme Négatif" — Reload Last Captured in Playground

As a user,
I want to reload the last clipboard content captured by the daemon into the playground input with a single click,
So that I can immediately test a transformation on real content without re-copying it.

**Acceptance Criteria:**

**Given** the daemon has captured at least one clipboard event
**When** the user clicks "Reload from clipboard" in the playground
**Then** the last captured text is loaded into the playground input field (FR16)
**And** this action calls `invoke('recall_original')` which reads from the same `Mutex<Option<String>>` used for Ctrl+V+V
**And** if no clipboard content has been captured yet, the button is disabled with a tooltip explaining why

---

## Epic 3: Custom Pipelines & Module Installation

Users can define custom pipelines in YAML, configure regex-based triggers (by content or destination app), install external modules from GitHub, and have configuration changes hot-reloaded without restarting.

### Story 3.1: YAML Pipeline Configuration & Hot-Reload

As a user,
I want to define pipelines as ordered sequences of modules in YAML files and have the app reload them automatically when the files change,
So that I can create and update my pipelines without restarting the application.

**Acceptance Criteria:**

**Given** a YAML file in `app_config_dir()/ultraclipboard/pipelines/` with a valid pipeline definition
**When** the app starts or the file changes on disk
**Then** the pipeline is loaded into memory and available for selection in the playground (FR18)
**And** the `notify` file watcher detects the change and triggers a reload without restarting the app (FR21)
**And** the playground pipeline selector updates to reflect newly loaded or modified pipelines
**And** a YAML parsing error is logged with the file name and error detail — the app continues running with previously valid config

### Story 3.2: Trigger Configuration — content_regex & recipient_app_regex

As a user,
I want to configure regex-based triggers on my pipelines so the daemon only activates them when the clipboard content or destination app matches,
So that transformations only apply in the right context and never unexpectedly.

**Acceptance Criteria:**

**Given** a pipeline YAML with a `content_regex` field (e.g. `^\d[\d.,]+$`)
**When** the daemon intercepts a clipboard event
**Then** the pipeline only executes if the clipboard content matches the regex (FR19)
**Given** a pipeline YAML with a `recipient_app_regex` field (e.g. `excel`)
**When** the daemon intercepts a clipboard event
**Then** the pipeline only executes if the active destination application name matches the regex (FR20)
**And** a clipboard event with no matching trigger leaves the clipboard unchanged
**And** multiple pipelines can define triggers — the first matching pipeline executes

### Story 3.3: External Module Installation & Loading

As a user,
I want to install a module from a GitHub repository by placing its files in the modules directory, and have the app load it automatically,
So that I can extend UltraClipboard with community modules without any app restart.

**Acceptance Criteria:**

**Given** the user places a module folder containing `module.yaml` and `index.js` in `app_data_dir()/ultraclipboard/modules/{module-name}/`
**When** the app starts or the modules directory changes
**Then** the module is loaded and available in the playground pipeline selector and pipeline YAML config (FR12)
**And** the `module.yaml` manifest is parsed and validated — an invalid manifest logs an error and skips the module without crashing
**And** the module's `async transform()` function executes through the same boa sandbox as builtin modules (same isolation, same timeout rules)
**And** `ModulesSection.svelte` lists all installed modules (builtin and external) with name, description, and version from their manifests

---

## Epic 4: Observability — Execution Log & Time Savings

Users can review the history of all pipeline executions (timestamp, pipeline name, outcome), see their cumulative time saved metric, and understand what succeeded or failed at a glance.

### Story 4.1: Execution Log — JSONL Persistence

As a user,
I want every pipeline execution to be recorded with its timestamp, pipeline name, and outcome,
So that I can always trace what happened and when, without any clipboard content being stored.

**Acceptance Criteria:**

**Given** a pipeline execution completes (success or error)
**When** `log_manager::append()` is called
**Then** a JSONL entry is appended to `app_data_dir()/ultraclipboard/logs/execution.jsonl` with `timestamp`, `pipeline_name`, `outcome` (`success` or `error`), and optionally `error` message (FR26)
**And** the log entry never contains clipboard content — only metadata (NFR5)
**And** on success, `time_saved_ms` is included in the entry, computed from the pipeline's modules' `time_saved_seconds` manifest values (FR27)
**And** the log file is append-only — existing entries are never modified or deleted

### Story 4.2: Log View & Time Saved Metrics

As a user,
I want to consult the execution log from within the app and see my cumulative time saved,
So that I can understand my transformation history and feel the tangible value UltraClipboard provides.

**Acceptance Criteria:**

**Given** the user opens the Log section
**When** the log file contains entries
**Then** `LogSection.svelte` displays each entry as a `LogEntry` component showing timestamp, pipeline name, and a success/error badge (FR28)
**And** a cumulative "Time saved" counter is displayed at the top of the section, summing `time_saved_ms` from all successful entries (FR27)
**And** only transformations that were actually pasted (confirmed collage) contribute to the time saved counter
**And** if the log is empty, a neutral empty state message is shown with a CTA to open the Playground

---

## Epic 5: Onboarding & Distribution

A new user is guided interactively through their first transformation in the playground. The app is distributed via GitHub Releases across all platforms on every version tag.

### Story 5.1: Interactive Onboarding Tour

As a new user,
I want to be guided through my first live pipeline transformation in the playground on first launch,
So that I understand what UltraClipboard does and reach the "aha moment" without reading documentation.

**Acceptance Criteria:**

**Given** the app is launched for the first time (no prior config detected)
**When** the playground opens
**Then** a DriverJS-style guided tour starts automatically, highlighting the input field, pipeline selector, and Run button in sequence (FR29)
**And** the input is pre-filled with a YouTube URL example and the `fetch-youtube-title` pipeline is pre-selected
**And** when the user clicks Run, the tour highlights each FlowNode as it animates, then points to the Markdown link output in StepOutputPanel
**And** the tour ends with a prompt to enable the daemon via the sidebar toggle
**And** the tour can be dismissed at any step and re-triggered from the Settings section
**And** once completed or dismissed, the tour does not auto-start on subsequent launches

### Story 5.2: GitHub Actions CI/CD — Cross-Platform Release

As a developer,
I want a GitHub Actions workflow that automatically builds and publishes distributable binaries for all three platforms when a version tag is pushed,
So that users can download and install UltraClipboard from GitHub Releases without any manual build step.

**Acceptance Criteria:**

**Given** a git tag matching `v*` (e.g. `v0.1.0`) is pushed to the repository
**When** the `release.yml` GitHub Actions workflow triggers
**Then** three parallel jobs build the app on `ubuntu-latest` (.AppImage), `macos-latest` (.dmg), and `windows-latest` (.exe and .msi)
**And** all artifacts are uploaded to a GitHub Release named after the tag
**And** a `CHANGELOG.md` following the Keep a Changelog format is present in the repository root
**And** the workflow completes successfully on a clean repository with no manual intervention required
