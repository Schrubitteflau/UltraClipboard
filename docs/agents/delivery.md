# Delivery

Read by `/ship` and its subagents.

## Checks

- Verify, on every ticket and on the integration branch:
  from `app/`: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`;
  from `app/ui/` (after `bun install`): `bun run check && bun run build`.
  The front-end has no unit tests yet (window checked by hand); add `bun run test` here when a ticket adds some.
- Builds are gentle on the human's machine: run cargo as `nice -n 19` with `CARGO_BUILD_JOBS=4`, and throwaway builds go in a `CARGO_TARGET_DIR` under `~/.cache/ship-uc/` (deleted afterwards), never `/tmp`. Reviewers copy the ticket worktree's `app/target` (or the implementer's target dir) into their scratch dir instead of rebuilding from scratch.
- Manual check (sandbox): run the app on a private session bus with a throwaway config, and drive it with `gdbus call --session …` / `gdbus monitor --session` inside the same `dbus-run-session`. It ends with the command; it never touches the real session bus nor the real config.
  The app is a Tauri process and needs a display; nothing started for the check may reach the human's. Strip the display from the environment BEFORE `dbus-run-session`, not inside it: the private `dbus-daemon` and every service it activates (xdg-desktop-portal-gtk/gnome, keyring…) inherit the environment `dbus-run-session` was started with, so unsetting inside only covers the app. Unsetting `DISPLAY`/`WAYLAND_DISPLAY` is not enough either: GTK then falls back to `$XDG_RUNTIME_DIR/wayland-0`, the human's compositor; `GDK_BACKEND=broadway` is REQUIRED. Point the XDG dirs at a temp dir too (the webview otherwise writes to the real `~/.local/share/org.ultraclipboard.UltraClipboard/`):
  ```sh
  T=$(mktemp -d -p ~/.cache/ship-uc)
  env -u DISPLAY -u WAYLAND_DISPLAY \
      GDK_BACKEND=broadway BROADWAY_DISPLAY=:17 NO_AT_BRIDGE=1 GIO_USE_VFS=local \
      XDG_CONFIG_HOME=$T/config XDG_DATA_HOME=$T/data XDG_CACHE_HOME=$T/cache \
      dbus-run-session -- sh -c 'broadwayd :17 & sleep 1; <binary> & sleep 4; gdbus call --session …; kill %1 %2'
  ```
  The D-Bus service, the hidden window and the tray icon start; `Copied` → `Apply` works. Portal services get activated on the private bus and die with it. The tray menu cannot be driven there (no StatusNotifier host).
  A debug binary (`cargo build`) loads the window from Vite's dev URL: without `bun run dev` its window is blank. The real window is checked with `tauri dev` (from `app/desktop/`: `../ui/node_modules/.bin/tauri dev`) or with the release build made by `install.sh`.
- Real browser: the window is a Tauri webview. Agents check its pages by serving the front-end with Vite (`bun run dev` in `app/ui/`, port 5173) and loading it headless with Playwright's Chromium (`~/.cache/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell`, driven by a throwaway `playwright-core` script outside the repo), reading console errors. Outside Tauri, in dev mode, `src/main.ts` installs `src/lib/backend/mock.ts` (Tauri's `mockIPC`, events mocked too): give each new command a plausible answer there. The real Tauri window, the tray icon and the GNOME extension are checked by the human.

## Off-limits

- Real data, never read or written: `~/.config/ultraclipboard/`, the human's clipboard, `~/.local/bin/ultraclipboard`, `~/.local/share/gnome-shell/extensions/`, `~/.config/systemd/user/`.
- Real actions, never performed: running `install.sh`; `systemctl --user` on `ultraclipboard.service`; enabling/disabling GNOME extensions; registering the D-Bus name `org.ultraclipboard.UltraClipboard` on the real session bus; sending desktop notifications on the real session (tests use the private bus).
- Paid services: none. The Internet is never called by tests: `youtube` is tested against a local fake oEmbed server.
- Shared resources: the human's GNOME session and session bus.

## Branches and commits

- Integration branch `ship/v1` from `master`; ticket branches `ship/v1-<ticket>` (e.g. `ship/v1-2`). Worktrees live in `../UltraClipboard-ship/`.
- Running `/ship` is the human's request to commit on those branches, never on `master`.
- Commit messages: French, imperative, short subject ending with the ticket, e.g. `Moteur asynchrone et service D-Bus sans fenêtre (#2)`; body if useful.
- At the end: push `ship/v1`, open a PR to `master`; the human merges after checking the app by hand.

## Docs

- Spec: issue #1; tickets: its sub-issues. Decision journal: `docs/decisions/ultraclipboard-v1.md`. ADRs: `docs/adr/`. Glossary: `GLOSSARY.md`. No separate review-standards file: the spec's "Testing Decisions" are the standard.

## Real usage

One user, one GNOME session. Copies arrive one at a time, by hand, sometimes in quick bursts (a few per second). Copied text is usually short (a URL, a line, a paragraph) but can be large (a whole file, hundreds of KB) and any Unicode. A handful of Rules and Pipelines in the config. `command` runs the user's own scripts; `youtube` hits oEmbed once per matching copy.

## Project edge cases

- zbus serialises method calls: work done inside a D-Bus call blocks the next one (why #2 exists).
- Our own clipboard write comes back as a copy (anti-loop).
- Editors save files by rename/replace, sometimes in two writes: the config watcher must survive it.
- GNOME Shell only sees a new extension after a log-out/log-in under Wayland.
