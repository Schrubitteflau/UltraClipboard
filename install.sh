#!/usr/bin/env bash
# Installe UltraClipboard pour l'utilisateur courant (Ubuntu 26.04, GNOME, Wayland) :
# binaire dans ~/.local/bin, extension GNOME liée vers ce dépôt, service systemd utilisateur.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
UUID="ultraclipboard@grochonnou"
EXT_LINK="$HOME/.local/share/gnome-shell/extensions/$UUID"
UNIT_DIR="$HOME/.config/systemd/user"

echo "==> Compilation de l'appli"
cargo build --release --manifest-path "$ROOT/app/Cargo.toml"
install -Dm755 "$ROOT/app/target/release/ultraclipboard" "$HOME/.local/bin/ultraclipboard"

echo "==> Extension GNOME Shell"
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
