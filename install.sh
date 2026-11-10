#!/usr/bin/env bash
# Installe (ou met à jour) UltraClipboard pour l'utilisateur courant
# (Ubuntu 26.04, GNOME, Wayland) :
#   - appli Tauri compilée en release, dans ~/.local/bin/ultraclipboard ;
#   - extension GNOME Shell liée vers ce dépôt, schéma des raccourcis compilé, activée ;
#   - service systemd utilisateur (démarrage avec la session, relance après un plantage).
# Relançable sans risque : chaque étape remplace la précédente installation.
set -euo pipefail

if [[ $EUID -eq 0 ]]; then
    echo "Lance ./install.sh avec ton utilisateur, pas en root (ni avec sudo) :"
    echo "tout s'installe dans ton dossier personnel."
    exit 1
fi

ROOT="$(dirname "$(readlink -f "$0")")"
UUID="ultraclipboard@grochonnou"
BUS_NAME="org.ultraclipboard.UltraClipboard"
# Chemins fixes : ceux où regardent le gestionnaire systemd utilisateur et GNOME
# Shell, quelles que soient les variables XDG_* du terminal.
BIN="$HOME/.local/bin/ultraclipboard" # repris par packaging/ultraclipboard.service
EXT_LINK="$HOME/.local/share/gnome-shell/extensions/$UUID"
UNIT_DIR="$HOME/.config/systemd/user"

has() { command -v "$1" >/dev/null 2>&1; }

echo "==> Prérequis"
apt_missing=()
other_missing=()
has cc || apt_missing+=(build-essential)
has glib-compile-schemas || apt_missing+=(libglib2.0-bin)
# Bibliothèques système de Tauri : module pkg-config → paquet apt.
tauri_libs=(
    "webkit2gtk-4.1:libwebkit2gtk-4.1-dev"
    "libsoup-3.0:libsoup-3.0-dev"
    "ayatana-appindicator3-0.1:libayatana-appindicator3-dev"
    "librsvg-2.0:librsvg2-dev"
)
if has pkg-config; then
    for lib in "${tauri_libs[@]}"; do
        pkg-config --exists "${lib%%:*}" || apt_missing+=("${lib#*:}")
    done
else
    apt_missing+=(pkg-config)
    for lib in "${tauri_libs[@]}"; do apt_missing+=("${lib#*:}"); done
