// Capteur d'UltraClipboard : voit chaque copie, la signale à l'appli par D-Bus,
// et écrit dans le presse-papier les résultats que l'appli renvoie.
// Toute la logique (Règles, Pipelines…) vit dans l'appli ; l'extension reste bête.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'org.ultraclipboard.UltraClipboard';
const OBJECT_PATH = '/org/ultraclipboard/UltraClipboard';
const IFACE = 'org.ultraclipboard.UltraClipboard1';

export default class UltraClipboardExtension extends Extension {
    enable() {
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
    }

    disable() {
        this._selection.disconnect(this._ownerChangedId);
        Gio.DBus.session.signal_unsubscribe(this._applyId);
        this._selection = null;
        this._clipboard = null;
    }

    _onCopy() {
        this._clipboard.get_text(St.ClipboardType.CLIPBOARD, (_clipboard, text) => {
            // Pas de texte (image, fichier…) : on ne fait rien.
            if (text === null)
                return;

            Gio.DBus.session.call(
                BUS_NAME, OBJECT_PATH, IFACE, 'Copied',
                new GLib.Variant('(s)', [text]), null,
                Gio.DBusCallFlags.NO_AUTO_START, -1, null,
                (connection, result) => {
                    try {
                        connection.call_finish(result);
                    } catch {
                        // Appli non lancée : le presse-papier reste tel quel.
                    }
                });
        });
    }
}
