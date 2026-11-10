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

- **Une seule Règle par copie** : la première du fichier qui correspond ; les copies non textuelles (images, fichiers) sont ignorées.
- **Tout ou rien** : si une étape échoue ou dépasse son délai, le presse-papier n'est pas touché. Un résultat identique au texte copié ne le touche pas non plus.
- **La dernière copie gagne** : une nouvelle copie, texte ou non (image, fichiers), arrête le Pipeline en cours (le programme d'une `command` est tué) ; son résultat est jeté.
- **Pas de boucle** : ce qu'UltraClipboard écrit lui-même n'est jamais retraité.
- **Coller trop tôt** (avant la fin d'une Transformation lente) donne l'Original.
- **Appli arrêtée** : les copies restent parfaitement normales.

## Configuration

Un seul fichier : `~/.config/ultraclipboard/config.yaml`. Au premier lancement, s'il n'existe pas,
il est créé avec une Règle YouTube d'exemple. Chaque enregistrement est pris en compte aussitôt,
sans redémarrer.

```yaml
config_version: 1           # obligatoire ; seule la version 1 existe
notifications: all          # all | errors | off ; défaut all

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

- **`pipelines`** : des Pipelines nommés, chacun une liste d'étapes exécutées dans l'ordre.
  Une Transformation sans paramètre s'écrit par son nom (`- trim`) ; avec paramètres, comme une clé
  (`- youtube: {template: …}`). `timeout` (en secondes, décimales acceptées) se met à côté, sur
  `youtube` comme sur `command`.
- **`rules`** : `match` est une expression régulière cherchée n'importe où dans le texte copié
  (ancrée seulement avec `^` / `$`), `pipeline` le nom d'un Pipeline.
- **Config invalide** (YAML cassé, `config_version` absente ou inconnue, regex invalide, Pipeline
  ou Transformation inconnus, paramètre manquant…) : elle est refusée avec un message précis
  (notification et fenêtre), l'ancienne config reste active. Invalide dès le lancement : l'appli
  démarre sans aucune Règle et charge le fichier dès qu'il est corrigé.

### Transformations

| Nom | Effet |
|---|---|
| `trim` | Retire les espaces en début et fin |
| `upper` / `lower` | Majuscules / minuscules (Unicode) |
| `youtube` | Récupère titre et chaîne de la vidéo via le service oEmbed de YouTube (sans clé d'API) et rend le `template` : `{title}`, `{url}` (l'URL copiée), `{author_name}`, `{author_url}`, `{thumbnail_url}` ; dans `{title}` et `{author_name}`, `\`, `[` et `]` sont échappés pour Markdown. Sans `template` : `[{title}]({url})`. Vidéo introuvable, réseau absent ou délai dépassé (10 s par défaut) = échec |
| `command` | Lance un exécutable : texte copié sur l'entrée standard, résultat lu sur la sortie standard |

`command` en détail :

- Liste d'arguments, **sans shell** : `["python3", "~/scripts/fix.py"]`. `~/` en début d'argument
  désigne ton dossier personnel. Pour un pipe ou une redirection, passe par `["sh", "-c", "…"]`.
- Les retours à la ligne finaux de la sortie sont retirés (comme `$(…)` en shell).
- Code de sortie ≠ 0 ou délai dépassé (10 s par défaut, `timeout` par étape) = échec ; le processus
  est tué. La sortie d'erreur est gardée dans l'historique.
- Sortie limitée à 8 Mo (au-delà : échec), sortie d'erreur coupée après 64 Ko.
- Un processus lancé en arrière-plan doit rediriger ses sorties (`cmd >/dev/null 2>&1 &`),
  sinon l'étape l'attend jusqu'au délai.

## Usage

### Raccourcis

| Raccourci | Effet |
|---|---|
| `Super+Shift+V` | **Restaurer l'Original** : remet le texte d'avant la dernière Transformation. Marche aussi en pause ; ne fait rien si rien n'a été transformé ; ne remet jamais un texte plus ancien par-dessus une copie plus récente (une copie en cours de traitement est simplement arrêtée) |
| `Super+Shift+R` | **Réappliquer** : passe le contenu actuel du presse-papier dans les Règles, même en pause (pour rattraper une copie faite en pause ou avant une modification de la config) |

Les combinaisons sont dans le schéma de l'extension ; pour les changer :

```sh
gsettings --schemadir ~/.local/share/gnome-shell/extensions/ultraclipboard@grochonnou/schemas \
  set org.gnome.shell.extensions.ultraclipboard ultraclipboard-reapply "['<Super><Alt>r']"
```

(clé `ultraclipboard-restore-original` pour la restauration ; `reset` au lieu de `set …` pour revenir
au défaut.)

### Icône et menu

Une icône dans la barre du haut ; son menu (le clic sur l'icône elle-même n'ouvre rien) :

- **Ouvrir** : affiche la fenêtre.
- **Mettre en pause** / **Reprendre** : en pause, aucune copie n'est transformée ; les deux raccourcis
  marchent toujours.
- **Couper les notifications** / **Rétablir les notifications** : jusqu'au prochain redémarrage de
  l'appli, quel que soit le niveau de la config.
- **Quitter** : arrête l'appli jusqu'à la prochaine session (ou `systemctl --user start ultraclipboard`).

### Notifications

- « ✓ <Pipeline> » quand un résultat est dans le presse-papier (la bulle remplace celle du succès précédent).
- « ✗ <Pipeline> » quand un Pipeline échoue, avec l'étape et la cause, ou rend un texte vide
  (« résultat vide : presse-papier inchangé »).
- « Config invalide » à chaque enregistrement d'une config refusée.

Niveau `notifications` de la config : `all` (tout), `errors` (échecs et config invalide), `off` (rien).

### Fenêtre

En lecture seule, mise à jour toute seule ; la fermer la cache, l'appli continue.

- **Règles** : les Règles dans l'ordre (motif, Pipeline), les Pipelines et leurs étapes, l'état de la
  config (valide, ou l'erreur).
- **Historique** : les 50 dernières transformations, la plus récente en tête : heure, Règle, Pipeline,
  Original, résultat ou erreur (étape, message, sortie d'erreur), statut Succès / Échec / Annulé
  (traitement arrêté avant la fin : copie plus récente, restauration, réapplication ou mise en pause). **En mémoire seulement** : perdu au redémarrage, rien du
  presse-papier n'est écrit sur disque.

## Installation

Prérequis : [Rust](https://rustup.rs), [bun](https://bun.sh) et les paquets système de Tauri :

```sh
sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libsoup-3.0-dev libayatana-appindicator3-dev librsvg2-dev
```

Puis, depuis le dépôt :

```sh
./install.sh
```

Le script vérifie les prérequis (et affiche la commande `sudo apt install` s'il en manque, sans rien
installer lui-même), puis :

1. compile l'appli en release et l'installe dans `~/.local/bin/ultraclipboard` (ce dossier n'est
   parfois dans le `PATH` qu'à la session suivante ; le service, lui, utilise le chemin complet) ;
2. lie l'extension dans `~/.local/share/gnome-shell/extensions/ultraclipboard@grochonnou` (lien vers
   `extension/` de ce dépôt : ne déplace pas le dépôt ; un lien vers un autre dossier est redirigé,
   l'ancienne cible est affichée), compile son schéma et l'active ;
3. installe `~/.config/systemd/user/ultraclipboard.service`, l'active et (re)démarre l'appli.

**Déconnecte-toi puis reconnecte-toi** : sous Wayland, GNOME Shell ne charge une extension nouvelle ou
modifiée qu'à l'ouverture de session. Ensuite l'appli démarre avec chaque session, et une seule
instance tourne (une seconde, lancée à la main, s'arrête : « déjà lancé »). Si une instance lancée à
la main tourne déjà pendant l'installation, le script le signale : le service prendra le relais quand
elle s'arrêtera.

Le service est lié à la session graphique et relancé 3 s après un plantage (`Restart=on-failure`),
sans limite : au démarrage de session, l'affichage peut ne pas être encore prêt. Seul un plantage (ou
`kill -9`) déclenche la relance : « Quitter », `systemctl --user stop ultraclipboard` ou un simple
`kill` (SIGTERM) sont des arrêts voulus.

`install.sh` réécrit `~/.config/systemd/user/ultraclipboard.service` à chaque passage : pour le
personnaliser, utilise un complément (`systemctl --user edit ultraclipboard`, sans `--full`), qui est
conservé.

```sh
systemctl --user status ultraclipboard     # état
journalctl --user -u ultraclipboard -f     # journal (erreurs de config, de notification…)
systemctl --user restart ultraclipboard    # redémarrer
```

### Mise à jour

```sh
git pull && ./install.sh
```

Le script recompile, remplace le binaire et redémarre le service. Si l'extension a changé (`extension/`),
déconnecte-toi / reconnecte-toi.

### Désinstallation

```sh
systemctl --user disable --now ultraclipboard
rm -rf ~/.config/systemd/user/ultraclipboard.service ~/.config/systemd/user/ultraclipboard.service.d
rm ~/.local/bin/ultraclipboard
systemctl --user daemon-reload
gnome-extensions disable ultraclipboard@grochonnou
# raccourcis modifiés : retour aux défauts (avant de retirer le lien, qui porte le schéma)
gsettings --schemadir ~/.local/share/gnome-shell/extensions/ultraclipboard@grochonnou/schemas \
  reset-recursively org.gnome.shell.extensions.ultraclipboard
rm ~/.local/share/gnome-shell/extensions/ultraclipboard@grochonnou
```

Si GNOME n'a jamais chargé l'extension, `gnome-extensions disable` échoue : retire-la alors de la liste
des extensions actives (`gsettings get org.gnome.shell enabled-extensions`, puis `gsettings set
org.gnome.shell enabled-extensions "[…]"` avec la même liste sans `'ultraclipboard@grochonnou'`).

La config (`~/.config/ultraclipboard/`) reste en place.

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
install.sh   Vérifie les prérequis, compile, installe l'appli, l'extension et le service.
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

## Outils similaires

Pas tout à fait ce que tu cherches ? Voici une liste non exhaustive d'outils proches :

- **[espanso](https://espanso.org/)** ([source](https://github.com/espanso/espanso)) : expansion de texte. Linux, Windows, macOS.
- **[AutoHotkey](https://www.autohotkey.com/)** ([source](https://github.com/AutoHotkey/AutoHotkey)) : automatisation clavier et scripts. Windows.
- **[Keyboard Maestro](https://www.keyboardmaestro.com/)** : automatisation, macros. macOS, payant.
- **[Hammerspoon](https://www.hammerspoon.org/)** ([source](https://github.com/Hammerspoon/hammerspoon)) : automatisation scriptée en Lua. macOS.
- **[Ditto](https://sabrogden.github.io/Ditto/)** ([source](https://github.com/sabrogden/Ditto)) : gestionnaire de presse-papier. Windows.
- **[ClipboardFusion](https://www.clipboardfusion.com/)** : presse-papier avec macros. Windows, macOS.
- **[CopyQ](https://hluk.github.io/CopyQ/)** ([source](https://github.com/hluk/CopyQ)) : gestionnaire de presse-papier scriptable. Linux, Windows, macOS.
