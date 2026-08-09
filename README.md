# deskmate

A tiny pixel pet that sits on your desktop and shows what your coding
agent is doing. Task starts, tool runs, finishes, and errors show up as
animations and status bubbles, so you can glance at the pet instead of
switching back to the terminal.

deskmate is tool-agnostic. It speaks a simple local HTTP protocol
([docs/PROTOCOL.md](docs/PROTOCOL.md)), and adapters translate each
tool's activity into deskmate events. Claude Code is supported today;
opencode, Cursor, and a generic CLI are on the roadmap.

## How it works

```
Claude Code ──hooks──▶ deskmate-hook.sh ──HTTP──▶ deskmate (Tauri)
opencode    ──plugin─▶ (planned)         127.0.0.1:8990    │
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
git clone https://github.com/YOURNAME/deskmate
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

## Connect Claude Code

See [adapters/claude-code/README.md](adapters/claude-code/README.md).
Two steps: copy one hook script, merge one settings snippet.

## Pet states

| State     | Trigger                        | Animation            |
| --------- | ------------------------------ | -------------------- |
| Idle      | Nothing happening              | Bobbing, blinking    |
| Working   | `task_start` / `tool_use`      | Wing-flapping        |
| Done      | `task_done`                    | Happy hop            |
| Attention | `notify` / `error`             | Shake (+ red bubble) |
| Sleeping  | 5 minutes with no events       | Eyes closed, z z z   |

Drag the pet anywhere on screen. It starts near the bottom-right corner.
Right-click it to switch creatures — v0.1 ships an owl and a cat, and the
choice is remembered.

## Roadmap

- opencode adapter (plugin API)
- Cursor adapter (agent hooks)
- `deskmate notify` CLI for scripts, Makefiles, and CI
- Transcript watcher: tail session logs (e.g. Claude Code JSONL) for
  tools with no hook system, zero per-tool setup
- MCP server mode, so agents can talk to the pet as a tool
- One pet / bubble lane per agent session
- More creatures and community skins (the sprite registry makes this a
  one-file change)
- Click-through mode and a settings panel

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Adapters for new tools are the
most wanted contribution and usually take under 100 lines.

## License

[MIT](LICENSE)
