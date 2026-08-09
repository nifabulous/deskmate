# deskmate

A tiny pixel pet that sits on your desktop and shows what your coding
agent is doing. Task starts, tool runs, finishes, and errors show up as
animations and status bubbles, so you can glance at the pet instead of
switching back to the terminal.

deskmate is tool-agnostic. It speaks a simple local HTTP protocol
([docs/PROTOCOL.md](docs/PROTOCOL.md)), and adapters translate each
tool's activity into deskmate events. Claude Code, opencode, and Cursor
(beta) are supported today; a generic CLI is on the roadmap.

## How it works

```
Claude Code ──hooks──▶ deskmate-hook.sh ──HTTP──▶ deskmate (Tauri)
opencode    ──plugin─▶ deskmate.js       127.0.0.1:8990    │
anything    ──curl───▶ your script                    pixel owl 🦉
```

The app is a frameless, transparent, always-on-top Tauri window. A small
Rust HTTP server receives events on `127.0.0.1:8990` and forwards them to
the pet UI, which is a single dependency-free HTML file. The sprite
frames are plain text grids in `ui/index.html`, so redrawing the pet
needs nothing more than a text editor.

## Install & run

Prerequisites: [Rust](https://rustup.rs) and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```sh
cargo install tauri-cli --version "^2"
git clone https://github.com/nifabulous/deskmate
cd deskmate/src-tauri
cargo tauri dev        # or: cargo tauri build
```

Then poke it:

```sh
curl -X POST http://127.0.0.1:8990/event \
  -H 'Content-Type: application/json' \
  -d '{"kind":"task_start","title":"Review PR #104","detail":"Reading the diff…"}'
```

You can also open `ui/index.html` directly in a browser; without the
Tauri bridge it runs a demo loop so you can iterate on animations.

## Connect your agent

- **Claude Code**: [adapters/claude-code/README.md](adapters/claude-code/README.md)
  — copy one hook script, merge one settings snippet.
- **opencode**: [adapters/opencode/README.md](adapters/opencode/README.md)
  — copy one plugin file into opencode's plugin folder.
- **Cursor** (beta): [adapters/cursor/README.md](adapters/cursor/README.md)
  — copy one hook script, merge one `hooks.json` snippet.
- **Anything else**: `bin/deskmate-notify` is a tiny CLI for scripts,
  Makefiles, and CI — `deskmate-notify -k task_done "Build finished"` —
  or POST to the endpoint directly per [docs/PROTOCOL.md](docs/PROTOCOL.md).

## Pet states

| State     | Trigger                        | Animation            |
| --------- | ------------------------------ | -------------------- |
| Idle      | Nothing happening              | Bobbing, blinking    |
| Working   | `task_start` / `tool_use`      | Wing-flapping        |
| Done      | `task_done`                    | Happy hop            |
| Attention | `notify` / `error`             | Shake; `error` bubbles red |
| Sleeping  | 5 minutes with no events       | Eyes closed, z z z   |

Messages stack above the pet, newest nearest its head. **Click the pet** to
close the panel and **click again** to reopen it — anything that arrives
while it is shut waits there, and a small count sits on the pet's shoulder
so you know how much. Opening gives everything a fresh 45 seconds before it
fades, so a burst that landed while you were away is still readable.

**Drag** the pet anywhere on screen; it starts near the bottom-right corner.
**Right-click** to switch creatures — v0.1 ships an owl and a cat, and the
choice is remembered. Clicks land on the pet and the open panel; the rest of
the window is click-through, so it never blocks what is underneath. Quit
from the tray icon.

## Roadmap

**Getting updates.** Today the only way to update is `git pull` and
rebuild, which needs a Rust toolchain just to pick up a bugfix. In order:

- Publish releases, so people download a build instead of installing
  Rust. Tagging `v*` builds macOS, Windows, and Linux and attaches them
  to a GitHub release — see [.github/workflows/release.yml](.github/workflows/release.yml).
- In-app updates via `tauri-plugin-updater`: check a signed manifest,
  install on restart. Needs a signing keypair before it can ship.
- Adapter version checks. Adapters are *copied* into place, so
  `git pull` updates the repo but not the installed hook — someone can
  run a months-old adapter against a current deskmate and never know.
  Stamping a version into each event and bubbling a mismatch is enough.

**Everything else.**

- Click a message to jump to the session that sent it. Blocked on a way
  to focus a session, and the answer differs by how the agent is run:

  - **In a terminal** this is doable now. The hook can capture
    `$TERM_PROGRAM` plus `$ITERM_SESSION_ID` / `$TERM_SESSION_ID` and
    send them with the event; focusing that tab is then a few lines of
    AppleScript on macOS.
  - **In the Claude desktop app** there is nothing to focus. None of
    those variables are set, and the app's only relevant URL route,
    `claude://resume`, imports a transcript into a new view rather than
    focusing an already-open tab — its own error strings say so
    (`code_deeplink_resume_import_failed`). The parameter format is
    undocumented.

  So this waits on a supported "focus this session" entry point. The
  session id, `transcript_path`, and `cwd` already arrive with every
  event, so the pet end is ready whenever the other end exists.

- Transcript watcher: tail session logs (e.g. Claude Code JSONL) for
  tools with no hook system, zero per-tool setup
- MCP server mode, so agents can talk to the pet as a tool
- One pet per agent session, rather than one pet with per-session lanes
- More creatures and community skins (the sprite registry makes this a
  one-file change)
- A settings panel

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Adapters for new tools are the
most wanted contribution and usually take under 100 lines.

## License

[MIT](LICENSE)
