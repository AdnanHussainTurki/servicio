# Servicio Phase 4 — Richer Detection + Worker Organization Design Spec

**Date:** 2026-06-18
**Status:** Approved (design); built via /loop.
**Builds on:** Phases 1–3 (complete, app distributable). Parent: `docs/superpowers/specs/2026-06-18-servicio-design.md`.

## 1. Summary

Phase 4 makes worker setup smarter and the dashboard organized: the Laravel detector reads the
actual queue driver, a new detector mines `.vscode/tasks.json` for commands, and workers gain
**groups** (one per worker, dashboard sectioning) + **tags** (many, filter/label). Detectors
pre-fill sensible groups/tags.

### Goals
- Laravel queue-driver detection (`.env` `QUEUE_CONNECTION` / `config/queue.php`) → driver-aware
  `queue:work <connection>` suggestion; skip `sync`.
- `.vscode/tasks.json` detector (JSONC-tolerant) → worker suggestions.
- `WorkerSpec.group: Option<String>` + `WorkerSpec.tags: Vec<String>` end-to-end (engine →
  daemon/IPC status → GUI wizard fields → dashboard group sections + tag chips + tag filter).

### Non-goals
- Per-group bulk actions (start/stop a whole group) — future.
- Tag-based scheduling/policies. Saved filter views.

## 2. Decisions
| Topic | Decision |
|---|---|
| Group vs tags | Both — `group: Option<String>` (single, sections) + `tags: Vec<String>` (many, filter). |
| Laravel `sync` | No worker suggested (runs inline) — skipped with a note. |
| tasks.json | Parse `.vscode/tasks.json` JSONC (strip comments + trailing commas); each task → daemon suggestion. |
| Detector defaults | Detectors set `group` = project folder name; `tags` = e.g. `laravel`, queue driver, `vscode-task`. |
| Backward-compat | `group`/`tags` are `#[serde(default)]` → existing specs/DBs load unchanged. |

## 3. Sub-plan 4.1 — Engine group/tags + status passthrough
- `servicio-core::worker::WorkerSpec`: add `#[serde(default)] pub group: Option<String>` +
  `#[serde(default)] pub tags: Vec<String>`. Serde defaults keep old JSON valid.
- `servicio-core::event::WorkerStatusCore`: add `group: Option<String>` + `tags: Vec<String>`;
  `Manager::status()` copies them from the spec.
- `servicio-ipc::types::WorkerStatus`: add `group` + `tags`; daemon `list_workers` maps them
  (DB-backed list already has the full spec — read group/tags from there).
- Tests: serde roundtrip incl. group/tags + back-compat (old JSON without them → defaults);
  status carries them.

## 4. Sub-plan 4.2 — Detector enhancements (`servicio-detect`)
- **Laravel queue driver:** read `<root>/.env` for `QUEUE_CONNECTION=`; fallback `config/queue.php`
  `'default' => env('QUEUE_CONNECTION', 'sync')` literal; else `sync`. If `sync` → omit the
  queue worker (note). Else suggest `php artisan queue:work <driver> --tries=3` with concurrency
  4 for redis/sqs, 2 otherwise. Label "Laravel queue (<driver>)". Tag the suggestion with
  `laravel` + the driver.
- **tasks.json detector (new `tasks.rs`):** read `<root>/.vscode/tasks.json`, strip `//`/`/* */`
  comments + trailing commas, parse. For each task in `tasks[]`: derive `command`+`args` from
  `command`(+`args`) — if `command` contains shell metachars (`|&;><$`) wrap as `sh -c "<cmd>"`.
  Suggestion source `vscode/tasks.json`, tag `vscode-task`, label = task `label`.
- **All detectors set `group`** = the scanned folder's name, and relevant `tags`.
- Tests: fixture `.env`/`config/queue.php` driver cases (redis, sync→skip, default); JSONC
  tasks.json fixture (with comments + trailing comma); group/tag population; `detect_all` dedup.

## 5. Sub-plan 4.3 — GUI groups/tags
- **Types:** `WorkerStatus` + `SuggestionDraft` TS gain `group?: string | null` + `tags: string[]`.
- **Wizard (CreateFlow):** Review/Recovery step adds **Group** (text) + **Tags** (comma-separated)
  inputs; suggestions pre-fill them; the assembled `WorkerSpec` includes `group`/`tags`.
- **Dashboard:** group workers into **collapsible sections** by `group` (null → "Ungrouped");
  render **tag chips** on each card; a **tag filter bar** (click tag → filter the grid; multi-select
  AND/OR — OR for v1). Summary chips + "New worker" stay. frontend-design throughout.
- Tests: Vitest — dashboard sections by group, tag filter narrows the set, card shows tags;
  wizard emits group/tags in the spec.

## 6. Testing
Per sub-plan: Rust serde/detector fixture tests (TDD); daemon integration (status carries
group/tags; detect returns driver-aware + tasks.json suggestions); Vitest for wizard + dashboard.
Engine `cargo test` + GUI `npm run test` green; builds clean; render-verified.

## 7. Distributable definition
Phase 4 done = the 3 features merged, all tests green, and a fresh universal `.dmg` rebuilt in
`dist/` (unsigned-ship-ready, as in Phase 3). Signed distribution still awaits the user's Apple cert.

## 8. Open questions / future
- Bulk group actions; tag AND-filtering; saved views; tag colors; more detectors (Symfony,
  Rails, docker-compose). Carried deferrals from earlier phases remain.
