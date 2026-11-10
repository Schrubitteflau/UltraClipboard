# Journal de décisions — UltraClipboard v1

Spec : [Schrubitteflau/UltraClipboard#1](https://github.com/Schrubitteflau/UltraClipboard/issues/1)
Session de grilling : 2026-11-09 → 2026-11-10

## Démarche et périmètre

### D1: Abandonner BMAD, trier ses livrables

- **Asked**: reprendre le projet cadré avec BMAD (brainstorming, brief, PRD, architecture, epics, UX)
- **Answer**: "j'ai pas envie de réutiliser BMAD parce que je trouve ça trop lourd. C'était bien à l'époque parce que je testais et le brainstorming est quand même vachement cool, mais il y a peut-être des trucs à prendre et à laisser parmi toutes les notes"
- **Decision**: on repart sans BMAD ; ses livrables servent de matière, pas de référence.
- **Spec**: Solution, Out of Scope

### D2: Vision : petit outil simple qui tourne en fond

- **Asked**: (spontané) la vision du produit
- **Answer**: "c'est pas de faire un truc trop compliqué, en fait. C'est vraiment d'avoir un petit logiciel qui tourne en fond et qui permet de transformer ton clipboard et que ça marche bien. Pas la peine de faire un truc trop compliqué avec une interface trop compliquée. Je veux un truc simple et efficace"
- **Decision**: simplicité avant tout ; l'outil vit en arrière-plan, l'interface reste minimale.
- **Spec**: Problem Statement, Solution

### D3: Espanso comme inspiration technique

- **Asked**: (spontané) comment faire marcher le presse-papier sous Linux
- **Answer**: "une inspiration, ça va être le logiciel Espanso, qui fonctionne bien pour tout ce qui est contrôle de clipboard. Donc à voir comment c'est foutu"
- **Decision**: étudier Espanso avant de choisir la technique ; on n'en reprend pas l'approche evdev/setcap (cf. D11).
- **Spec**: not in spec (ADR 0001)

### D4: Ubuntu 26.04 / GNOME / Wayland uniquement

- **Asked**: Q1 — viser seulement Ubuntu 26.04 / GNOME / Wayland pour la v1, sans couche multiplateforme anticipée
- **Answer**: "Q1 : oui" ; plus tôt : "qui marche sur Ubuntu, déjà Ubuntu 26, donc qui marche sur ma machine. Déjà, c'est très bien si c'est comme ça"
- **Decision**: cible unique GNOME / Wayland sur Ubuntu 26.04 ; pas d'abstraction multiplateforme.
- **Spec**: Out of Scope

### D5: Travailler par spec, tickets et implémentation

- **Asked**: Claude avançait étape par étape sans spec ni tickets, et proposait des fiches légères `docs/steps/`
- **Answer**: "du coup tu fais tout à la volée sans ticket ni spec, c'est voulu ?" puis "perso j'ai aucun problème avec $to-spec et $to-tickets puis $implement , ça reste 10x plus léger que BMAD"
- **Decision**: le travail passe par `/to-spec`, `/end-grilling`, `/to-tickets`, puis `/ship` ou `/implement` ; suivi dans les issues GitHub.
- **Spec**: not in spec (processus)

## Déclenchement et choix de la Règle

### D6: Transformation automatique à la copie, pour laisser le temps aux Transformations lentes

- **Asked**: Q2 (1er tour) — auto à la copie, à la demande par raccourci, ou les deux
- **Answer**: "l'avantage que ce soit automatique à la copie, c'est que si on a quelque chose qui n'est pas instantané, typiquement l'URL YouTube, bah en fait, il faut aller fetch le titre et le nom de la chaîne […] si le CTRL-C et CTRL-V est fait trop rapidement, ça aura pas le temps de finir. […] Si je fais CTRL-C... ça se lance dans tous les cas. Et lorsque je fais CTRL-V, à ce moment-là, ça va aller récupérer le fruit d'une transformation éventuelle ?"
- **Decision**: le déclenchement se fait à la copie ; l'idée « Ctrl+V récupère le résultat » (collage différé) a été étudiée et jugée infaisable depuis une extension (cf. D12).
- **Spec**: User Stories 1, 6, 9 ; Out of Scope (collage différé)

### D7: La Règle se choisit selon le contenu copié

- **Asked**: Q3 (1er tour) — un raccourci par Transformation, un sélecteur, ou la première Règle qui correspond au contenu
- **Answer**: "en théorie, la transformation est censée être choisie selon le type de contenu. Si je copie une URL YouTube, bah évidemment que ça va appeler le raccourci de transformation YouTube […] Alors, la question se pose si on en a deux transformateurs qui se overlappent."
- **Decision**: sélection par motif sur le contenu ; le chevauchement est tranché en D16.
- **Spec**: User Stories 1–4

### D8: Approche « raccourci GNOME + commande ponctuelle » issue de l'étude Espanso

- **Asked**: suite à l'étude d'Espanso, laisser GNOME porter le raccourci et lancer une commande ponctuelle qui lit, transforme et réécrit le presse-papier
- **Answer**: "concernant ce que tu as dit, suite à l'étude de Espanso, je suis... Ok, je te suis, on fait comme ça."
- **Decision**: pas de lecture du clavier façon Espanso (evdev/setcap).
- **Spec**: not in spec — superseded by D11 (le déclenchement passe par une extension GNOME Shell, automatique à la copie)

## Architecture et technique

### D9: Une petite fenêtre en lecture seule, en Svelte + shadcn

- **Asked**: Q4 (1er tour) — abandonner toute fenêtre (service + YAML + notifications seulement)
- **Answer**: "Non. […] J'aime l'idée d'avoir une petite interface, même si c'est un truc hyper, hyper simpliste qui permet pas de faire grand-chose. […] une interface qui affiche l'historique, par exemple des transformations, ou même encore plus simple, qui affiche la liste des transformeurs qui sont enregistrés […] une interface qui permet de consulter de la donnée et pas de la modifier […] on oublie le debugger, l'onboarding […] si on pouvait quand même faire un truc portable, donc par exemple avec du Svelte et ChatCN, ce serait cool."
- **Decision**: une fenêtre en lecture seule (Règles/Pipelines et historique) ; pas de Playground, debugger ni onboarding ; techno web portable.
- **Spec**: User Stories 40–44 ; Implementation Decisions (Appli Tauri, icône, fenêtre) ; Out of Scope

### D10: Tauri 2 + Svelte 5 + shadcn-svelte, backend Rust, icône avec menu

- **Asked**: Q4 (2e tour) — fenêtre Tauri + Svelte 5 + shadcn-svelte, backend Rust, une seule appli qui tourne en continu, ouverte via l'icône
- **Answer**: "Q4 : oui"
- **Decision**: un seul processus Tauri 2 (Rust) héberge le moteur ; fenêtre Svelte 5 + shadcn-svelte ; icône dans la barre du haut.
- **Spec**: Implementation Decisions (Découpage en modules ; Appli Tauri, icône, fenêtre)

### D11: Extension GNOME Shell comme capteur, déclenchement automatique

- **Asked**: Q2 (2e tour) — accepter une extension GNOME Shell (capteur des copies, écriture du presse-papier) parlant à l'appli ; transformation à la copie, notification à la fin, coller trop tôt donne l'Original ; raccourci manuel en secours (« réappliquer » ou « forcer tel Pipeline »)
- **Answer**: "Q2 : ok" ; puis, après l'étude de faisabilité : "je suis d'accord"
- **Decision**: extension GNOME Shell mince + appli via D-Bus ; transformation automatique à la copie ; coller avant la fin donne l'Original. Le raccourci manuel de secours fait partie de ce qui a été accepté.
- **Spec**: Implementation Decisions (Extension GNOME Shell, Contrat D-Bus) ; ADR 0001 ; le raccourci de secours est en Out of Scope (voir écart G1)

### D12: Pas de Ctrl+V+V ; raccourci dédié pour restaurer l'Original

- **Asked**: Q7 — un raccourci dédié (ex. `Super+Shift+V`) remet l'Original dans le presse-papier, sans simulation de touches ; Ctrl+V+V jugé irréaliste
- **Answer**: 1er tour : "je comprends pas en quoi le ctrl+V+V n'est pas réaliste. Parce que justement, moi, je réfléchis du point de vue utilisateur. On fait contrôle V, on a encore les deux doigts sur contrôle V […] il suffit de rappuyer une fois […] Est-ce que c'est pas réaliste techniquement?" ; après explication (Wayland ne laisse pas voir le 2e V, le 1er V a déjà collé, Ctrl+V+V veut déjà dire « coller deux fois ») : "Q7 : oui"
- **Decision**: raccourci global dédié « restaurer l'Original », qui ne touche qu'au presse-papier ; pas d'interception de Ctrl+V.
- **Spec**: User Stories 30–33 ; Out of Scope (Ctrl+V+V)

### D13: Transformations extensibles par commande externe

- **Asked**: Q5 — Transformations intégrées + `command` (stdin/stdout), modules JS embarqués (BMAD), ou intégrées seulement
- **Answer**: "Q5 : a"
- **Decision**: intégrées + `command` ; pas de moteur JS, de manifest ni d'écosystème.
- **Spec**: Implementation Decisions (Transformations) ; ADR 0002 ; Out of Scope

### D14: Transformations de la v1

- **Asked**: Q6 — `youtube-markdown`, `trim`, `upper`/`lower`, `command`
- **Answer**: "Q6 : oui ces 4 là"
- **Decision**: ces quatre Transformations, rien d'autre en v1.
- **Spec**: User Stories 14–24

### D15: Vocabulaire du domaine

- **Asked**: Q8 — Transformation, Pipeline, Règle, Original
- **Answer**: "Q8 : oui on adopte ces termes"
- **Decision**: ces termes sont canoniques (GLOSSARY.md) ; éviter module, plugin, transformer, trigger.
- **Spec**: Solution (renvoi au glossaire)

### D16: La première Règle qui correspond gagne

- **Asked**: Q3 (2e tour) — première Règle dans l'ordre du YAML, une seule s'applique, aucune = rien
- **Answer**: "Q3 : oui"
- **Decision**: ordre du fichier significatif ; une seule Règle par copie ; sans correspondance, presse-papier intact.
- **Spec**: User Stories 2–4

### D17: Historique en mémoire, 50 entrées, avec le contenu

- **Asked**: Q9 — 50 dernières transformations, en mémoire seulement, avec Original et résultat
- **Answer**: "Q9 : oui"
- **Decision**: historique en RAM, plafonné à 50, perdu au redémarrage ; rien du presse-papier sur disque.
- **Spec**: User Stories 42–43

### D18: YouTube via oEmbed, avec l'URL de la chaîne

- **Asked**: Q10 — oEmbed (titre + chaîne, sans clé), format réglable dans le YAML
- **Answer**: "Q10 : ok mais ça renvoie aussi l'URL de la chaîne ?" (oui : `author_url`)
- **Decision**: oEmbed ; gabarit avec `{title}`, `{url}`, `{author_name}`, `{author_url}`, `{thumbnail_url}`.
- **Spec**: User Stories 16–19 ; Implementation Decisions (Transformations)

## Notifications

### D19: Notifications désactivables

- **Asked**: (spontané, en validant D11)
- **Answer**: "faudrait juste que l'utilisateur puisse choisir de désactiver les notifications car je sais que ça peut vite être relou"
- **Decision**: les notifications doivent pouvoir être réduites ou coupées.
- **Spec**: User Stories 36–37

### D20: Trois niveaux, défaut `all`, bascule dans le menu

- **Asked**: Q12 — `all` / `errors` / `off` dans le YAML (défaut `all`) + bascule « Notifications on/off » dans le menu, valable jusqu'au redémarrage
- **Answer**: "oui à tout"
- **Decision**: comme proposé.
- **Spec**: User Stories 34–37 ; Implementation Decisions (Interface du moteur)

## Configuration

### D21: Un fichier YAML unique, rechargé à chaud

- **Asked**: Q11 — `~/.config/ultraclipboard/config.yaml`, `pipelines` nommés + `rules` ordonnées ; YAML invalide → ancienne config gardée + notification
- **Answer**: "oui à tout"
- **Decision**: comme proposé.
- **Spec**: User Stories 25, 27–28 ; Implementation Decisions (Configuration)

### D22: Versionner la config

- **Asked**: (spontané, en validant Q11–Q20)
- **Answer**: "ajoute juste du versioning dans le fichier de config style \"config_version: 1\""
- **Decision**: `config_version: 1` obligatoire ; version absente ou inconnue refusée.
- **Spec**: User Story 26 ; Implementation Decisions (Configuration)

## Comportement du moteur

### D23: `command` : liste d'arguments, stdin/stdout, échec et délai

- **Asked**: Q13 — liste d'arguments sans shell ; stdin → stdout ; code ≠ 0 ou délai (10 s, `timeout`) = échec ; stderr dans l'historique
- **Answer**: "oui à tout"
- **Decision**: comme proposé.
- **Spec**: User Stories 20–24 ; Implementation Decisions (Transformations)

### D24: Pipeline tout ou rien

- **Asked**: Q14 — une étape qui échoue arrête tout ; presse-papier intact ; jamais de résultat partiel
- **Answer**: "oui à tout"
- **Decision**: comme proposé.
- **Spec**: User Story 11

### D25: La dernière copie gagne ; on ne retraite pas nos propres écritures

- **Asked**: Q15 — une nouvelle copie annule le Pipeline en cours ; ce qu'UltraClipboard écrit n'est pas retraité
- **Answer**: "oui à tout"
- **Decision**: comme proposé.
- **Spec**: User Stories 7, 12, 31

### D26: Pause depuis le menu

- **Asked**: Q16 — « Mettre en pause / Reprendre » dans le menu ; la restauration de l'Original marche toujours en pause
- **Answer**: "oui à tout"
- **Decision**: comme proposé.
- **Spec**: User Stories 32, 39

## Organisation, livraison, documentation

### D27: Monorepo et installation locale

- **Asked**: Q17 — `app/` + `extension/` + `install.sh` (compile, installe l'extension en local, démarrage auto) ; pas de publication sur extensions.gnome.org
- **Answer**: "oui à tout"
- **Decision**: comme proposé.
- **Spec**: User Stories 45–47 ; Implementation Decisions (Installation) ; Out of Scope

### D28: Tranche verticale d'abord, puis TDD sur le moteur

- **Asked**: Q18 — ordre : tranche verticale (`upper` en dur), config/Règles, `youtube` + annulation, notifications/Original, fenêtre/icône ; TDD sur le moteur
- **Answer**: "oui à tout" ; puis "Oui, étape 1" et, après test réel : "ok ça marche"
- **Decision**: la tranche verticale est faite et validée sur la machine ; la suite suit cet ordre, en TDD.
- **Spec**: Testing Decisions ; Further Notes

### D29: Supprimer les livrables BMAD

- **Asked**: Q19 — supprimer `_bmad-output/` et `INITIAL_PROMPT.md` (gardés dans l'historique git)
- **Answer**: "oui à tout" (l'utilisateur a lancé lui-même la suppression)
- **Decision**: supprimés.
- **Spec**: not in spec (ménage du repo)

### D30: Deux ADR

- **Asked**: Q20 — ADR 0001 (extension GNOME Shell comme capteur) et 0002 (commandes externes plutôt que modules JS)
- **Answer**: "oui à tout"
- **Decision**: écrits dans `docs/adr/`.
- **Spec**: Solution (renvoi aux ADR)

### D31: Commits et push

- **Asked**: commiter l'étape 1 ; `.agents/`
- **Answer**: "commite et commite également .agents/ dans un autre commit" puis "Oui, pousse"
- **Decision**: les skills d'agent sont versionnés dans un commit séparé du code.
- **Spec**: not in spec (processus)

### D32: Seams de test

- **Asked**: seam principal = API publique du moteur (vraies commandes, faux serveur oEmbed local) ; seam fin = contrat D-Bus sur bus privé ; extension, icône et fenêtre vérifiées à la main ; notifications envoyées par l'appli
- **Answer**: "validé"
- **Decision**: comme proposé.
- **Spec**: Testing Decisions

## Revue des écarts (/end-grilling)

### D33: Raccourci « réappliquer » comme secours manuel

- **Asked**: G1 — la spec excluait le secours manuel accepté en D11 ; proposition : un seul raccourci `Super+Shift+R` qui réapplique les Règles au presse-papier actuel, « forcer tel Pipeline » restant hors périmètre
- **Answer**: "je valide tout"
- **Decision**: raccourci « réappliquer » (méthode D-Bus `Reapply`, porté par l'extension) ; il fonctionne en pause et ignore l'anti-boucle, puisque c'est une action explicite ; « forcer tel Pipeline » hors périmètre.
- **Spec**: User Story 49 ; Implementation Decisions (Interface du moteur, Contrat D-Bus, Extension GNOME Shell) ; Out of Scope

### D34: `Super+Shift+V` n'est pas un choix de l'agent

- **Asked**: G2 — retirer `Super+Shift+V` de la liste des choix faits sans l'utilisateur (il a été accepté en Q7)
- **Answer**: "je valide tout"
- **Decision**: corrigé dans Further Notes.
- **Spec**: Further Notes

### D35: Les choix proposés par l'agent sont validés

- **Asked**: confirmer ou changer : retours à la ligne finaux retirés de la sortie de `command` ; `~/` développé ; gabarit `youtube` par défaut `[{title}]({url})` ; config d'exemple créée au premier lancement ; restauration sans Original = rien ; entrées « annulé » dans l'historique ; notifications envoyées par l'appli ; méthode D-Bus `SetPaused`
- **Answer**: "je valide tout"
- **Decision**: les huit choix sont des décisions de l'utilisateur.
- **Spec**: Further Notes ; Implementation Decisions

## Découpage en tickets (/to-tickets)

### D36: Config invalide au premier lancement : démarrer sans Règle

- **Asked**: config invalide au tout premier lancement (aucune ancienne config à garder) : démarrer sans aucune Règle (rien n'est transformé, erreur notifiée et visible dans la fenêtre, chargée dès que le fichier est corrigé), ou refuser de démarrer (boucle de redémarrages avec `Restart=on-failure`)
- **Answer**: "je suis d'accord sur la question 2"
- **Decision**: l'appli démarre sans aucune Règle et signale l'erreur ; elle ne refuse jamais de démarrer pour une config invalide.
- **Spec**: not in spec (porté par le ticket #3)

### D37: Découpage révisé après relecture par deux agents

- **Asked**: découpage initial en 12 tickets, puis version révisée en 14 après deux relectures indépendantes (ticket de préparation : moteur asynchrone et `Copied` qui rend la main ; hébergement Tauri tôt ; raccourcis de l'extension regroupés ; dépendances corrigées ; tests de la spec répartis)
- **Answer**: "demande à 2 subagents neutres ce qu'ils pensent de ce découpage" puis "ok on suit ce découpage"
- **Decision**: 14 tickets, sous-issues de #1 (#2 à #15), avec dépendances natives GitHub.
- **Spec**: not in spec (découpage)

### D38: Prérequis système de Tauri installés

- **Asked**: installer maintenant les paquets `-dev` (WebKitGTK, AppIndicator, librsvg, libsoup) pour que le ticket Tauri soit faisable par un agent
- **Answer**: "j'ai installé les paquets système"
- **Decision**: le ticket Tauri (#4) est `ready-for-agent`, pas `ready-for-human`.
- **Spec**: not in spec
