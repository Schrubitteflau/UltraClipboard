---
stepsCompleted: [step-01-init, step-02-discovery, step-02b-vision, step-02c-executive-summary, step-03-success, step-04-journeys, step-05-domain, step-06-innovation, step-07-project-type, step-08-scoping, step-09-functional, step-10-nonfunctional, step-11-polish, step-12-complete]
status: complete
completedAt: '2026-03-06'
inputDocuments:
  - '_bmad-output/planning-artifacts/product-brief-UltraClipboard-2026-03-05.md'
  - '_bmad-output/brainstorming/brainstorming-session-2026-03-05-2126.md'
workflowType: 'prd'
briefCount: 1
researchCount: 0
brainstormingCount: 1
projectDocsCount: 0
classification:
  projectType: desktop_app
  domain: general/productivity
  complexity: low-medium
  projectContext: greenfield
---

# Product Requirements Document - UltraClipboard

**Author:** Grochonnou
**Date:** 2026-03-06

## Executive Summary

UltraClipboard est une application native multiplateforme (Windows, macOS, Linux) qui automatise les transformations de texte répétitives au niveau du clipboard. Elle intercepte le contenu copié, applique des pipelines de transformation composés de modules JavaScript réutilisables, et délivre le résultat prêt à coller — silencieusement, dans le geste Ctrl+C / Ctrl+V existant. Aucune nouvelle habitude. Aucun changement de contexte. Aucune courbe d'apprentissage.

Le produit cible les travailleurs non-techniques qui font un métier sur ordinateur — rédacteurs, comptables, chercheurs, assistants, journalistes — et qui exécutent les mêmes reformatages manuels des dizaines de fois par jour sans réaliser qu'ils pourraient être automatisés. Le critère de succès est unique : l'utilisateur oublie que l'outil est là, et il continue de l'aider.

### What Makes This Special

Trois différenciateurs interdépendants définissent UltraClipboard :

1. **L'invisibilité comme design.** Le produit gagne quand l'utilisateur ne pense plus à lui. Les transformations s'exécutent silencieusement à la copie ; le workflow reste intact. Aucun nouveau geste, aucune UI à ouvrir, aucun mode à activer.

2. **La philosophie Unix comme architecture.** Chaque module fait une chose, bien. Les pipelines composent les modules librement. Cette contrainte est à la fois un principe de design et une garantie de qualité — les modules restent petits, auditables et fiables. Le runtime playground-first rend cela visible : l'utilisateur voit la string à chaque étape du pipeline avant d'activer toute automatisation en arrière-plan.

3. **Un écosystème communautaire sans infrastructure.** Les modules suivent la convention `ultraclipboard-module-*` sur GitHub, installables manuellement depuis un repo. La recherche GitHub *est* le store. Aucun marketplace à maintenir, aucun processus d'approbation, aucune infrastructure à opérer — la convention *est* le canal de distribution.

L'insight fondamental : le clipboard est l'interface universelle entre toutes les applications d'un bureau — utilisé plus de fois par jour que n'importe quelle autre action — et il n'a jamais été augmenté par aucun outil grand public. UltraClipboard se positionne exactement à cette intersection.

## Project Classification

- **Project Type:** Application desktop native, cross-platform (Tauri + Svelte)
- **Domain:** Productivité / travail de la connaissance généraliste
- **Complexity:** Basse–Moyenne (aucune compliance réglementaire ; défis techniques en interception clipboard cross-platform, runtime de modules JS, exécution de pipelines async)
- **Project Context:** Greenfield

## Success Criteria

### User Success

- **Pipeline adoptée et utilisée :** L'utilisateur configure au moins un pipeline et le colle régulièrement sans l'avoir désactivé — la transformation est devenue transparente dans son workflow.
- **Rétention 30 jours :** L'app reste installée et active après 30 jours.
- **Aha moment :** Le premier collage d'un contenu automatiquement transformé sans action supplémentaire de l'utilisateur.

### Business Success

Ce projet n'est pas un business. Les signaux de santé pour un projet open source sont :

