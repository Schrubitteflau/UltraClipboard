# UltraClipboard

Petit outil qui tourne en fond et transforme automatiquement ce que tu copies.
Tu copies une URL YouTube, tu colles `[Titre](URL) — [Chaîne](URL de la chaîne)`.

Cible : **Ubuntu 26.04, GNOME, Wayland**. Rien d'autre pour l'instant.
Vocabulaire : voir [GLOSSARY.md](GLOSSARY.md). Décisions structurantes : [docs/adr/](docs/adr/).

## Comment ça marche

1. Tu copies (Ctrl+C) comme d'habitude.
2. L'extension GNOME Shell voit la copie et l'envoie à l'appli (D-Bus).
3. L'appli cherche la première **Règle** dont le motif correspond au texte.
   Aucune Règle ? Rien ne se passe.
4. Elle exécute le **Pipeline** associé (Transformations en chaîne).
5. Le résultat remplace le presse-papier, une notification le signale.
   Tu colles (Ctrl+V).

Comportements garantis :

- **Tout ou rien** : si une étape échoue ou dépasse son délai, le presse-papier n'est pas touché.
- **La dernière copie gagne** : une nouvelle copie, texte ou non (image, fichiers), arrête le Pipeline en cours (le programme d'une `command` est tué) ; son résultat est jeté.
- **Pas de boucle** : ce qu'UltraClipboard écrit lui-même n'est jamais retraité.
- **Coller trop tôt** (avant la fin d'une Transformation lente) donne l'Original.
- **Restaurer l'Original** : un raccourci remet le texte d'avant la dernière Transformation.
- **Pause** depuis le menu de l'icône ; la restauration reste disponible.
- **Notifications** : « ✓ <Pipeline> » quand un résultat est dans le presse-papier (la bulle remplace celle du succès précédent) ; « ✗ <Pipeline> » quand un Pipeline échoue, avec l'étape et la cause, ou rend un texte vide (« résultat vide : presse-papier inchangé ») ; « Config invalide » à chaque enregistrement d'une config refusée. Niveau `notifications` de la config (`all` : tout ; `errors` : échecs et config invalide ; `off` : rien), et « Couper / Rétablir les notifications » dans le menu de l'icône jusqu'au redémarrage.

## Configuration

`~/.config/ultraclipboard/config.yaml`, rechargé à chaud. YAML invalide → l'ancienne config est conservée et une erreur est notifiée.

```yaml
config_version: 1
notifications: all          # all | errors | off

pipelines:
  youtube-markdown:
    - youtube:
        template: "[{title}]({url}) — [{author_name}]({author_url})"
  clean:
    - trim
    - command: ["python3", "~/scripts/fix-numbers.py"]
      timeout: 10           # secondes, défaut 10

rules:                      # la première Règle qui correspond gagne
  - match: '^https?://(www\.)?(youtube\.com/watch|youtu\.be/)'
    pipeline: youtube-markdown
```

`config_version` est obligatoire ; une version inconnue est refusée.

### Transformations intégrées

| Nom | Effet |
|---|---|
| `trim` | Retire les espaces en début et fin |
| `upper` / `lower` | Majuscules / minuscules |
| `youtube` | Récupère titre et chaîne via oEmbed, rend le `template` (`{title}`, `{url}`, `{author_name}`, `{author_url}`, `{thumbnail_url}`) |
| `command` | Lance un exécutable (liste d'arguments, sans shell) : texte sur stdin, résultat sur stdout. Code ≠ 0 ou délai dépassé = échec ; stderr visible dans l'historique. Sortie limitée à 8 Mo (au-delà : échec), stderr coupée après 64 Ko. Un processus lancé en arrière-plan doit rediriger ses sorties (`cmd >/dev/null 2>&1 &`), sinon l'étape l'attend jusqu'au délai |

## Interface

- **Icône dans la barre du haut**, menu : Ouvrir, Pause/Reprendre, Couper/Rétablir les notifications (jusqu'au redémarrage), Quitter.
- **Fenêtre en lecture seule** : Règles et Pipelines chargés, historique.
- **Historique** : les 50 dernières transformations (Original, résultat, Pipeline, heure, succès/erreur), **en mémoire seulement**, perdu au redémarrage. Rien du presse-papier n'est écrit sur disque.

## Installation

Prérequis : [Rust](https://rustup.rs), [bun](https://bun.sh) et les bibliothèques système de Tauri :

```sh
sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libayatana-appindicator3-dev librsvg2-dev
```

Puis `./install.sh` (il vérifie ces prérequis avant de compiler).

## Architecture

```
extension/   Extension GNOME Shell (GJS) : voit les copies, écrit le presse-papier,
             porte les raccourcis. Parle à l'appli par D-Bus.
app/         Appli Tauri 2 (un seul processus) :
  engine/        moteur Rust (Règles, Pipelines, Transformations, historique) ;
  ultraclipboard/ service D-Bus, sans fenêtre (testé sur un bus privé) ;
  desktop/       hôte Tauri : icône, menu, fenêtre ; binaire `ultraclipboard` ;
  ui/            fenêtre Svelte 5 / shadcn-svelte (Vite, bun).
packaging/   Service systemd utilisateur.
install.sh   Compile la fenêtre et l'appli Tauri, installe l'extension en local, démarrage auto.
```

## Hors périmètre (pour l'instant)

Windows/macOS, collage différé (le collage attend la fin de la Transformation), Ctrl+V+V,
modules JS embarqués, écosystème de modules, édition de la config depuis la fenêtre,
détection de l'application de destination (impossible sous GNOME/Wayland).

## Ordre de construction

1. Tranche verticale : copie → extension → D-Bus → Rust `upper` → presse-papier.
2. Config YAML, Règles, Pipelines, `trim`, `upper`/`lower`, `command`.
3. `youtube` + annulation « la dernière copie gagne ».
4. Notifications, Original et raccourci de restauration.
5. Fenêtre Svelte, puis icône et menu.

Le moteur Rust est développé en TDD.
