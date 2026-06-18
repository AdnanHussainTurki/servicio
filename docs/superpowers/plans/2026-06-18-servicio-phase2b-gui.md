# Servicio Phase 2b — Minimal Tauri GUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. For the visual React components, implementers SHOULD invoke the `frontend-design` skill to keep the UI distinctive and polished.

**Goal:** A Tauri desktop app that auto-spawns the `servicio-daemon` sidecar, connects over the Phase-2a socket API via a Rust bridge that reuses `servicio-cli`'s `Client`, and presents a live card-grid dashboard, a worker detail view with live logs, start/stop/restart controls, and a simple add-worker form.

**Architecture:** The Tauri Rust backend owns all socket I/O (React can't open a Unix socket): it holds a command `Client` in managed state, runs a background `subscribe` task that forwards daemon events to the frontend via Tauri's event system, and manages the daemon sidecar lifecycle. The React/TS frontend (Vite + Tailwind + Zustand) invokes backend commands and listens for `worker-event`s to live-update a store.

**Tech Stack:** Tauri 2, Rust (tokio), `servicio-cli` (Client lib) + `servicio-ipc` (path deps); React 18 + TypeScript + Vite + Tailwind + Zustand; `@tauri-apps/cli` (npm, no global install); Vitest + Testing Library for frontend tests.

**Builds on:** Phase 1 + 2a (merged). Spec: `docs/superpowers/specs/2026-06-18-servicio-phase2b-gui-design.md`.

**Environment notes:** Node 20 + npm 10 + rustc 1.92 are present; `cargo-tauri` is NOT installed (use the npm `@tauri-apps/cli`). Tauri/npm installs download many packages + heavy Rust crates; the network has been intermittently slow — if a download times out, retry (`npm install` / `cargo build`) rather than changing dependencies. macOS provides WebKit for Tauri; no extra system libs needed on this host.

---

## File Structure

```
apps/desktop/
  package.json              # react, vite, tailwind, zustand, @tauri-apps/api, @tauri-apps/cli, vitest
  vite.config.ts
  tailwind.config.js, postcss.config.js
  index.html
  tsconfig.json
  src/
    main.tsx                # React entry
    App.tsx                 # shell + routing-by-state
    types.ts                # TS mirrors of ipc JSON shapes
    api.ts                  # typed invoke wrappers + event subscription
    store.ts                # Zustand store (workers, logs, daemon)
    store.test.ts           # store unit tests
    components/
      Sidebar.tsx
      StatusFooter.tsx
      Dashboard.tsx
      WorkerCard.tsx
      WorkerDetail.tsx
      LogView.tsx
      AddWorkerForm.tsx
      *.test.tsx            # component tests
    index.css               # tailwind directives + theme
  src-tauri/
    Cargo.toml              # standalone crate; deps: tauri, tokio, servicio-cli, servicio-ipc, serde
    tauri.conf.json         # externalBin sidecar = servicio-daemon
    build.rs
    src/
      main.rs               # Tauri builder, state, command registration, startup
      state.rs              # AppState { client, base, token }
      sidecar.rs            # resolve base, ensure daemon, connect
      bridge.rs             # command bodies (injectable, testable)
      events.rs             # subscribe task -> emit worker-event
    tests/
      bridge_integration.rs # bridge commands vs in-process real daemon
```

`apps/desktop/src-tauri` is a STANDALONE Cargo crate (its own `[package]`, NOT a member of the root engine workspace), depending on the engine crates by relative path. This keeps `cargo test` on the engine fast and isolates Tauri's heavy build deps.

---

## Task 0: Scaffold the Tauri + React app

**Files:** create `apps/desktop/**` baseline.

- [ ] **Step 1: Create the React app skeleton with Vite**

From the repo root:
```bash
mkdir -p apps/desktop
cd apps/desktop
npm create vite@latest . -- --template react-ts
```
If `npm create` prompts, accept the React + TypeScript template into the current directory. Then:
```bash
npm install
npm install -D @tauri-apps/cli vitest @testing-library/react @testing-library/jest-dom jsdom tailwindcss postcss autoprefixer
npm install @tauri-apps/api zustand
npx tailwindcss init -p
```
If any install times out, re-run it (network retries) before proceeding.

- [ ] **Step 2: Configure Tailwind**

Set `apps/desktop/tailwind.config.js` `content`:
```js
/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: { extend: {} },
  plugins: [],
};
```
Replace `apps/desktop/src/index.css` with:
```css
@tailwind base;
@tailwind components;
@tailwind utilities;

:root { color-scheme: light dark; }
body { @apply bg-slate-50 text-slate-900 dark:bg-slate-950 dark:text-slate-100; }
```

- [ ] **Step 3: Initialize Tauri (npm CLI, no global install)**

From `apps/desktop`:
```bash
npx @tauri-apps/cli@latest init --ci \
  --app-name servicio \
  --window-title Servicio \
  --frontend-dist ../dist \
  --dev-url http://localhost:5173 \
  --before-dev-command "npm run dev" \
  --before-build-command "npm run build"
```
This creates `src-tauri/`. If the interactive prompts still appear, answer: app name `servicio`, window title `Servicio`, web assets `../dist`, dev server `http://localhost:5173`, dev command `npm run dev`, build command `npm run build`.

- [ ] **Step 4: Add the Vitest config + scripts**

In `apps/desktop/package.json` `scripts`, add:
```json
"test": "vitest run",
"tauri": "tauri"
```
Create `apps/desktop/vitest.config.ts`:
```ts
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: { environment: "jsdom", globals: true, setupFiles: [] },
});
```

- [ ] **Step 5: Verify the skeleton builds**

Run (from `apps/desktop`): `npm run build`
Expected: Vite builds `dist/` with no errors. (Do NOT run `npm run tauri dev` yet — backend wiring comes next; a Rust compile of bare Tauri is fine but slow.)

- [ ] **Step 6: Commit**

```bash
cd /Users/adnanhussain/Documents/projects/servicio
git add apps/desktop
git commit -m "chore(gui): scaffold tauri + react/vite/tailwind/zustand app"
```

> Note: add `apps/desktop/node_modules`, `apps/desktop/dist`, and `apps/desktop/src-tauri/target` to `.gitignore` if not already covered. The root `.gitignore` has `node_modules/` and `/target`; add `apps/desktop/dist/` and `apps/desktop/src-tauri/target/`.

---

## Task 1: Backend state + sidecar connect + `daemon_status` (TDD)

**Files:** `apps/desktop/src-tauri/Cargo.toml`, `src/{state,sidecar,bridge,main}.rs`, `tests/bridge_integration.rs`.

- [ ] **Step 1: Set the Tauri crate dependencies**

Edit `apps/desktop/src-tauri/Cargo.toml` `[dependencies]` to include (keep the tauri/serde lines the scaffold generated; add ours):
```toml
servicio-cli = { path = "../../../crates/servicio-cli" }
servicio-ipc = { path = "../../../crates/servicio-ipc" }
servicio-core = { path = "../../../crates/servicio-core" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"

[dev-dependencies]
servicio-daemon = { path = "../../../crates/servicio-daemon" }
tempfile = "3"
```
Confirm this crate is NOT listed in the root `Cargo.toml` workspace `members` (it must stay standalone). If cargo complains the path-dep crates belong to a different workspace, add to `apps/desktop/src-tauri/Cargo.toml`:
```toml
[workspace]
```
(an empty `[workspace]` table makes this crate its own workspace root, detaching it from the engine workspace).

- [ ] **Step 2: Write the failing integration test**

Create `apps/desktop/src-tauri/tests/bridge_integration.rs`:
```rust
// Drive the bridge command bodies against a real in-process daemon over the socket.
use servicio_daemon_lib::paths::Paths;
use servicio_daemon_lib::serve::serve;
use std::time::Duration;
use servicio_app::bridge;
use servicio_app::state::AppState;

async fn running_daemon(dir: &std::path::Path) -> (Paths, servicio_daemon_lib::serve::ServeHandle, AppState) {
    let paths = Paths::new(dir.to_path_buf());
    let handle = serve(paths.clone(), "secret".into()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let state = AppState::connect(&paths.socket(), "secret").await.unwrap();
    (paths, handle, state)
}

#[tokio::test]
async fn daemon_status_reports_connected() {
    let dir = tempfile::tempdir().unwrap();
    let (_p, handle, state) = running_daemon(dir.path()).await;
    let status = bridge::daemon_status(&state).await.unwrap();
    assert!(status.connected);
    handle.shutdown().await;
}
```
This requires the Tauri crate to expose a library named `servicio_app` in addition to the binary. The scaffold made a `[[bin]]`; we add a `[lib]`.

- [ ] **Step 3: Make the crate a lib + bin**

In `apps/desktop/src-tauri/Cargo.toml`, ensure:
```toml
[lib]
name = "servicio_app"
path = "src/lib.rs"

[[bin]]
name = "servicio"
path = "src/main.rs"
```

- [ ] **Step 4: Implement state + bridge (minimal) + lib.rs**

Create `apps/desktop/src-tauri/src/lib.rs`:
```rust
pub mod bridge;
pub mod events;
pub mod sidecar;
pub mod state;
```

Create `apps/desktop/src-tauri/src/state.rs`:
```rust
use anyhow::Result;
use servicio_cli_lib::Client;
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

/// Backend state shared across Tauri commands.
pub struct AppState {
    pub client: Mutex<Client>,
    pub base: PathBuf,
    pub token: String,
}

impl AppState {
    /// Connect a command Client to an already-running daemon socket.
    pub async fn connect(socket: &Path, token: &str) -> Result<Self> {
        let client = Client::connect(socket, token).await?;
        Ok(Self {
            client: Mutex::new(client),
            base: socket.parent().unwrap_or(Path::new("/")).to_path_buf(),
            token: token.to_string(),
        })
    }
}
```

Create `apps/desktop/src-tauri/src/bridge.rs`:
```rust
use crate::state::AppState;
use serde::Serialize;

#[derive(Serialize)]
pub struct DaemonStatus {
    pub connected: bool,
    pub version: String,
    pub uptime_secs: u64,
    pub worker_count: u32,
    pub running_count: u32,
}

/// Query daemon_info via the command Client.
pub async fn daemon_status(state: &AppState) -> Result<DaemonStatus, String> {
    let mut client = state.client.lock().await;
    match client.daemon_info().await {
        Ok(v) => Ok(DaemonStatus {
            connected: true,
            version: v.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            uptime_secs: v.get("uptime_secs").and_then(|x| x.as_u64()).unwrap_or(0),
            worker_count: v.get("worker_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            running_count: v.get("running_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        }),
        Err(e) => Err(e.to_string()),
    }
}
```

Create stub `apps/desktop/src-tauri/src/sidecar.rs` and `events.rs`:
```rust
// sidecar.rs — filled in Step 6 / Task 3.
```
```rust
// events.rs — filled in Task 3.
```

- [ ] **Step 5: Run the integration test**

From `apps/desktop/src-tauri`: `cargo test --test bridge_integration`
Expected: PASS (1 test). If Tauri's own deps are still downloading, retry once.

- [ ] **Step 6: Implement sidecar resolve + ensure-daemon (used by main.rs)**

Replace `apps/desktop/src-tauri/src/sidecar.rs`:
```rust
use anyhow::{anyhow, Result};
use servicio_cli_lib::Client;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;

/// Default base dir for the bundled daemon (mirrors the daemon's own default).
pub fn default_base() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir).join("servicio")
    } else {
        std::env::temp_dir().join("servicio")
    }
}

pub fn socket_path(base: &std::path::Path) -> PathBuf { base.join("daemon.sock") }
pub fn token_path(base: &std::path::Path) -> PathBuf { base.join("token") }

/// Ensure a daemon is reachable at `base`: if a connection fails, spawn the sidecar
/// `servicio-daemon serve --base <base>` (via the provided command program) and wait for
/// the socket. Returns the token once available.
pub async fn ensure_daemon(base: &std::path::Path, daemon_program: &str) -> Result<String> {
    std::fs::create_dir_all(base).ok();
    // Already up?
    if let Ok(token) = read_token(base) {
        if Client::connect(&socket_path(base), &token).await.is_ok() {
            return Ok(token);
        }
    }
    // Spawn the sidecar.
    Command::new(daemon_program)
        .arg("serve")
        .arg("--base")
        .arg(base)
        .spawn()
        .map_err(|e| anyhow!("spawn daemon: {e}"))?;
    // Wait for the socket + token, then a successful connect.
    for _ in 0..50 {
        if let Ok(token) = read_token(base) {
            if Client::connect(&socket_path(base), &token).await.is_ok() {
                return Ok(token);
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(anyhow!("daemon did not become ready"))
}

fn read_token(base: &std::path::Path) -> Result<String> {
    Ok(std::fs::read_to_string(token_path(base))?.trim().to_string())
}
```

- [ ] **Step 7: Commit**

```bash
cd /Users/adnanhussain/Documents/projects/servicio
git add apps/desktop/src-tauri
git commit -m "feat(gui): backend AppState, daemon_status bridge, sidecar ensure-daemon"
```

---

## Task 2: Bridge commands — list/add/start/stop/restart (TDD)

**Files:** `apps/desktop/src-tauri/src/bridge.rs`, `tests/bridge_integration.rs`.

- [ ] **Step 1: Add failing integration tests**

Append to `apps/desktop/src-tauri/tests/bridge_integration.rs`:
```rust
use servicio_core::worker::{RestartPolicy, RunMode, WorkerSpec};
use std::collections::BTreeMap;

fn sleeper(name: &str) -> WorkerSpec {
    WorkerSpec {
        name: name.into(),
        command: "sh".into(),
        args: vec!["-c".into(), "sleep 30".into()],
        working_dir: std::path::PathBuf::from("/"),
        env: BTreeMap::new(),
        run_mode: RunMode::Daemon { concurrency: 1 },
        restart: RestartPolicy::default(),
        autostart: false,
        enabled: true,
    }
}

#[tokio::test]
async fn add_list_start_stop_via_bridge() {
    let dir = tempfile::tempdir().unwrap();
    let (_p, handle, state) = running_daemon(dir.path()).await;

    bridge::add_worker(&state, sleeper("q")).await.unwrap();
    let list = bridge::list_workers(&state).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "q");

    bridge::start_worker(&state, "q").await.unwrap();
    bridge::stop_worker(&state, "q").await.unwrap();
    bridge::restart_worker(&state, "q").await.unwrap();
    bridge::stop_worker(&state, "q").await.unwrap();

    handle.shutdown().await;
}
```

- [ ] **Step 2: Run, confirm FAIL**

From `apps/desktop/src-tauri`: `cargo test --test bridge_integration add_list_start_stop`
Expected: FAIL — `no function add_worker in bridge`.

- [ ] **Step 3: Implement the commands**

Add to `apps/desktop/src-tauri/src/bridge.rs`:
```rust
use servicio_core::worker::WorkerSpec;
use servicio_ipc::types::WorkerStatus;

pub async fn list_workers(state: &AppState) -> Result<Vec<WorkerStatus>, String> {
    let mut client = state.client.lock().await;
    client.list_workers().await.map_err(|e| e.to_string())
}

pub async fn add_worker(state: &AppState, spec: WorkerSpec) -> Result<(), String> {
    let mut client = state.client.lock().await;
    client.add_worker(&spec).await.map_err(|e| e.to_string())
}

pub async fn start_worker(state: &AppState, name: &str) -> Result<(), String> {
    let mut client = state.client.lock().await;
    client.start_worker(name).await.map_err(|e| e.to_string())
}

pub async fn stop_worker(state: &AppState, name: &str) -> Result<(), String> {
    let mut client = state.client.lock().await;
    client.stop_worker(name).await.map_err(|e| e.to_string())
}

/// The daemon has no restart RPC; compose stop + start.
pub async fn restart_worker(state: &AppState, name: &str) -> Result<(), String> {
    {
        let mut client = state.client.lock().await;
        client.stop_worker(name).await.map_err(|e| e.to_string())?;
    }
    let mut client = state.client.lock().await;
    client.start_worker(name).await.map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Run, confirm PASS**

From `apps/desktop/src-tauri`: `cargo test --test bridge_integration`
Expected: PASS — `daemon_status_reports_connected` + `add_list_start_stop_via_bridge`.

- [ ] **Step 5: Commit**

```bash
cd /Users/adnanhussain/Documents/projects/servicio
git add apps/desktop/src-tauri
git commit -m "feat(gui): bridge commands list/add/start/stop/restart"
```

---

## Task 3: Event forwarding + Tauri command registration (TDD where possible)

**Files:** `apps/desktop/src-tauri/src/{events,main}.rs`, `tests/bridge_integration.rs`.

- [ ] **Step 1: Add a failing test for the event-forwarding source**

The Tauri `emit` needs an `AppHandle`, which isn't available in a plain unit test. So factor the forwarding so the *frame→payload mapping* is a pure, testable function, and only the emit is Tauri-bound. Append to `tests/bridge_integration.rs`:
```rust
use servicio_app::events::event_payload;
use servicio_ipc::Frame;
use serde_json::json;

#[test]
fn maps_state_event_frame_to_payload() {
    let frame = Frame::Event {
        topic: "state".into(),
        payload: json!({"worker":"q","instance":0,"from":"starting","to":"running"}),
    };
    let p = event_payload(&frame).unwrap();
    assert_eq!(p["kind"], "state");
    assert_eq!(p["worker"], "q");
    assert_eq!(p["to"], "running");
}

#[test]
fn non_event_frame_maps_to_none() {
    let frame = Frame::Response { id: 1, result: None, error: None };
    assert!(event_payload(&frame).is_none());
}
```

- [ ] **Step 2: Run, confirm FAIL**

From `apps/desktop/src-tauri`: `cargo test --test bridge_integration maps_state_event`
Expected: FAIL — `no function event_payload`.

- [ ] **Step 3: Implement events.rs**

Replace `apps/desktop/src-tauri/src/events.rs`:
```rust
use serde_json::{json, Value};
use servicio_ipc::Frame;

/// Map a daemon Event frame to the `worker-event` payload the frontend consumes.
/// Returns None for non-event frames.
pub fn event_payload(frame: &Frame) -> Option<Value> {
    match frame {
        Frame::Event { topic, payload } => {
            let mut obj = payload.clone();
            if let Value::Object(map) = &mut obj {
                map.insert("kind".into(), json!(topic));
            }
            Some(obj)
        }
        _ => None,
    }
}

/// Run the subscribe loop: connect a dedicated Client, subscribe to state+log,
/// and call `emit` for each mapped payload until the connection closes.
pub async fn run_event_pump<F>(socket: std::path::PathBuf, token: String, emit: F)
where
    F: Fn(Value) + Send + 'static,
{
    use servicio_cli_lib::Client;
    use tokio::io::AsyncBufReadExt;
    let client = match Client::connect(&socket, &token).await {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut lines = match client.subscribe(&["state", "log"], None).await {
        Ok(l) => l,
        Err(_) => return,
    };
    while let Ok(Some(line)) = lines.next_line().await {
        if let Ok(frame) = Frame::from_line(&line) {
            if let Some(p) = event_payload(&frame) {
                emit(p);
            }
        }
    }
}
```

- [ ] **Step 4: Run, confirm PASS**

From `apps/desktop/src-tauri`: `cargo test --test bridge_integration`
Expected: PASS — including the two mapping tests.

- [ ] **Step 5: Wire main.rs (Tauri commands + startup + event pump)**

Replace `apps/desktop/src-tauri/src/main.rs` with:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use servicio_app::bridge::{self, DaemonStatus};
use servicio_app::events::run_event_pump;
use servicio_app::sidecar::{default_base, ensure_daemon, socket_path};
use servicio_app::state::AppState;
use servicio_core::worker::WorkerSpec;
use servicio_ipc::types::WorkerStatus;
use tauri::{Emitter, Manager};

#[tauri::command]
async fn daemon_status(state: tauri::State<'_, AppState>) -> Result<DaemonStatus, String> {
    bridge::daemon_status(&state).await
}

#[tauri::command]
async fn list_workers(state: tauri::State<'_, AppState>) -> Result<Vec<WorkerStatus>, String> {
    bridge::list_workers(&state).await
}

#[tauri::command]
async fn add_worker(state: tauri::State<'_, AppState>, spec: WorkerSpec) -> Result<(), String> {
    bridge::add_worker(&state, spec).await
}

#[tauri::command]
async fn start_worker(state: tauri::State<'_, AppState>, name: String) -> Result<(), String> {
    bridge::start_worker(&state, &name).await
}

#[tauri::command]
async fn stop_worker(state: tauri::State<'_, AppState>, name: String) -> Result<(), String> {
    bridge::stop_worker(&state, &name).await
}

#[tauri::command]
async fn restart_worker(state: tauri::State<'_, AppState>, name: String) -> Result<(), String> {
    bridge::restart_worker(&state, &name).await
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let base = default_base();
                // The bundled daemon binary is named "servicio-daemon"; in dev it must be on PATH
                // or built. Use the binary name; packaging wires the sidecar path.
                let token = match ensure_daemon(&base, "servicio-daemon").await {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("daemon not ready: {e}");
                        return;
                    }
                };
                let socket = socket_path(&base);
                if let Ok(state) = AppState::connect(&socket, &token).await {
                    handle.manage(state);
                }
                // Event pump on a second connection.
                let emit_handle = handle.clone();
                run_event_pump(socket, token, move |payload| {
                    let _ = emit_handle.emit("worker-event", payload);
                })
                .await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            daemon_status, list_workers, add_worker, start_worker, stop_worker, restart_worker
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```
> Note: `tauri::State` requires the state be managed before a command runs. In this minimal version state is managed after `ensure_daemon`; the frontend should call `daemon_status` with retry until managed (handled in the UI). If `Emitter`/`Manager` import paths differ in the installed Tauri version, adjust imports to match `tauri`'s API (Tauri 2 uses `tauri::Emitter` for `emit`).

- [ ] **Step 6: Build the backend**

From `apps/desktop/src-tauri`: `cargo build`
Expected: compiles. Retry on download timeouts. Report warnings.

- [ ] **Step 7: Commit**

```bash
cd /Users/adnanhussain/Documents/projects/servicio
git add apps/desktop/src-tauri
git commit -m "feat(gui): event forwarding pump + tauri command registration"
```

---

## Task 4: Frontend types + api + Zustand store (TDD)

**Files:** `apps/desktop/src/{types,api,store,store.test}.ts`.

- [ ] **Step 1: Types**

Create `apps/desktop/src/types.ts`:
```ts
export type InstanceState =
  | "stopped" | "starting" | "running" | "stopping" | "crashed" | "backoff" | "failed";

export interface InstanceStatus {
  index: number;
  state: InstanceState;
  restart_count: number;
  pid: number | null;
}
export interface RunModeDaemon { type: "daemon"; concurrency: number }
export type RunMode = RunModeDaemon;

export interface WorkerStatus {
  name: string;
  run_mode: RunMode;
  instances: InstanceStatus[];
}
export interface DaemonStatus {
  connected: boolean;
  version: string;
  uptime_secs: number;
  worker_count: number;
  running_count: number;
}
export interface StateEventPayload {
  kind: "state"; worker: string; instance: number; from: InstanceState; to: InstanceState;
}
export interface LogEventPayload {
  kind: "log"; worker: string; instance: number; stream: string; line: string;
}
export type WorkerEvent = StateEventPayload | LogEventPayload;
```

- [ ] **Step 2: Failing store test**

Create `apps/desktop/src/store.test.ts`:
```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useStore } from "./store";

describe("store", () => {
  beforeEach(() => useStore.getState().reset());

  it("seeds workers from list", () => {
    useStore.getState().setWorkers([
      { name: "q", run_mode: { type: "daemon", concurrency: 1 }, instances: [] },
    ]);
    expect(Object.keys(useStore.getState().workers)).toEqual(["q"]);
  });

  it("applies a state event to the matching instance", () => {
    useStore.getState().setWorkers([
      { name: "q", run_mode: { type: "daemon", concurrency: 1 },
        instances: [{ index: 0, state: "starting", restart_count: 0, pid: null }] },
    ]);
    useStore.getState().applyEvent({ kind: "state", worker: "q", instance: 0, from: "starting", to: "running" });
    expect(useStore.getState().workers["q"].instances[0].state).toBe("running");
  });

  it("appends log lines with a ring-buffer cap", () => {
    const s = useStore.getState();
    for (let i = 0; i < 1100; i++) {
      s.applyEvent({ kind: "log", worker: "q", instance: 0, stream: "stdout", line: `l${i}` });
    }
    const logs = useStore.getState().logs["q"];
    expect(logs.length).toBe(1000);
    expect(logs[logs.length - 1]).toContain("l1099");
  });
});
```

- [ ] **Step 3: Run, confirm FAIL**

From `apps/desktop`: `npm run test`
Expected: FAIL — cannot import `./store`.

- [ ] **Step 4: Implement the store**

Create `apps/desktop/src/store.ts`:
```ts
import { create } from "zustand";
import type { WorkerStatus, WorkerEvent, DaemonStatus } from "./types";

const LOG_CAP = 1000;

interface State {
  workers: Record<string, WorkerStatus>;
  logs: Record<string, string[]>;
  daemon: DaemonStatus | null;
  setWorkers: (list: WorkerStatus[]) => void;
  setDaemon: (d: DaemonStatus) => void;
  applyEvent: (e: WorkerEvent) => void;
  reset: () => void;
}

export const useStore = create<State>((set) => ({
  workers: {},
  logs: {},
  daemon: null,
  setWorkers: (list) =>
    set(() => ({ workers: Object.fromEntries(list.map((w) => [w.name, w])) })),
  setDaemon: (daemon) => set(() => ({ daemon })),
  applyEvent: (e) =>
    set((s) => {
      if (e.kind === "state") {
        const w = s.workers[e.worker];
        if (!w) return {};
        const instances = w.instances.map((i) =>
          i.index === e.instance ? { ...i, state: e.to } : i
        );
        return { workers: { ...s.workers, [e.worker]: { ...w, instances } } };
      } else {
        const prev = s.logs[e.worker] ?? [];
        const next = [...prev, `[${e.stream}] ${e.line}`];
        if (next.length > LOG_CAP) next.splice(0, next.length - LOG_CAP);
        return { logs: { ...s.logs, [e.worker]: next } };
      }
    }),
  reset: () => set(() => ({ workers: {}, logs: {}, daemon: null })),
}));
```

- [ ] **Step 5: Implement api.ts (typed invoke wrappers + event subscription)**

Create `apps/desktop/src/api.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { WorkerStatus, DaemonStatus, WorkerEvent, RunMode } from "./types";
import { useStore } from "./store";

export const api = {
  daemonStatus: () => invoke<DaemonStatus>("daemon_status"),
  listWorkers: () => invoke<WorkerStatus[]>("list_workers"),
  startWorker: (name: string) => invoke<void>("start_worker", { name }),
  stopWorker: (name: string) => invoke<void>("stop_worker", { name }),
  restartWorker: (name: string) => invoke<void>("restart_worker", { name }),
  addWorker: (spec: AddWorkerSpec) => invoke<void>("add_worker", { spec }),
};

export interface AddWorkerSpec {
  name: string;
  command: string;
  args: string[];
  working_dir: string;
  env: Record<string, string>;
  run_mode: RunMode;
  restart: { kind: string; max_retries: number; base_secs: number; max_secs: number; reset_window_secs: number };
  autostart: boolean;
  enabled: boolean;
}

/** Wire daemon events into the store. Call once at app start. */
export async function subscribeEvents() {
  await listen<WorkerEvent>("worker-event", (ev) => {
    useStore.getState().applyEvent(ev.payload);
  });
}
```

- [ ] **Step 6: Run, confirm PASS**

From `apps/desktop`: `npm run test`
Expected: PASS — 3 store tests. (`api.ts` isn't unit-tested directly; it's covered by component tests with mocked tauri.)

- [ ] **Step 7: Commit**

```bash
cd /Users/adnanhussain/Documents/projects/servicio
git add apps/desktop/src
git commit -m "feat(gui): frontend types, api wrappers, zustand store + tests"
```

---

## Task 5: Dashboard + WorkerCard (TDD)

**Files:** `apps/desktop/src/components/{Dashboard,WorkerCard,Dashboard.test}.tsx`. Implementer SHOULD invoke `frontend-design` for visual quality.

- [ ] **Step 1: Failing component test**

Create `apps/desktop/src/components/Dashboard.test.tsx`:
```tsx
import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { useStore } from "../store";
import { Dashboard } from "./Dashboard";

vi.mock("../api", () => ({
  api: { startWorker: vi.fn(), stopWorker: vi.fn() },
}));

describe("Dashboard", () => {
  beforeEach(() => useStore.getState().reset());

  it("renders a card per worker with its status", () => {
    useStore.getState().setWorkers([
      { name: "queue", run_mode: { type: "daemon", concurrency: 2 },
        instances: [{ index: 0, state: "running", restart_count: 0, pid: 10 },
                    { index: 1, state: "running", restart_count: 0, pid: 11 }] },
      { name: "img", run_mode: { type: "daemon", concurrency: 1 },
        instances: [{ index: 0, state: "crashed", restart_count: 5, pid: null }] },
    ]);
    render(<Dashboard onOpen={() => {}} onAdd={() => {}} />);
    expect(screen.getByText("queue")).toBeDefined();
    expect(screen.getByText("img")).toBeDefined();
    // crashed worker shows a crashed indicator
    expect(screen.getByText(/crashed/i)).toBeDefined();
  });
});
```
Add `apps/desktop/src/setupTests.ts` with `import "@testing-library/jest-dom";` and reference it from `vitest.config.ts` `setupFiles: ["./src/setupTests.ts"]`.

- [ ] **Step 2: Run, confirm FAIL**, then implement `WorkerCard.tsx` and `Dashboard.tsx`.

Create `apps/desktop/src/components/WorkerCard.tsx`:
```tsx
import type { WorkerStatus, InstanceState } from "../types";

const DOT: Record<InstanceState, string> = {
  running: "bg-green-500", starting: "bg-amber-400", backoff: "bg-amber-400",
  stopping: "bg-amber-400", stopped: "bg-slate-400", crashed: "bg-red-500", failed: "bg-red-600",
};

function worstState(w: WorkerStatus): InstanceState {
  const order: InstanceState[] = ["failed", "crashed", "backoff", "starting", "stopping", "running", "stopped"];
  for (const s of order) if (w.instances.some((i) => i.state === s)) return s;
  return "stopped";
}

export function WorkerCard({
  w, onOpen, onStart, onStop,
}: { w: WorkerStatus; onOpen: () => void; onStart: () => void; onStop: () => void }) {
  const state = worstState(w);
  const restarts = w.instances.reduce((n, i) => n + i.restart_count, 0);
  const running = w.instances.filter((i) => i.state === "running").length;
  return (
    <div onClick={onOpen}
      className="cursor-pointer rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4 shadow-sm hover:shadow transition">
      <div className="flex items-center gap-2">
        <span className={`h-2.5 w-2.5 rounded-full ${DOT[state]}`} />
        <span className="font-semibold flex-1 truncate">{w.name}</span>
      </div>
      <div className="mt-1 text-xs opacity-60">daemon ×{w.run_mode.concurrency}</div>
      <div className="mt-2 text-sm">{state} · {running}/{w.instances.length} up · {restarts} restarts</div>
      <div className="mt-3 flex gap-2" onClick={(e) => e.stopPropagation()}>
        <button className="text-xs rounded bg-green-600 text-white px-2 py-1" onClick={onStart}>Start</button>
        <button className="text-xs rounded bg-slate-200 dark:bg-slate-700 px-2 py-1" onClick={onStop}>Stop</button>
      </div>
    </div>
  );
}
```

Create `apps/desktop/src/components/Dashboard.tsx`:
```tsx
import { useStore } from "../store";
import { api } from "../api";
import { WorkerCard } from "./WorkerCard";

export function Dashboard({ onOpen, onAdd }: { onOpen: (name: string) => void; onAdd: () => void }) {
  const workers = Object.values(useStore((s) => s.workers));
  const running = workers.filter((w) => w.instances.some((i) => i.state === "running")).length;
  const crashed = workers.filter((w) => w.instances.some((i) => i.state === "crashed" || i.state === "failed")).length;
  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-4">
        <div className="flex gap-2 text-xs">
          <span className="rounded-full bg-green-100 text-green-800 px-2 py-0.5">{running} running</span>
          <span className="rounded-full bg-red-100 text-red-800 px-2 py-0.5">{crashed} crashed</span>
        </div>
        <button className="rounded bg-blue-600 text-white text-sm px-3 py-1.5" onClick={onAdd}>+ New worker</button>
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
        {workers.map((w) => (
          <WorkerCard key={w.name} w={w}
            onOpen={() => onOpen(w.name)}
            onStart={() => api.startWorker(w.name)}
            onStop={() => api.stopWorker(w.name)} />
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Run, confirm PASS**

From `apps/desktop`: `npm run test`
Expected: store + Dashboard tests PASS.

- [ ] **Step 4: Commit**

```bash
cd /Users/adnanhussain/Documents/projects/servicio
git add apps/desktop/src
git commit -m "feat(gui): dashboard card grid + worker cards"
```

---

## Task 6: Worker detail + LogView (TDD)

**Files:** `apps/desktop/src/components/{WorkerDetail,LogView,LogView.test}.tsx`.

- [ ] **Step 1: Failing test for LogView**

Create `apps/desktop/src/components/LogView.test.tsx`:
```tsx
import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useStore } from "../store";
import { LogView } from "./LogView";

describe("LogView", () => {
  beforeEach(() => useStore.getState().reset());
  it("renders buffered log lines for the worker", () => {
    useStore.getState().applyEvent({ kind: "log", worker: "q", instance: 0, stream: "stdout", line: "hello world" });
    render(<LogView worker="q" />);
    expect(screen.getByText(/hello world/)).toBeDefined();
  });
});
```

- [ ] **Step 2: Implement LogView + WorkerDetail**

Create `apps/desktop/src/components/LogView.tsx`:
```tsx
import { useEffect, useRef } from "react";
import { useStore } from "../store";

export function LogView({ worker }: { worker: string }) {
  const lines = useStore((s) => s.logs[worker] ?? []);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => { ref.current?.scrollTo(0, ref.current.scrollHeight); }, [lines.length]);
  return (
    <div ref={ref} className="h-80 overflow-auto rounded-lg bg-slate-950 text-slate-200 font-mono text-xs p-3">
      {lines.length === 0 ? <div className="opacity-50">No logs yet…</div>
        : lines.map((l, i) => <div key={i} className="whitespace-pre-wrap">{l}</div>)}
    </div>
  );
}
```

Create `apps/desktop/src/components/WorkerDetail.tsx`:
```tsx
import { useStore } from "../store";
import { api } from "../api";
import { LogView } from "./LogView";
import { useState } from "react";