- **Premier utilisateur externe :** Quelqu'un hors du réseau personnel installe et utilise l'app — Month 1–2.
- **Premier module communautaire :** Un développeur publie un repo `ultraclipboard-module-*` de façon autonome — Month 3–6. Signal que la convention d'écosystème est comprise et adoptée.
- **100 étoiles GitHub :** Indicateur de découverte organique fonctionnelle — Month 6–12.
- **Issues ouvertes :** Utilisateurs suffisamment investis pour signaler des bugs ou proposer des features. Qualité prime sur quantité.

### Technical Success

Pas de SLA de performance formel pour le MVP — l'objectif est un produit fonctionnel. La philosophie Unix (un module, une chose, bien faite) est une contrainte de design, pas une règle technique : les modules qui s'en écartent sont naturellement moins composables et donc moins adoptés.

### Measurable Outcomes

- **Métrique "temps économisé" :** Agrégat cumulé des transformations effectivement collées (paste confirmé, pas simplement déclenché). Chaque module déclare `time_saved_seconds` dans son manifest.
- **Critère de validation personnel :** L'auteur utilise UltraClipboard quotidiennement pour son workflow YouTube URL → Markdown link Obsidian. Ce critère est le premier et le plus important. Tout le reste est bonus.

## User Journeys

### Journey 1 — Marie, le passage de l'essai à l'invisibilité

**Persona :** Marie, 34 ans, rédactrice indépendante. Elle utilise Obsidian comme second cerveau et passe ses journées à lire, regarder des vidéos, et capturer des notes. Chaque référence YouTube lui prend 4–5 étapes manuelles : copier l'URL, ouvrir l'onglet, trouver le titre, formater en Markdown, coller. Elle le fait 15 fois par jour sans y penser.

**Opening Scene :** Marie tombe sur un post Reddit : *"J'ai automatisé mes reformatages clipboard avec UltraClipboard."* Elle n'y croit pas vraiment, mais elle clique.

**Rising Action :** Elle installe l'app. L'onboarding lui montre en live : elle entre une URL YouTube dans le playground, sélectionne le pipeline `fetch-youtube-title`, et voit apparaître `[Titre de la vidéo](https://youtube.com/...)` à l'output. Elle voit la string intermédiaire après chaque module. Elle active le pipeline en mode daemon avec le pipeline par défaut — aucun YAML écrit.

**Climax :** Deux jours plus tard, dans Obsidian, elle copie une URL YouTube et fait Ctrl+V. Le lien Markdown est là. Elle ne s'y attendait plus. Elle s'arrête une seconde.

**Resolution :** Au bout d'une semaine, elle a oublié qu'elle faisait ce formatage à la main. L'outil est dans sa barre système. Elle ne l'ouvre jamais. Il tourne.

*Capabilities revealed :* onboarding avec playground live, pipeline préconfiguré, daemon background, interception clipboard, mode full auto, step debugger.

---

### Journey 2 — Thomas, la configuration unique qui dure

**Persona :** Thomas, 41 ans, contrôleur de gestion. Il travaille entre un ERP, des exports CSV, et des tableurs Excel. Chaque copier-coller entre systèmes casse les formats : `1.234.567,89` devient `1.234.567` dans Excel US. Il corrige à la main depuis 6 ans.

**Opening Scene :** Un collègue dev lui montre UltraClipboard. Thomas est sceptique — il n'est pas technique. Mais le collègue configure le pipeline `normalize-number` en 3 minutes devant lui.

**Rising Action :** Thomas installe l'app. Il charge `ultraclipboard-module-normalize-number` depuis GitHub. Il ouvre le playground, colle `1.234.567,89`, sélectionne le pipeline, et voit `1234567.89` apparaître. Il configure un trigger : `content_regex: ^\d[\d.,]+$`. Il active le daemon.

**Climax :** Lundi matin, il copie une valeur de l'ERP et colle dans Excel. Le format est correct. Il refait le geste. Toujours correct. Il continue sa journée.

