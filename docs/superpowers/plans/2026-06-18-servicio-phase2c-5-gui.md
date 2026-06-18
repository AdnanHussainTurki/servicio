# Servicio Phase 2c.5 — GUI (detect→wizard, metrics graphs, notifications)

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. New visual components MUST be built with the `frontend-design` skill, matching the existing 2b "control-room" aesthetic (dark sidebar, copper `signal` accent, signal status palette, JetBrains-Mono metrics/logs). Checkbox steps.

**Goal:** Wire the engine's new capabilities into the desktop GUI: a folder-scan **Detect** step feeding a **4-step creation wizard** (Command → Mode → Recovery → Review, with Daemon/Scheduled/Batch), a **Metrics tab** with live cpu/mem graphs on the worker detail, and **native notifications** on crash/crash-loop/recovery.

**Architecture:** Backend bridge gains `detect_workers` + `metrics` commands and forwards the `metric` event topic to the frontend. Frontend gains types/api/store for suggestions + metrics, a Detect→Wizard create flow replacing the simple add form, a metrics graph component, and the Tauri notification plugin wired to state events.

**Tech Stack:** Tauri 2.11, React 19 + TS + Vite + Tailwind + Zustand, `@tauri-apps/plugin-notification`. Tests: Rust bridge integration vs real daemon; Vitest + Testing Library (Tauri mocked).

**Builds on:** 2c.1/2c.2/2c.3 (merged) + 2b GUI. Spec: `docs/superpowers/specs/2026-06-18-servicio-phase2c-design.md` §8.

---

## Task 1: backend bridge — detect + metrics commands + metric forwarding (TDD)
**Files:** `apps/desktop/src-tauri/src/{bridge,events,main}.rs`, `tests/bridge_integration.rs`

- [ ] **Step 1 — failing integration tests.** Append to `apps/desktop/src-tauri/tests/bridge_integration.rs`:
```rust
#[tokio::test]
async fn detect_workers_via_bridge_finds_generic() {
    let dir = tempfile::tempdir().unwrap();
    let (_p, handle, state) = running_daemon(dir.path()).await;
    let proj = dir.path().join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let suggestions = bridge::detect_workers(&state, proj.to_str().unwrap()).await.unwrap();
    assert!(suggestions.as_array().unwrap().iter().any(|s| s["source"] == "generic"));
    handle.shutdown().await;
}

#[tokio::test]
async fn metrics_via_bridge_returns_array() {
    let dir = tempfile::tempdir().unwrap();
    let (_p, handle, state) = running_daemon(dir.path()).await;
    let v = bridge::metrics(&state, "nope", 900).await.unwrap();
    assert!(v.is_array()); // empty array for unknown worker, but a valid response
    handle.shutdown().await;
}
```
- [ ] **Step 2 — run, FAIL.**
- [ ] **Step 3 — implement bridge fns.** Add to `apps/desktop/src-tauri/src/bridge.rs`:
```rust
pub async fn detect_workers(state: &AppState, path: &str) -> Result<serde_json::Value, String> {
    let mut client = state.client.lock().await;
    client.detect(path).await.map_err(|e| e.to_string())
}

pub async fn metrics(state: &AppState, worker: &str, since_secs: u64) -> Result<serde_json::Value, String> {
    let mut client = state.client.lock().await;
    client.metrics(worker, since_secs).await.map_err(|e| e.to_string())
}
```
(`Client::detect` and `Client::metrics` exist from 2c.2/2c.3.)
- [ ] **Step 4 — forward metric events.** In `apps/desktop/src-tauri/src/events.rs` `run_event_pump`, change the subscribe topics to include metric:
```rust
    let mut lines = match client.subscribe(&["state", "log", "metric"], None).await {
```
(`event_payload` already injects `kind: topic`, so a metric event becomes `{kind:"metric",...}` for the frontend.)
- [ ] **Step 5 — register tauri commands.** In `apps/desktop/src-tauri/src/main.rs`, add commands + register them:
```rust
#[tauri::command]
async fn detect_workers(state: tauri::State<'_, AppState>, path: String) -> Result<serde_json::Value, String> {
    bridge::detect_workers(&state, &path).await
}

#[tauri::command]
async fn metrics(state: tauri::State<'_, AppState>, worker: String, since_secs: u64) -> Result<serde_json::Value, String> {
    bridge::metrics(&state, &worker, since_secs).await
}
```
Add `detect_workers, metrics` to the `tauri::generate_handler![...]` list.
- [ ] **Step 6 — verify.** `cd apps/desktop/src-tauri && cargo test --test bridge_integration` → PASS. `cargo build` clean.
- [ ] **Step 7 — commit:** `git add apps/desktop/src-tauri && git commit -m "feat(gui): bridge detect_workers + metrics commands + metric event forward"`

