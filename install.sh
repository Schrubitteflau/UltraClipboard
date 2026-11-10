#!/usr/bin/env bash
# Installe UltraClipboard pour l'utilisateur courant (Ubuntu 26.04, GNOME, Wayland) :
# appli Tauri dans ~/.local/bin, extension GNOME liée vers ce dépôt (schéma des raccourcis compilé),
# service systemd utilisateur.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
UUID="ultraclipboard@grochonnou"
EXT_LINK="$HOME/.local/share/gnome-shell/extensions/$UUID"
UNIT_DIR="$HOME/.config/systemd/user"

echo "==> Prérequis système de Tauri"
missing=()
for pc in webkit2gtk-4.1 libsoup-3.0 ayatana-appindicator3-0.1 librsvg-2.0; do
    pkg-config --exists "$pc" || missing+=("$pc")
done
if ((${#missing[@]})); then
    echo "    Manquant : ${missing[*]}. Installe-les avec :"
    echo "    sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libayatana-appindicator3-dev librsvg2-dev"
    exit 1
fi
command -v bun >/dev/null || { echo "    bun est requis (https://bun.sh)"; exit 1; }

echo "==> Compilation de l'appli (fenêtre + appli Tauri)"
bun install --frozen-lockfile --cwd "$ROOT/app/ui"
(cd "$ROOT/app/desktop" && "$ROOT/app/ui/node_modules/.bin/tauri" build --no-bundle)
install -Dm755 "${CARGO_TARGET_DIR:-$ROOT/app/target}/release/ultraclipboard" "$HOME/.local/bin/ultraclipboard"

echo "==> Extension GNOME Shell"
glib-compile-schemas --strict "$ROOT/extension/schemas"
mkdir -p "$(dirname "$EXT_LINK")"
ln -sfn "$ROOT/extension" "$EXT_LINK"
if ! gnome-extensions enable "$UUID" 2>/dev/null; then
    echo "    GNOME ne connaît pas encore l'extension (normal sous Wayland) :"
    echo "    déconnecte-toi / reconnecte-toi, puis : gnome-extensions enable $UUID"
fi

echo "==> Service utilisateur (démarrage automatique)"
install -Dm644 "$ROOT/packaging/ultraclipboard.service" "$UNIT_DIR/ultraclipboard.service"
systemctl --user daemon-reload
systemctl --user enable ultraclipboard.service
systemctl --user restart ultraclipboard.service

echo "==> Installé. Journal : journalctl --user -u ultraclipboard -f"