**Resolution :** Thomas n'a plus jamais pensé aux formats numériques. Six mois plus tard, il ne se souvient même plus avoir configuré quoi que ce soit. L'app a disparu dans son workflow.

*Capabilities revealed :* installation de module externe, configuration YAML (trigger `content_regex`), playground de test, daemon background, mode full auto.

---

### Journey 3 — Alex, l'auteur de module communautaire

**Persona :** Alex, 28 ans, développeur frontend. Il utilise UltraClipboard depuis deux semaines et réalise qu'il copie souvent des couleurs hex depuis Figma et doit les convertir en `rgb()` pour son CSS. Le module n'existe pas encore.

**Opening Scene :** Alex consulte le manifest spec dans le README. Le contrat est simple : `async transform(input: string, config: object): Promise<string>`. Il a écrit ce genre de fonction des centaines de fois.

**Rising Action :** En 20 minutes, il écrit `ultraclipboard-module-hex-to-rgb`. Il le teste dans le playground — voit `#1a2b3c` devenir `rgb(26, 43, 60)` étape par étape. Il crée un repo GitHub public avec un `module.yaml` valide et publie. Il poste dans r/webdev : *"J'ai fait un module hex → rgb en 20 min."*

**Climax :** Une semaine plus tard, son repo a 12 stars. Quelqu'un ouvre une issue pour demander le support RGBA. Alex merge la PR d'un contributeur. Le module vit sans lui.

**Resolution :** Alex a publié trois autres modules depuis. Il recommande UltraClipboard à ses collègues : *"C'est simple à étendre, le contrat est propre."* La convention `ultraclipboard-module-*` a fait son travail — GitHub *est* le store.

*Capabilities revealed :* module manifest standardisé (`module.yaml`), contrat `async transform()`, playground comme environnement de test de module, step debugger, convention GitHub.

---

### Journey 4 — Marie, edge case async

**Persona :** Marie, même profil. Un matin, elle copie une URL YouTube comme d'habitude.

**Opening Scene :** L'API YouTube est lente. Le module `fetch-youtube-title` est async — il est toujours en cours quand Marie fait Ctrl+V quelques secondes après.

**Rising Action :** L'icône de l'app dans le system tray change de couleur — signal discret qu'un traitement est en cours. Marie fait quand même Ctrl+V. Le comportement exact (coller l'original en attendant vs. attendre la fin) est à trancher en phase de design. Dans les deux cas, aucun message d'erreur agressif, aucune popup.

**Climax :** Si l'API échoue définitivement : l'icône passe en couleur d'erreur brièvement. Le clipboard original est préservé et collé tel quel. Marie voit son URL brute dans Obsidian.

**Resolution :** Marie fait Ctrl+Z dans Obsidian, puis Ctrl+V+V — colle l'URL originale depuis la mémoire de l'app. Elle consulte le log : `fetch-youtube-title — erreur API — 09:14`. Elle passe ce pipeline en mode "Validation" plutôt que "Full Auto". Elle a tout le contrôle, sans friction.

*Capabilities revealed :* indicateur visuel tray (idle / processing / error), fallback clipboard original, Ctrl+V+V (original recall), traceability log, mode validation vs full auto.

---

### Journey Requirements Summary

| Capability | Journeys |
|---|---|
| Onboarding avec playground live | J1 |
| Step debugger | J1, J3 |
| Pipeline préconfiguré / builtin modules | J1 |
| Daemon background + interception clipboard | J1, J2, J4 |
| Mode full auto | J1, J2, J4 |
| Mode validation | J4 |
| Configuration YAML + triggers | J2 |
| Installation module externe | J2 |
| Module manifest standardisé | J3 |
| Contrat `async transform()` | J3, J4 |
| Indicateur visuel tray (idle / processing / error) | J4 |
| Fallback + Ctrl+V+V (original recall) | J4 |
| Traceability log | J4 |

## Innovation & Novel Patterns

### Detected Innovation Areas

**Clipboard augmentation at the OS level**
Le clipboard est l'interface universelle entre toutes les applications desktop — le geste le plus répété dans une journée de travail — et aucun outil mainstream ne l'augmente. UltraClipboard s'y positionne avec une approche modulaire, composable, et invisible : les transformations s'intègrent dans le geste existant sans en modifier la mécanique.