---

## Task 2: frontend types + api + store (TDD)
**Files:** `apps/desktop/src/{types,api,store,store.test}.ts`

- [ ] **Step 1 — types.** Add to `apps/desktop/src/types.ts`:
```ts
export type RunModeAny =
  | { type: "daemon"; concurrency: number }
  | { type: "scheduled"; schedule: { cron: string } | { interval_secs: number }; overlap: "skip" | "queue" | "kill_previous" }
  | { type: "batch"; run_count: number; delay_secs: number };

export interface SuggestionDraft {
  label: string; source: string; name: string;
  command: string; args: string[]; working_dir: string; run_mode: RunModeAny;
}
export interface MetricPointT { ts: number; cpu: number; mem: number }
export interface MetricEventPayload { kind: "metric"; worker: string; instance: number; ts: number; cpu: number; mem: number }
```
Extend the existing `WorkerEvent` union to include `MetricEventPayload`.
- [ ] **Step 2 — store metric handling (TDD).** Add to `apps/desktop/src/store.test.ts`:
```ts
  it("buffers metric samples per worker with a cap", () => {
    for (let i = 0; i < 250; i++) {
      useStore.getState().applyEvent({ kind: "metric", worker: "q", instance: 0, ts: i, cpu: 1.0, mem: 100 });
    }
    const m = useStore.getState().metrics["q"];
    expect(m.length).toBe(200);
    expect(m[m.length - 1].ts).toBe(249);
  });
```
- [ ] **Step 3 — implement store.** In `apps/desktop/src/store.ts`: add `metrics: Record<string, MetricPointT[]>` to state (init `{}`, include in `reset()`), and in `applyEvent` handle `e.kind === "metric"`:
```ts
      } else if (e.kind === "metric") {
        const prev = s.metrics[e.worker] ?? [];
        const next = [...prev, { ts: e.ts, cpu: e.cpu, mem: e.mem }];
        const CAP = 200;
        if (next.length > CAP) next.splice(0, next.length - CAP);
        return { metrics: { ...s.metrics, [e.worker]: next } };
      }
```
(import `MetricPointT` from types.) Keep state/log handling unchanged.
- [ ] **Step 4 — api wrappers.** Add to `apps/desktop/src/api.ts` `api` object:
```ts
  detectWorkers: (path: string) => invoke<SuggestionDraft[]>("detect_workers", { path }),
  metrics: (worker: string, sinceSecs: number) => invoke<{ instance: number; points: MetricPointT[] }[]>("metrics", { worker, sinceSecs }),
```
(import `SuggestionDraft, MetricPointT` from types. Note the tauri arg name is `sinceSecs` — Tauri maps camelCase JS args to snake_case Rust params, so the Rust `since_secs` param matches `sinceSecs`. Confirm during the GUI run; if not, use `since_secs` as the key.)
- [ ] **Step 5 — verify.** `cd apps/desktop && npm run test` → all pass (incl. new metric buffer test). `npm run build` clean.
- [ ] **Step 6 — commit:** `git add apps/desktop/src && git commit -m "feat(gui): frontend suggestion/metric types, store metric buffer, api wrappers"`

---

## Task 3: Detect → Wizard create flow (frontend-design, TDD)
**Files:** `apps/desktop/src/components/{CreateFlow,WizardMode}.tsx` + tests; `App.tsx` wiring

INVOKE the `frontend-design` skill before building these visual components; match the 2b aesthetic.