export function WorkerDetail({ name, onBack }: { name: string; onBack: () => void }) {
  const w = useStore((s) => s.workers[name]);
  const [tab, setTab] = useState<"logs" | "config">("logs");
  if (!w) return <div className="p-6">Worker not found. <button onClick={onBack} className="underline">Back</button></div>;
  const restarts = w.instances.reduce((n, i) => n + i.restart_count, 0);
  return (
    <div className="p-6">
      <button onClick={onBack} className="text-sm underline mb-3">← Back</button>
      <div className="flex items-center gap-3 mb-4">
        <h2 className="text-xl font-semibold">{name}</h2>
        <span className="text-xs opacity-60">daemon ×{w.run_mode.concurrency} · {restarts} restarts</span>
        <span className="flex-1" />
        <button className="rounded bg-green-600 text-white text-sm px-3 py-1.5" onClick={() => api.startWorker(name)}>Start</button>
        <button className="rounded bg-slate-200 dark:bg-slate-700 text-sm px-3 py-1.5" onClick={() => api.stopWorker(name)}>Stop</button>
        <button className="rounded bg-slate-200 dark:bg-slate-700 text-sm px-3 py-1.5" onClick={() => api.restartWorker(name)}>Restart</button>
      </div>
      <div className="flex gap-4 border-b border-slate-200 dark:border-slate-800 mb-3 text-sm">
        <button className={tab === "logs" ? "border-b-2 border-blue-600 pb-1" : "pb-1 opacity-60"} onClick={() => setTab("logs")}>Logs</button>
        <button className={tab === "config" ? "border-b-2 border-blue-600 pb-1" : "pb-1 opacity-60"} onClick={() => setTab("config")}>Config</button>
      </div>
      {tab === "logs" ? <LogView worker={name} /> : (
        <pre className="text-xs bg-slate-100 dark:bg-slate-900 rounded p-3 overflow-auto">
          {JSON.stringify(w, null, 2)}
        </pre>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Run, confirm PASS**, then commit.

From `apps/desktop`: `npm run test` → PASS.
```bash
cd /Users/adnanhussain/Documents/projects/servicio
git add apps/desktop/src && git commit -m "feat(gui): worker detail view + live log tab"
```

---

## Task 7: Add-worker form (TDD)

**Files:** `apps/desktop/src/components/{AddWorkerForm,AddWorkerForm.test}.tsx`.

- [ ] **Step 1: Failing test (form builds the correct payload)**

Create `apps/desktop/src/components/AddWorkerForm.test.tsx`:
```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { AddWorkerForm } from "./AddWorkerForm";

describe("AddWorkerForm", () => {
  it("submits a well-formed add_worker spec", () => {
    const onSubmit = vi.fn();
    render(<AddWorkerForm onSubmit={onSubmit} onCancel={() => {}} />);
    fireEvent.change(screen.getByLabelText(/name/i), { target: { value: "q" } });
    fireEvent.change(screen.getByLabelText(/command/i), { target: { value: "php" } });
    fireEvent.change(screen.getByLabelText(/args/i), { target: { value: "artisan queue:work" } });
    fireEvent.change(screen.getByLabelText(/working dir/i), { target: { value: "/srv/app" } });
    fireEvent.click(screen.getByText(/create/i));
    expect(onSubmit).toHaveBeenCalledTimes(1);
    const spec = onSubmit.mock.calls[0][0];
    expect(spec.name).toBe("q");
    expect(spec.command).toBe("php");
    expect(spec.args).toEqual(["artisan", "queue:work"]);
    expect(spec.run_mode).toEqual({ type: "daemon", concurrency: 1 });
    expect(spec.enabled).toBe(true);
  });
});
```

- [ ] **Step 2: Implement AddWorkerForm**

Create `apps/desktop/src/components/AddWorkerForm.tsx`:
```tsx
import { useState } from "react";
import type { AddWorkerSpec } from "../api";

export function AddWorkerForm({
  onSubmit, onCancel,
}: { onSubmit: (spec: AddWorkerSpec) => void; onCancel: () => void }) {
  const [name, setName] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [dir, setDir] = useState("");
  const [concurrency, setConcurrency] = useState(1);
  const [maxRetries, setMaxRetries] = useState(5);
  const [autostart, setAutostart] = useState(true);

  function submit() {
    if (!name || !command) return;
    onSubmit({
      name,
      command,
      args: args.trim() ? args.trim().split(/\s+/) : [],
      working_dir: dir || ".",
      env: {},
      run_mode: { type: "daemon", concurrency },
      restart: { kind: "on_failure", max_retries: maxRetries, base_secs: 1, max_secs: 60, reset_window_secs: 30 },
      autostart,
      enabled: true,
    });
  }

  const field = "w-full rounded border border-slate-300 dark:border-slate-700 bg-transparent px-2 py-1 text-sm";
  return (
    <div className="p-6 max-w-lg">
      <h2 className="text-xl font-semibold mb-4">New worker</h2>
      <label className="block text-xs mb-2">Name<input className={field} value={name} onChange={(e) => setName(e.target.value)} /></label>
      <label className="block text-xs mb-2">Command<input className={field} value={command} onChange={(e) => setCommand(e.target.value)} /></label>
      <label className="block text-xs mb-2">Args (space-separated)<input className={field} value={args} onChange={(e) => setArgs(e.target.value)} /></label>
      <label className="block text-xs mb-2">Working dir<input className={field} value={dir} onChange={(e) => setDir(e.target.value)} /></label>
      <label className="block text-xs mb-2">Concurrency<input type="number" min={1} className={field} value={concurrency} onChange={(e) => setConcurrency(+e.target.value)} /></label>
      <label className="block text-xs mb-2">Max retries<input type="number" min={0} className={field} value={maxRetries} onChange={(e) => setMaxRetries(+e.target.value)} /></label>
      <label className="flex items-center gap-2 text-xs mb-4"><input type="checkbox" checked={autostart} onChange={(e) => setAutostart(e.target.checked)} /> Autostart on daemon boot</label>
      <div className="flex gap-2">
        <button className="rounded bg-blue-600 text-white text-sm px-3 py-1.5" onClick={submit}>Create</button>
        <button className="rounded bg-slate-200 dark:bg-slate-700 text-sm px-3 py-1.5" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}
```
> Note: the Tauri folder-picker dialog (`@tauri-apps/plugin-dialog`) is optional polish; a plain text path field is sufficient for 2b and keeps the test runtime-free. If adding the dialog plugin, gate it so tests (no Tauri runtime) still pass.

- [ ] **Step 3: Run, confirm PASS**, then commit.

From `apps/desktop`: `npm run test` → PASS.
```bash
cd /Users/adnanhussain/Documents/projects/servicio
git add apps/desktop/src && git commit -m "feat(gui): add-worker form"
```

---

## Task 8: App shell wiring + status footer + theme

**Files:** `apps/desktop/src/{App,main}.tsx`, `components/{Sidebar,StatusFooter}.tsx`.

- [ ] **Step 1: Implement the shell**

Create `apps/desktop/src/components/StatusFooter.tsx`:
```tsx
import { useStore } from "../store";

export function StatusFooter() {
  const daemon = useStore((s) => s.daemon);
  const ok = daemon?.connected;
  return (
    <div className="flex items-center gap-2 px-3 py-2 text-xs border-t border-slate-200 dark:border-slate-800">
      <span className={`h-2 w-2 rounded-full ${ok ? "bg-green-500" : "bg-red-500"}`} />
      <span>{ok ? `daemon ${daemon?.version} · ${daemon?.running_count} running` : "daemon not connected"}</span>
    </div>
  );
}
```

Create `apps/desktop/src/components/Sidebar.tsx`:
```tsx
export function Sidebar({ view, setView }: { view: string; setView: (v: string) => void }) {
  const item = (id: string, label: string) =>
    <button onClick={() => setView(id)}
      className={`text-left px-3 py-2 rounded ${view === id ? "bg-slate-200 dark:bg-slate-800" : "opacity-70"}`}>{label}</button>;
  return (
    <nav className="w-44 shrink-0 p-3 flex flex-col gap-1 border-r border-slate-200 dark:border-slate-800">
      <div className="font-bold mb-3 px-3">⚙ servicio</div>
      {item("dashboard", "▦ Dashboard")}
      {item("settings", "⚙ Settings")}
    </nav>
  );
}
```

Replace `apps/desktop/src/App.tsx`:
```tsx
import { useEffect, useState } from "react";
import { useStore } from "./store";
import { api, subscribeEvents } from "./api";
import { Sidebar } from "./components/Sidebar";
import { StatusFooter } from "./components/StatusFooter";
import { Dashboard } from "./components/Dashboard";
import { WorkerDetail } from "./components/WorkerDetail";
import { AddWorkerForm } from "./components/AddWorkerForm";

export default function App() {
  const [view, setView] = useState("dashboard");
  const [detail, setDetail] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  useEffect(() => {
    subscribeEvents();
    let alive = true;
    const tick = async () => {
      try {
        const d = await api.daemonStatus();
        useStore.getState().setDaemon(d);
        useStore.getState().setWorkers(await api.listWorkers());
      } catch { /* daemon not ready yet */ }
      if (alive) setTimeout(tick, 2000);
    };
    tick();
    return () => { alive = false; };
  }, []);

  return (
    <div className="h-screen flex flex-col">
      <div className="flex-1 flex overflow-hidden">
        <Sidebar view={view} setView={setView} />
        <main className="flex-1 overflow-auto">
          {adding ? (
            <AddWorkerForm
              onSubmit={async (spec) => { await api.addWorker(spec); setAdding(false); }}
              onCancel={() => setAdding(false)} />
          ) : detail ? (
            <WorkerDetail name={detail} onBack={() => setDetail(null)} />
          ) : (
            <Dashboard onOpen={setDetail} onAdd={() => setAdding(true)} />
          )}
        </main>
      </div>
      <StatusFooter />
    </div>
  );
}
```
Ensure `apps/desktop/src/main.tsx` renders `<App/>` and imports `./index.css` (the Vite template does; adjust if it imports a different root component).

- [ ] **Step 2: Frontend tests + build**

From `apps/desktop`: `npm run test` (all component + store tests pass) and `npm run build` (Vite build clean).

- [ ] **Step 3: Commit**

```bash
cd /Users/adnanhussain/Documents/projects/servicio
git add apps/desktop/src && git commit -m "feat(gui): app shell, sidebar, status footer, view routing"
```

---

## Task 9: README + manual smoke

**Files:** `README.md`, `.gitignore`.

- [ ] **Step 1: gitignore**

Ensure `.gitignore` contains:
```
apps/desktop/node_modules/
apps/desktop/dist/
apps/desktop/src-tauri/target/
```

- [ ] **Step 2: README — add a Phase 2b section**

Add under the status list in `README.md`:
```markdown
- **Phase 2b (done):** minimal Tauri desktop GUI — auto-spawns the daemon sidecar,
  card-grid dashboard with live status, worker detail + live logs, start/stop/restart,
  and a simple add-worker form.

### Run the GUI (dev)

The GUI spawns the `servicio-daemon` binary as a sidecar; build it first so it is on PATH:

```bash
cargo build -p servicio-daemon          # provides target/debug/servicio-daemon
cd apps/desktop
npm install
PATH="$PWD/../../target/debug:$PATH" npm run tauri dev
```
The window opens, the daemon auto-starts under `$XDG_RUNTIME_DIR/servicio` (or a temp dir),
and the dashboard shows workers live. Add one with "+ New worker".
```

- [ ] **Step 3: Manual smoke (document the result)**

Build the daemon, run `npm run tauri dev` with the daemon dir on PATH, add a worker
(`name=ticker`, `command=sh`, `args=-c "while true; do echo tick; sleep 1; done"`),
confirm it appears running on the dashboard, open detail, see live `tick` logs, Stop it.
Record that this worked (or any issue).

- [ ] **Step 4: Commit**

```bash
git add README.md .gitignore && git commit -m "docs(gui): phase 2b readme + run instructions"
```

---

## Definition of Done (Phase 2b)
- `apps/desktop/src-tauri` Rust bridge tests pass against a real in-process daemon
  (`cargo test` in that crate): `daemon_status`, add/list/start/stop/restart, event mapping.
- Frontend `npm run test` green: store ring-buffer + event apply, dashboard cards, log view,
  add-form payload.
- `npm run build` (Vite) and `cargo build` (backend) succeed.
- Manual smoke documented: daemon auto-spawns, dashboard live, logs stream, controls work.
- Engine workspace `cargo test` is unaffected (Tauri crate is standalone).

## Out of scope (Phase 2c+)
- 4-step creation wizard, framework autodetect UI, folder-picker polish.
- Metrics graphs, native notifications, Events/audit tab.
- OS-service install, signing/notarization, installers, survive-GUI-close daemon.
- Full E2E (tauri-driver/Playwright).

## Deferred to Phase 3 (from final review)

All 10 tasks implemented; backend bridge tests (vs real daemon) + 6 Vitest tests pass, both
builds clean, engine workspace unaffected (53 tests, Tauri crate standalone). No Critical
issues. The `tauri dev` window render + live emit/listen round-trip is the only piece not
auto-verifiable headless (needs a manual launch). Deferred (out of 2b scope, Phase 3):
- **User-visible error feedback.** A permanent `ensure_daemon` failure leaves a live but
  inert window (only the footer dot signals it); in-window command errors are swallowed by
  fire-and-forget `invoke`. Add a toast/error surface + retry `ensure_daemon`.
- **Sidecar lifecycle:** spawned daemon is detached (orphans after GUI close — matches the
  deferred "survive-close" decision) and resolved by bare PATH name in dev. Packaging phase
  wires the bundled sidecar path + a managed lifecycle.
- **Event/seed ordering:** a `state` event for a worker not yet seeded by `list_workers` is
  dropped (reconciled within the 2s poll); fine for 2b, revisit if the poll is removed.
- **Test gaps:** event-pump end-to-end, sidecar spawn, command-before-state-managed,
  frontend `subscribeEvents` listener wiring.
- Cosmetic: `tauri.conf.json` `identifier` is the default `com.tauri.dev` (fix at packaging).

## Self-review notes
- **Spec coverage:** architecture/bridge (§3–4) → Tasks 1–3; frontend stack/screens (§5) →
  Tasks 4–8; testing (§6) → embedded; build/sidecar (§3,§7) → Tasks 0,1,3,9. Dashboard,
  detail+logs, add-form, status footer all present.
- **Type consistency:** Rust `bridge::{daemon_status,list_workers,add_worker,start_worker,
  stop_worker,restart_worker}` + `AppState::connect` + `events::{event_payload,run_event_pump}`
  + `sidecar::{default_base,socket_path,ensure_daemon}` used consistently across tasks and in
  `main.rs`. TS `WorkerStatus`/`InstanceStatus`/`WorkerEvent`/`DaemonStatus`/`AddWorkerSpec`
  align with the ipc JSON and the store/components. The `add_worker` payload field names
  (`run_mode`, `working_dir`, `restart`, `autostart`, `enabled`) match `servicio-core::WorkerSpec`'s
  serde shape (snake_case, `run_mode` tagged `type:"daemon"`).
- **Known risk:** Tauri 2 API import paths (`Emitter`/`Manager`/`@tauri-apps/api/core`) can vary
  by exact version; Task 3/4 note to adjust to the installed version. Network-heavy installs may
  need retries (documented up top).
