// Capteur d'UltraClipboard : voit chaque copie, la signale à l'appli par D-Bus,
// écrit dans le presse-papier les résultats que l'appli renvoie, et porte les
// raccourcis « restaurer l'Original » et « réappliquer ».
// Toute la logique (Règles, Pipelines, Original…) vit dans l'appli ; l'extension reste bête.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const BUS_NAME = 'org.ultraclipboard.UltraClipboard';
const OBJECT_PATH = '/org/ultraclipboard/UltraClipboard';
const IFACE = 'org.ultraclipboard.UltraClipboard1';

// Au-delà, le texte n'est pas envoyé : un message D-Bus trop gros (limite 128 Mio)
// fermerait la connexion de GNOME Shell au bus de session, donc la session.
// 10 M unités UTF-16 font au plus 30 Mo d'UTF-8.
const MAX_TEXT_LENGTH = 10_000_000;

const RESTORE_KEY = 'ultraclipboard-restore-original';
const REAPPLY_KEY = 'ultraclipboard-reapply';

export default class UltraClipboardExtension extends Extension {
    enable() {
        // En premier : sans schéma compilé, getSettings() lève une exception
        // et rien n'a encore été branché.
        this._settings = this.getSettings();
        this._clipboard = St.Clipboard.get_default();
        this._selection = global.display.get_selection();

        this._ownerChangedId = this._selection.connect('owner-changed', (_selection, type) => {
            if (type === Meta.SelectionType.SELECTION_CLIPBOARD)
                this._onCopy();
        });

        this._applyId = Gio.DBus.session.signal_subscribe(
            BUS_NAME, IFACE, 'Apply', OBJECT_PATH, null, Gio.DBusSignalFlags.NONE,
            (_connection, _sender, _path, _iface, _signal, params) => {
                const [text] = params.deepUnpack();
                this._clipboard.set_text(St.ClipboardType.CLIPBOARD, text);
            });

        // Raccourcis globaux : combinaisons lues dans le schéma GSettings de l'extension.
        this._addShortcut(RESTORE_KEY, () => this._call('RestoreOriginal', null));
        this._addShortcut(REAPPLY_KEY, () => this._onReapply());
    }

    disable() {
        Main.wm.removeKeybinding(RESTORE_KEY);
        Main.wm.removeKeybinding(REAPPLY_KEY);
        this._settings = null;
        this._selection.disconnect(this._ownerChangedId);
        Gio.DBus.session.signal_unsubscribe(this._applyId);
        this._selection = null;
        this._clipboard = null;
    }

    _addShortcut(key, handler) {
        Main.wm.addKeybinding(
            key, this._settings,
            Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW,
            handler);
    }

    _onCopy() {
        this._clipboard.get_text(St.ClipboardType.CLIPBOARD, (_clipboard, text) => {
            // Pas de texte (image, fichier…) ou texte trop gros pour D-Bus : rien
            // à transformer, mais l'appli doit savoir qu'une copie a eu lieu,
            // pour qu'un résultat tardif ne la remplace pas.
            if (text === null || text.length > MAX_TEXT_LENGTH)
                this._call('CopiedNonText', null);
            else
                this._call('Copied', new GLib.Variant('(s)', [text]));
        });
    }

    _onReapply() {
        this._clipboard.get_text(St.ClipboardType.CLIPBOARD, (_clipboard, text) => {
            // Presse-papier vide, sans texte ou trop gros : rien à réappliquer.
            if (text && text.length <= MAX_TEXT_LENGTH)
                this._call('Reapply', new GLib.Variant('(s)', [text]));
        });
    }

    // Appel sans auto-démarrage. Toute erreur d'appel (appli absente, en cours de
    // démarrage ou de redémarrage, message refusé…) est avalée : rien ne se passe.
    _call(method, params) {
        Gio.DBus.session.call(
            BUS_NAME, OBJECT_PATH, IFACE, method,
            params, null,
            Gio.DBusCallFlags.NO_AUTO_START, -1, null,
            (connection, result) => {
                try {
                    connection.call_finish(result);
                } catch {
                    // Le presse-papier reste tel quel.
                }
            });
    }
}
