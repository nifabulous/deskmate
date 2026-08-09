# Contributing to deskmate

Thanks for stopping by. The project is small on purpose, and most
contributions fall into one of three buckets.

## 1. Adapters (most wanted)

An adapter feeds a tool's activity into deskmate as HTTP events. Read
[docs/PROTOCOL.md](docs/PROTOCOL.md), then look at
[adapters/claude-code/](adapters/claude-code/) as the reference. Rules of
thumb: never block the host tool, fail silent when deskmate is down, and
keep bubble titles short. Put each adapter in `adapters/<tool-name>/`
with its own README covering installation.

## 2. The pet

Pets live in `ui/index.html` in the `PETS` registry: each one is a
palette plus five 16x16 string-grid frames (idle, blink, workA, workB,
sleep). Adding a creature means drawing five text grids and adding one
registry entry. The animation logic is a couple hundred lines of vanilla
JS.
Open `ui/index.html` in a browser and it runs a demo event loop, so you
can iterate without building the Rust app. Keep the UI dependency-free.

## 3. The app shell

The Rust side (`src-tauri/`) is intentionally thin: one window, one HTTP
listener. Build with `cargo tauri dev`. Changes that add background
behavior, network calls beyond localhost, or heavyweight dependencies
need a strong reason.

## Ground rules

- Events never leave the machine. deskmate binds to 127.0.0.1 only.
- Don't render secrets: adapters must not put tokens, keys, or full env
  output into `title`/`detail`.
- Sprite art must be original or clearly licensed. Don't copy sprites
  from other pets or games.
- Run `cargo fmt` and keep `cargo clippy` clean for Rust changes.

## Releases

Maintainers cut releases with `cargo tauri build` per platform. Version
lives in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`.
