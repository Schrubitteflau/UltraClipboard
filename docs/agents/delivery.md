# Delivery

Read by `/ship` and its subagents.

## Checks

- Verify, on every ticket and on the integration branch, from `app/`:
  `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
  Once the window front-end exists (ticket #4), its type check and tests (`bun run check`, `bun run test` or whatever #4 sets up) are added here by that ticket.
- Manual check (sandbox): run the app on a private session bus with a throwaway config:
  `XDG_CONFIG_HOME=$(mktemp -d) dbus-run-session -- <command>`; drive it with `gdbus call --session …` / `gdbus monitor --session` inside the same `dbus-run-session`. It ends with the command; it never touches the real session bus nor the real config.
- Real browser: the window is a Tauri webview. Agents check its pages by serving the front-end dev server (Vite, `bun run dev`) and loading it headless with Playwright's Chromium (already in `~/.cache/ms-playwright`), reading console errors; Tauri commands/events are mocked at that level. The real Tauri window, the tray icon and the GNOME extension are checked by the human.

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
