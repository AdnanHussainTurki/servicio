# Contributing to Servicio

Servicio is a cross-platform service supervisor: a Rust engine + daemon that runs and supervises long-running workers, with a Tauri (React/TypeScript) desktop GUI.

Thanks for taking the time to contribute! This guide covers how to build, test, and submit changes.

## Prerequisites

- **Rust** (stable, via [rustup](https://rustup.rs)) — `rustfmt` and `clippy` components.
- **Node.js 20** and npm — for the desktop frontend.
- **macOS** — required to build the GUI app and the universal (Intel + Apple Silicon) bundle. The engine workspace itself builds and tests on Linux and macOS.

## Project layout

- `crates/` — the engine cargo workspace: `servicio-core`, `servicio-ipc`, `servicio-daemon`, `servicio-cli`, `servicio-detect`. Pure Rust, no GUI deps.
- `apps/desktop/` — the Tauri desktop app: a React/TypeScript frontend plus `src-tauri/`, which is its **own** cargo workspace (the GUI bridge to the engine).
- `docs/` — design specs and release docs.

The **daemon supervises long-running workers** — it spawns, monitors, and restarts them. The CLI and GUI are clients that talk to the daemon over IPC.

## Build & test

```bash
cargo test                                   # engine workspace (crates/*)
cd apps/desktop && npm ci && npm run test    # frontend (vitest)
cd apps/desktop/src-tauri && cargo test      # GUI bridge (needs the engine)
PATH="$HOME/.cargo/bin:$PATH" npm run tauri dev   # run the app (build servicio-daemon first / on PATH)
```

> The root `cargo test` covers only the engine workspace. The Tauri bridge in `apps/desktop/src-tauri` is a separate workspace and is tested/built independently.

## Code style

Everything below must pass **before** you open a PR:

- `cargo fmt --all --check` — formatting (rustfmt) must be clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — no clippy warnings.
- `npm run lint` (in `apps/desktop`) — eslint must report **0 errors**.

CI runs all of these; a PR can't merge until they're green.

## Branch & PR workflow

1. Fork the repo and create a branch off `main` (e.g. `feat/my-feature` or `fix/some-bug`).
2. Make your change, keeping commits focused.
3. Run the build/test/lint commands above locally.
4. Open a PR against `main`. Fill in the PR template and link any related issue.
5. Make sure CI is green — maintainers review once checks pass.

## Commit style

This repo uses [Conventional Commits](https://www.conventionalcommits.org/). Prefix commit messages with the type of change:

- `feat:` — a new feature
- `fix:` — a bug fix
- `docs:` — documentation only
- `chore:` — tooling, CI, deps, housekeeping

Example: `fix: restart worker after IPC reconnect`.

## Architecture

The design and architecture live in [`docs/superpowers/specs/2026-06-18-servicio-design.md`](docs/superpowers/specs/2026-06-18-servicio-design.md) (with follow-on phase specs in the same directory). Start there to understand how the core, daemon, IPC, and GUI fit together.

## Releasing

Maintainers cut releases by tagging — see [`docs/RELEASING.md`](docs/RELEASING.md).

Welcome aboard, and thanks for helping make Servicio better!