fi
has cargo || other_missing+=("Rust (cargo) : https://rustup.rs")
has bun || other_missing+=("bun : https://bun.sh")
if ((${#apt_missing[@]} + ${#other_missing[@]})); then
    if ((${#apt_missing[@]})); then
        echo "    Paquets système manquants : ${apt_missing[*]}. Installe-les avec :"
        echo "    sudo apt install ${apt_missing[*]}"
    fi
    for tool in "${other_missing[@]}"; do
        echo "    Manquant : $tool"
    done
    echo "    Puis relance ./install.sh."
    exit 1
fi

# L'extension est installée comme lien vers ce dépôt : un vrai dossier à sa place
# (ancienne copie) ne doit pas être écrasé.
if [[ -e $EXT_LINK && ! -L $EXT_LINK ]]; then
    echo "    $EXT_LINK existe et n'est pas un lien (installation manuelle ?) :"
    echo "    déplace-le ou supprime-le, puis relance ./install.sh."
    exit 1
fi

echo "==> Compilation de l'appli (fenêtre + appli Tauri, release)"
bun install --frozen-lockfile --cwd "$ROOT/app/ui"
# `--bun` : la CLI Tauri et Vite tournent sous bun, Node.js n'est pas requis.
(cd "$ROOT/app/desktop" && bun --bun "$ROOT/app/ui/node_modules/.bin/tauri" build --no-bundle)
# Dossier de compilation tel que cargo le voit (CARGO_TARGET_DIR relatif, config cargo…).
target_dir=$(cd "$ROOT/app/desktop" && cargo metadata --format-version 1 --no-deps |
    sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p') || true
install -Dm755 "${target_dir:-$ROOT/app/target}/release/ultraclipboard" "$BIN"
echo "    $BIN"

echo "==> Extension GNOME Shell"
glib-compile-schemas --strict "$ROOT/extension/schemas"
mkdir -p "$(dirname "$EXT_LINK")"
if [[ -L $EXT_LINK ]]; then
    old_target=$(readlink "$EXT_LINK")
    if [[ $old_target != "$ROOT/extension" ]]; then
        echo "    Lien déplacé de $old_target vers $ROOT/extension"
    fi
fi
ln -sfn "$ROOT/extension" "$EXT_LINK"
echo "    $EXT_LINK -> $ROOT/extension"

# Ajoute $UUID à la liste org.gnome.shell `$1` (format GVariant `as`).
shell_list_add() {
    local list
    list=$(gsettings get org.gnome.shell "$1") || return 1
    [[ $list == *"'$UUID'"* ]] && return 0
    case $list in
        "@as []" | "[]") list="['$UUID']" ;;
        *) list="${list%]}, '$UUID']" ;;
    esac
    gsettings set org.gnome.shell "$1" "$list"
}

# Retire $UUID de la liste org.gnome.shell `$1`.
shell_list_remove() {
    local list
    list=$(gsettings get org.gnome.shell "$1") || return 1
    [[ $list == *"'$UUID'"* ]] || return 0
    list=${list//", '$UUID'"/}
    list=${list//"'$UUID', "/}
    list=${list//"'$UUID'"/}
    gsettings set org.gnome.shell "$1" "$list"
}

# Sous Wayland, GNOME Shell ne voit une nouvelle extension qu'à la session suivante :
# `gnome-extensions enable` échoue alors ; on l'inscrit directement dans la liste des
# extensions actives, que GNOME Shell lit à l'ouverture de session.
if gnome-extensions enable "$UUID" >/dev/null 2>&1; then
    echo "    Extension activée."
elif has gsettings && shell_list_add enabled-extensions && shell_list_remove disabled-extensions; then
    echo "    Extension activée à partir de la prochaine session."
else
    echo "    Impossible d'activer l'extension depuis ce terminal ; après reconnexion :"
    echo "    gnome-extensions enable $UUID"
fi
if has gsettings && [[ $(gsettings get org.gnome.shell disable-user-extensions 2>/dev/null) == true ]]; then
    echo "    Attention : les extensions utilisateur sont désactivées dans GNOME"
    echo "    (appli Extensions, interrupteur général) ; l'extension ne tournera pas."
fi

echo "==> Service utilisateur (démarrage avec la session)"
user_systemctl() {
    systemctl --user "$@" && return 0
    echo "    « systemctl --user $* » a échoué : lance ./install.sh depuis ta session graphique"
    echo "    (un terminal de ton bureau GNOME, pas une connexion SSH ni sudo)."
    exit 1
}
install -Dm644 "$ROOT/packaging/ultraclipboard.service" "$UNIT_DIR/ultraclipboard.service"
user_systemctl daemon-reload
user_systemctl enable ultraclipboard.service
user_systemctl restart ultraclipboard.service
# L'appli s'arrête aussitôt si le nom D-Bus est déjà pris : on lui laisse le temps.
sleep 2
if [[ $(systemctl --user is-active ultraclipboard.service 2>/dev/null) == active ]]; then
    echo "    ultraclipboard.service actif (relancé avec la nouvelle version)."
elif gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.NameHasOwner "$BUS_NAME" 2>/dev/null | grep -q true; then
    echo "    Une autre instance d'UltraClipboard tourne (lancée à la main ?) : le service"
    echo "    prendra le relais quand elle s'arrêtera (Quitter dans son menu)."
else
    echo "    Le service ne démarre pas : journalctl --user -u ultraclipboard -e"
fi

config_home=$(systemctl --user show-environment 2>/dev/null | sed -n 's/^XDG_CONFIG_HOME=//p') || true
cat <<EOF
==> Installé.
    Déconnecte-toi puis reconnecte-toi pour que GNOME Shell charge l'extension
    (première installation, ou extension modifiée par la mise à jour).
    Config : ${config_home:-$HOME/.config}/ultraclipboard/config.yaml (créée au premier lancement).
    Journal : journalctl --user -u ultraclipboard -f
EOF