- [ ] **Step 1 — failing tests.** Create `apps/desktop/src/components/CreateFlow.test.tsx`:
```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { CreateFlow } from "./CreateFlow";

vi.mock("../api", () => ({
  api: {
    detectWorkers: vi.fn().mockResolvedValue([
      { label: "Custom worker", source: "generic", name: "", command: "", args: [], working_dir: "/p", run_mode: { type: "daemon", concurrency: 1 } },
    ]),
    addWorker: vi.fn().mockResolvedValue(undefined),
  },
}));

describe("CreateFlow", () => {
  it("scans a path and lists suggestions", async () => {
    render(<CreateFlow onDone={() => {}} onCancel={() => {}} />);
    fireEvent.change(screen.getByLabelText(/folder/i), { target: { value: "/p" } });
    fireEvent.click(screen.getByText(/scan/i));
    expect(await screen.findByText(/custom worker/i)).toBeDefined();
  });
});
```
- [ ] **Step 2 — implement CreateFlow + WizardMode.** Build a component that:
  - **Detect step:** a labelled "Folder" text input + a "Scan" button → calls `api.detectWorkers(path)` → renders each `SuggestionDraft` as a selectable row (checkbox + label + command + run-mode chip) + a "Start from scratch" option. A "Continue" advances selected suggestions into the wizard.
  - **Wizard:** for the selected draft(s), Command → Mode → Recovery → Review. The **Mode** step (factor into `WizardMode.tsx`) has Daemon/Scheduled/Batch tabs swapping option fields (concurrency / cron+interval+overlap / run_count+delay). Review shows the resulting `WorkerSpec`-shaped object; confirm calls `api.addWorker(spec)`.
  - Build the spec to match `servicio-core::WorkerSpec` serde shape (snake_case fields: `name, command, args, working_dir, env:{}, run_mode, restart, autostart, enabled`). For Scheduled `run_mode`: `{ type:"scheduled", schedule:{ cron:"..." } | { interval_secs:N }, overlap:"skip" }`. For Batch: `{ type:"batch", run_count:N, delay_secs:N }`.
  - Match 2b styling (frontend-design). Keep the `getByLabelText(/folder/i)`, `/scan/i`, and the suggestion label text assertions working.
- [ ] **Step 3 — wire into App.tsx.** Replace the existing `AddWorkerForm` usage in `App.tsx` with `CreateFlow` (the "+ New worker" / "Add" actions open `CreateFlow`); keep `AddWorkerForm` file for now (or remove if unused — App must compile). On `onDone`, refresh the worker list.
- [ ] **Step 4 — verify.** `npm run test` (CreateFlow + existing pass), `npm run build` clean.
- [ ] **Step 5 — commit:** `git add apps/desktop/src && git commit -m "feat(gui): detect-to-wizard create flow (frontend-design)"`

---

## Task 4: Metrics tab graphs (frontend-design, TDD)
**Files:** `apps/desktop/src/components/{MetricsTab,Sparkline}.tsx` + test; `WorkerDetail.tsx`

INVOKE `frontend-design`; terminal/instrument aesthetic for the graphs (SVG sparklines, no heavy chart lib).

- [ ] **Step 1 — failing test.** Create `apps/desktop/src/components/MetricsTab.test.tsx`:
```tsx
import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { useStore } from "../store";
import { MetricsTab } from "./MetricsTab";

vi.mock("../api", () => ({ api: { metrics: vi.fn().mockResolvedValue([]) } }));

describe("MetricsTab", () => {
  beforeEach(() => useStore.getState().reset());
  it("shows current cpu/mem from the latest sample", () => {
    useStore.getState().applyEvent({ kind: "metric", worker: "q", instance: 0, ts: 1, cpu: 4.2, mem: 1048576 });
    render(<MetricsTab worker="q" />);
    expect(screen.getByText(/4\.2/)).toBeDefined();      // cpu %
    expect(screen.getByText(/1(\.0)? ?MB/i)).toBeDefined(); // mem formatted (1 MiB)
  });
});
```
- [ ] **Step 2 — implement.** `Sparkline.tsx` = a small SVG line chart from `number[]`. `MetricsTab.tsx`:
  - on mount, `api.metrics(worker, 900)` to seed `useStore.metrics[worker]` (merge points), then rely on live `metric` events already flowing into the store.
  - render current cpu% + mem (formatted, e.g. bytes→MB) tiles + two `Sparkline`s (cpu, mem) from the buffered points + a window note.
  - Match 2b aesthetic.
