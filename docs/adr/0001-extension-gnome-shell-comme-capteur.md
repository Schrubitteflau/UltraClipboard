# Une extension GNOME Shell comme capteur du presse-papier

Sous GNOME/Wayland, Mutter n'implémente ni `wlr-data-control` ni `ext-data-control-v1` : un processus ordinaire ne peut ni voir les copies ni écrire le presse-papier sans avoir le focus (d'où le scintillement de `wl-clipboard`). Nous confions donc la détection des copies, l'écriture du presse-papier et les raccourcis à une petite extension GNOME Shell (signal `owner-changed` de `Meta.Selection`, `St.Clipboard`), qui délègue tout le reste à l'appli Rust via D-Bus.

## Considered Options

- **Lire le clavier façon Espanso** (evdev + `setcap cap_dac_override`) : voit toutes les frappes, coût de sécurité élevé, et ne donne toujours pas accès au presse-papier sans focus.
- **Démon qui interroge `wl-paste`** : chaque lecture vole le focus et fait scintiller l'écran.
- **Raccourci GNOME lançant une commande ponctuelle** : simple et sans privilège, mais pas de déclenchement automatique à la copie, or c'est ce qui laisse le temps aux Transformations lentes (YouTube) de finir avant le collage.

## Consequences

- GNOME uniquement ; l'extension doit être revalidée à chaque version majeure de GNOME (`shell-version` dans `metadata.json`).
- Pas de collage différé (fournir le texte seulement au moment du collage) : GJS ne permet pas d'implémenter `Meta.SelectionSource.read_async` (GJS #72). Piste non vérifiée : posséder le presse-papier côté X11 via Xwayland.
- Ctrl+V reste invisible pour l'extension ; d'où un raccourci dédié pour restaurer l'Original plutôt que Ctrl+V+V.