**Playground-First Runtime Architecture**
Inversion délibérée du modèle habituel (daemon d'abord, UI secondaire). Dans UltraClipboard, le playground *est* le produit principal : un environnement de test et visualisation interactif où l'utilisateur voit sa string transformée étape par étape avant d'activer le daemon. Le daemon background est le second mode du même moteur d'exécution.

**Zero-Infrastructure Ecosystem Distribution**
La convention `ultraclipboard-module-*` sur GitHub constitue à elle seule un canal de distribution communautaire fonctionnel. GitHub Search est le store, les stars sont la curation, les forks sont la contribution. Aucune infrastructure à opérer — la convention *est* l'écosystème.

### Market Context & Competitive Landscape

Les alternatives existantes échouent sur des axes différents :
- **Text expanders (Espanso) :** opèrent sur la frappe, pas sur le clipboard. Incapables de transformations dynamiques (appels API).
- **Plateformes d'automatisation (Make, Zapier, Apple Shortcuts) :** conçues pour des workflows complets, disproportionnées pour des transformations ponctuelles de clipboard.
- **Macros clavier / snippets éditeur :** liés à une app, non portables, non modulaires.

Aucun outil existant ne combine : interception clipboard native + pipeline modulaire composable + écosystème communautaire + expérience playground-first.

### Validation Approach

- **Personnelle :** L'auteur utilise l'outil quotidiennement pour son workflow YouTube → Obsidian.
- **Écosystème :** Le premier module publié par quelqu'un d'extérieur au projet prouve que la convention est adoptable.
- **Playground :** Des utilisateurs non-techniques testent un pipeline et comprennent le résultat sans documentation.

### Risk Mitigation

- **Interception clipboard cross-platform :** Tauri expose des plugins dédiés par OS. Commencer par une plateforme et valider avant d'étendre. Prototype d'interception isolé en premier.
- **Écosystème ne décolle pas :** 2–3 modules builtin utiles d'emblée garantissent la valeur sans dépendance communautaire.
- **Modules async dégradent l'expérience :** Indicateur visuel tray + fallback clipboard original + mode validation comme alternative au full auto.

## Desktop App Specific Requirements

### Technical Stack

- **Tauri + Svelte** — runtime natif léger (non-Electron), UI web compilée, accès aux APIs système via plugins Tauri
- **`boa` (JS engine en Rust)** — exécution des modules JS dans un contexte isolé côté backend Rust, intégration native avec Tauri
- **YAML config** — pipelines et triggers définis en fichiers YAML, hot-reloadables par le backend

### Platform Support

| Plateforme | Support MVP | Format de distribution |
|---|---|---|
| Linux | ✓ | `.AppImage` (GitHub Releases) |
| macOS | ✓ | `.dmg` (GitHub Releases) |
| Windows | ✓ | `.exe` / `.msi` (GitHub Releases) |

Distribution MVP : GitHub Releases uniquement, sans auto-update. `tauri-plugin-updater` disponible pour implémentation post-MVP.

### System Integration

- **Clipboard interception :** Surveillance du presse-papiers système au niveau OS. Déclenche le pipeline correspondant au Ctrl+C selon les triggers configurés.
- **System tray :** Point d'accès principal — icône toujours présente, ouvre le playground au clic. États visuels : idle / processing / error.
- **Raccourcis clavier globaux :** Ctrl+V+V pour coller le contenu clipboard original. Autres raccourcis à définir en phase de design.
- **Démarrage au boot :** Hors MVP — l'utilisateur lance l'app manuellement. Tauri supporte cette feature nativement pour une version ultérieure.

### Implementation Constraints

- Le daemon clipboard doit être non-bloquant et à faible empreinte CPU/mémoire.
- Les fichiers YAML de configuration sont hot-reloadables sans redémarrage.
- Le sandboxing des modules JS (accès filesystem, réseau) est contraint par le runtime `boa`.

## Project Scoping & Phased Development

### MVP Strategy

**Approche :** Problem-solving MVP. L'objectif est un outil que l'auteur utilise quotidiennement. Si le workflow YouTube → Obsidian fonctionne invisiblement, le concept est validé. L'écosystème communautaire vient ensuite.

**Ressources :** Projet 100% solo. Chaque feature justifie son coût par sa valeur directe.

### MVP Feature Set (Phase 1)

- Interception clipboard système (Windows, macOS, Linux)
- Playground avec step debugger (string visible à chaque étape du pipeline)
- "Clipboard comme Négatif" — rechargement du dernier clipboard sans re-copier
- Exécution de pipelines via runtime JS `boa`
- Configuration pipelines & triggers en YAML (`content_regex`, `recipient_app_regex`)
- Module manifest standardisé (`module.yaml`)
- Contrat module : `async transform(input: string, config: object): Promise<string>`
- Mode full auto (transform au Ctrl+C) et mode validation (confirmation au Ctrl+V)
- Indicateur visuel system tray : idle / processing / error
- Ctrl+V+V : coller le contenu clipboard original du dernier Ctrl+C (le Ctrl+Z dans l'app de destination annule le paste transformé ; Ctrl+V+V permet ensuite de recoller l'original)
- Traceability log : timestamp + pipeline + outcome
- Métrique temps économisé basée sur les collages effectifs (`time_saved_seconds` par module)
- Modules builtin : `trim`, `uppercase`, `fetch-youtube-title`
- Distribution GitHub Releases (`.AppImage`, `.dmg`, `.exe`)
- Onboarding guidé (DriverJS) — *première feature à couper si nécessaire ; un README suffit pour le MVP*

### Phase 2 — Growth

- Lancement au boot (Tauri natif)
- Auto-update via `tauri-plugin-updater`
- Onboarding DriverJS (si coupé du MVP)
- Starter packs / profils par type d'utilisateur
- Pattern detection et suggestions de pipelines

### Phase 3 — Expansion

- Cache par input hash (même input + même pipeline = résultat instantané)
- Index ou registre communautaire de modules mis en valeur
- Protocole local : apps tierces exposent leurs propres modules via socket/API locale
- Modules générés par IA à la volée
- Mode CLI / headless

### Risk Mitigation

- **Interception clipboard cross-platform :** Prototype isolé en premier. Commencer par une plateforme (macOS ou Linux) avant d'étendre.
- **Runtime JS `boa` :** `boa` est en développement actif avec couverture ECMAScript partielle. À prototyper tôt pour valider la compatibilité avec le contrat `async transform()`. Les modules builtin sont simples et ne dépendent pas de features ES avancées.
- **Adoption écosystème :** Les modules builtin garantissent la valeur sans dépendance communautaire. Le premier module externe est un milestone, pas un prérequis.
- **Projet solo :** Si une feature résiste trop longtemps, elle passe en Phase 2.

## Functional Requirements

### Clipboard Management

- **FR1 :** Le système intercepte le contenu textuel du clipboard à chaque événement de copie (Ctrl+C)
- **FR2 :** Le système conserve le contenu clipboard original (pré-transformation) du dernier Ctrl+C en mémoire jusqu'au prochain événement de copie
- **FR3 :** L'utilisateur peut rappeler le contenu clipboard original via un raccourci clavier global (Ctrl+V+V)
- **FR4 :** Le système restaure le contenu clipboard original comme valeur active du clipboard sur demande

### Pipeline Engine

- **FR5 :** Le système exécute un pipeline de modules sur le contenu clipboard intercepté
- **FR6 :** Le système exécute des pipelines automatiquement au moment de la copie (mode full auto)
- **FR7 :** Le système demande une confirmation à l'utilisateur avant d'appliquer un pipeline au moment du collage (mode validation)
- **FR8 :** Le système gère l'état où l'utilisateur déclenche un collage pendant qu'un module async est encore en cours d'exécution
- **FR9 :** Le système revient au contenu clipboard original en cas d'échec ou de timeout d'un module

### Module System

- **FR10 :** Les auteurs de modules définissent un module via un manifest standardisé (`module.yaml`) déclarant : `name`, `description`, `tags`, `version`, `time_saved_seconds`, `config_schema`
- **FR11 :** Le système charge et exécute des modules JavaScript conformes au contrat `async transform(input: string, config: object): Promise<string>`
- **FR12 :** Les utilisateurs installent des modules depuis des sources externes (repos GitHub)
- **FR13 :** Le système inclut des modules builtin : `trim`, `uppercase`, `fetch-youtube-title`

### Playground

- **FR14 :** Les utilisateurs saisissent un texte manuellement et exécutent un pipeline contre ce texte dans un environnement interactif
- **FR15 :** Les utilisateurs visualisent la string intermédiaire après chaque étape de module dans un pipeline (step debugger)
- **FR16 :** Les utilisateurs rechargent le dernier contenu clipboard intercepté dans le playground sans re-copier ("Clipboard comme Négatif")
- **FR17 :** Les utilisateurs sélectionnent quel pipeline exécuter dans le playground

### Configuration

- **FR18 :** Les utilisateurs définissent des pipelines comme séquences ordonnées de modules en YAML
- **FR19 :** Les utilisateurs définissent des conditions de déclenchement basées sur le contenu clipboard (`content_regex`)
- **FR20 :** Les utilisateurs définissent des conditions de déclenchement basées sur l'application de destination (`recipient_app_regex`)
- **FR21 :** Le système recharge les modifications de configuration sans redémarrage de l'application

### System Integration

- **FR22 :** L'application s'exécute en permanence comme processus du system tray
- **FR23 :** Les utilisateurs ouvrent le playground en cliquant sur l'icône du system tray
- **FR24 :** Les utilisateurs activent ou désactivent le daemon background depuis le system tray
- **FR25 :** L'icône du system tray communique l'état courant du daemon (idle, processing, error) via des indicateurs visuels distincts

### Observability & Metrics

- **FR26 :** Le système maintient un journal de toutes les exécutions de pipeline avec horodatage, nom du pipeline et résultat (succès ou erreur)
- **FR27 :** Le système cumule le temps économisé basé sur les transformations effectivement collées, en utilisant `time_saved_seconds` du manifest de chaque module
- **FR28 :** Les utilisateurs consultent le journal d'exécution depuis l'application

### Onboarding

- **FR29 :** Les nouveaux utilisateurs sont présentés avec un tour guidé interactif démontrant une transformation de pipeline en direct dans le playground

## Non-Functional Requirements

### Performance

- **NFR1 :** Le daemon clipboard a une empreinte mémoire et CPU négligeable en état idle — imperceptible pour l'utilisateur dans le gestionnaire de tâches système.
- **NFR2 :** Les modules synchrones produisent un résultat sans latence perceptible entre le Ctrl+C et la disponibilité du contenu transformé dans le clipboard.
- **NFR3 :** Le démarrage de l'application (cold start) complète en moins de 3 secondes sur un matériel grand public.

### Security

- **NFR4 :** Le contenu clipboard intercepté n'est jamais écrit sur disque — le contenu original du dernier Ctrl+C est maintenu uniquement en RAM et écrasé au prochain événement de copie.
- **NFR5 :** Le traceability log persiste uniquement les métadonnées d'exécution (timestamp, pipeline name, outcome, temps économisé) — jamais le contenu clipboard.
- **NFR6 :** Les modules JS s'exécutent dans le contexte isolé du runtime `boa`, sans accès direct au filesystem ou aux APIs réseau non déclarées dans le manifest.

### Reliability

- **NFR7 :** Une exception non gérée dans un module n'affecte pas le daemon — le système reste opérationnel et traite les événements clipboard suivants normalement.
- **NFR8 :** Une terminaison inattendue du daemon est signalée via un changement d'état visible dans le system tray.
- **NFR9 :** Les modules async s'exécutent avec un timeout implicite configurable — un module non répondant ne bloque pas indéfiniment le clipboard ou le daemon.