- [ ] **Step 3 — wire into WorkerDetail.** Add a "Metrics" tab between Logs and Config in `WorkerDetail.tsx` that renders `<MetricsTab worker={name} />`.
- [ ] **Step 4 — verify.** `npm run test` + `npm run build`.
- [ ] **Step 5 — commit:** `git add apps/desktop/src && git commit -m "feat(gui): metrics tab with live cpu/mem sparklines (frontend-design)"`

---

## Task 5: native notifications + verify render
**Files:** `apps/desktop/package.json`, `src-tauri/Cargo.toml`, `src-tauri/src/main.rs`, `src-tauri/capabilities/default.json`, `apps/desktop/src/{api,App}.tsx`

- [ ] **Step 1 — add the plugin.** From `apps/desktop`: `npm install @tauri-apps/plugin-notification`. In `src-tauri/Cargo.toml` `[dependencies]`: `tauri-plugin-notification = "2"`. In `src-tauri/src/main.rs` builder chain add `.plugin(tauri_plugin_notification::init())`. In `src-tauri/capabilities/default.json` add `"notification:default"` to the permissions list. (Retry npm/cargo on network timeout.)
- [ ] **Step 2 — notify on transitions.** In `apps/desktop/src/api.ts` (or a small `notify.ts`), add a helper that, given a `StateEventPayload`, sends a native notification when `to` is `crashed` or `failed`, or when recovering (`from` in [crashed,backoff,failed] and `to === running`). Use `@tauri-apps/plugin-notification`'s `sendNotification` (request permission once via `isPermissionGranted`/`requestPermission`). Call it from the existing `worker-event` listener in `subscribeEvents` for `state` events. Guard so a denied/unavailable permission never throws.
- [ ] **Step 3 — verify build + render.** `npm run test` + `npm run build` + `cd src-tauri && cargo build`. Then start the dev server (`npm run dev`) and, if a Playwright/browser tool is available, navigate to the printed localhost URL, confirm the app still renders (shell + dashboard, no white screen, 0 console errors beyond the known non-Tauri warnings), and screenshot. Report the result. (The actual native notification only fires in the real Tauri window — document as manual.)
- [ ] **Step 4 — commit:** `git add apps/desktop && git commit -m "feat(gui): native notifications on crash/recovery"`

---

## Task 6: README + final verify
**Files:** `README.md`

- [ ] **Step 1 — README.** Update the status list: Phase 2c done (run modes, metrics, autodetect, wizard, notifications). Note `servicio detect <path>` / `servicio metrics <name>` CLI.
- [ ] **Step 2 — full verify.** Root `cargo test` (engine unaffected), `cd apps/desktop && npm run test && npm run build`, `cd src-tauri && cargo test && cargo build`. All green.
- [ ] **Step 3 — commit:** `git add README.md && git commit -m "docs: phase 2c complete (run modes, metrics, autodetect, wizard, notifications)"`

---

## Definition of Done (2c.5 / Phase 2c)
- Bridge `detect_workers` + `metrics` commands; `metric` events forwarded to the frontend.
- Frontend: suggestion/metric types, store metric buffer, detect→4-step wizard create flow (Daemon/Scheduled/Batch), metrics tab with live graphs, native notifications on crash/recovery.
- Rust bridge integration + Vitest tests green; both builds clean; render verified.
- Engine `cargo test` unaffected (standalone Tauri crate).

## Out of scope
- Folder-picker dialog plugin (text path input for v1; Browse is a nice-to-have).
- Events/audit tab, configurable retention, overlap Queue/KillPrevious UI.
- Signing/packaging (packaging phase).

## Self-review notes
- Spec §8 covered. Reuses 2a/2b/2c.2/2c.3 plumbing. `detect_workers`/`metrics` bridge → `Client::{detect,metrics}`; metric topic added to the event pump; store gains `metrics` buffer; `RunModeAny`/`SuggestionDraft`/`MetricPointT` TS mirror the Rust serde shapes (RunMode tagged `type`; Schedule externally-tagged `{cron}`/`{interval_secs}`).
- Tauri camelCase→snake_case arg mapping noted for `since_secs`.
- frontend-design applied to CreateFlow/WizardMode/MetricsTab/Sparkline.
