# Servicio Phase 2c.3 — `servicio-detect` crate + detect IPC

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Checkbox steps.

**Goal:** A pure `servicio-detect` crate that scans a project folder and returns worker `SuggestionDraft`s via six detectors (Laravel, Python, Node, Procfile, Crontab, Generic), exposed over the socket as a `detect_workers{path}` method and a `servicio detect <path>` CLI.

**Architecture:** New leaf crate `servicio-detect` (depends only on `servicio-core` for `RunMode` + serde). One `Detector` trait, six impls, `detect_all(root)` runs all + dedups. `SuggestionDraft` is serde-serializable so the daemon returns it directly (no ipc mirror type needed; the GUI reads the JSON shape).

**Tech Stack:** Rust, serde; tests use `tempfile` fixture trees. No async.

**Builds on:** 2c.1, 2c.2 (merged). Spec: `docs/superpowers/specs/2026-06-18-servicio-phase2c-design.md` §6,§7.

---

## Task 1: crate scaffold + SuggestionDraft + Detector + Generic (TDD)
**Files:** root `Cargo.toml`, `crates/servicio-detect/Cargo.toml`, `crates/servicio-detect/src/lib.rs`

- [ ] **Step 1 — workspace member.** Root `Cargo.toml` `members`: append `"crates/servicio-detect"`.
- [ ] **Step 2 — manifest.** `crates/servicio-detect/Cargo.toml`:
```toml
[package]
name = "servicio-detect"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
servicio-core = { path = "../servicio-core" }
serde.workspace = true
serde_json.workspace = true

[dev-dependencies]
tempfile.workspace = true
```
- [ ] **Step 3 — failing test + types.** Create `crates/servicio-detect/src/lib.rs`:
```rust
//! servicio-detect: scan a project folder, suggest workers. Pure, no IO beyond reading `root`.

use serde::{Deserialize, Serialize};
use servicio_core::worker::RunMode;
use std::path::{Path, PathBuf};

mod laravel;
mod python;
mod node;
mod procfile;
mod crontab;

/// A proposed worker the user confirms/edits before it's created.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuggestionDraft {
    pub label: String,
    pub source: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub run_mode: RunMode,
}

pub trait Detector {
    fn name(&self) -> &str;
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft>;
}

/// Always-present fallback so the wizard can start from scratch.
pub struct Generic;
impl Detector for Generic {
    fn name(&self) -> &str { "generic" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        vec![SuggestionDraft {
            label: "Custom worker".into(),
            source: "generic".into(),
            name: String::new(),
            command: String::new(),
            args: vec![],
            working_dir: root.to_path_buf(),
            run_mode: RunMode::Daemon { concurrency: 1 },
        }]
    }
}

/// Run every detector against `root`, dedup by (command, args, working_dir).
pub fn detect_all(root: &Path) -> Vec<SuggestionDraft> {
    let detectors: Vec<Box<dyn Detector>> = vec![
        Box::new(laravel::Laravel),
        Box::new(python::Python),
        Box::new(node::Node),
        Box::new(procfile::Procfile),
        Box::new(crontab::Crontab),
        Box::new(Generic),
    ];
    let mut out: Vec<SuggestionDraft> = Vec::new();
    for d in &detectors {
        for s in d.detect(root) {
            let dup = out.iter().any(|e| e.command == s.command && e.args == s.args && e.working_dir == s.working_dir);
            if !dup { out.push(s); }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_always_suggests_one_draft() {
        let dir = tempfile::tempdir().unwrap();
        let s = Generic.detect(dir.path());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].source, "generic");
    }
    #[test]
    fn detect_all_includes_generic_on_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let all = detect_all(dir.path());
        assert!(all.iter().any(|s| s.source == "generic"));
    }
}
```
- [ ] **Step 4 — stub the 5 detector modules** so it compiles (each replaced in its task):
`crates/servicio-detect/src/{laravel,python,node,procfile,crontab}.rs`, each:
```rust
use crate::{Detector, SuggestionDraft};
use std::path::Path;
pub struct PLACEHOLDER;
```
Replace `PLACEHOLDER` per file with the right struct name: `Laravel`, `Python`, `Node`, `Procfile`, `Crontab`. And give each a no-op Detector impl returning `vec![]`:
```rust
impl Detector for Laravel {
    fn name(&self) -> &str { "laravel" }
    fn detect(&self, _root: &Path) -> Vec<SuggestionDraft> { vec![] }
}
```
(analogous for the others; these are replaced with real logic in Tasks 2–6).
- [ ] **Step 5 — verify.** `cargo test -p servicio-detect` → PASS (2 tests). `cargo build -p servicio-detect`.
- [ ] **Step 6 — commit:** `git add Cargo.toml Cargo.lock crates/servicio-detect && git commit -m "feat(detect): crate scaffold, SuggestionDraft, Detector trait, Generic + detect_all"`

---

## Task 2: Laravel detector (TDD)
**Files:** `crates/servicio-detect/src/laravel.rs`

- [ ] **Step 1 — failing test.** Replace `laravel.rs`:
```rust
use crate::{Detector, SuggestionDraft};
use servicio_core::worker::{RunMode, Schedule, OverlapPolicy};
use std::path::Path;

pub struct Laravel;

impl Detector for Laravel {
    fn name(&self) -> &str { "laravel" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        if !root.join("artisan").exists() { return vec![]; }
        let composer = std::fs::read_to_string(root.join("composer.json")).unwrap_or_default();
        let mut out = vec![
            draft(root, "Laravel queue worker", "artisan queue:work",
                  vec!["artisan".into(), "queue:work".into()],
                  RunMode::Daemon { concurrency: 2 }),
            draft(root, "Laravel scheduler", "artisan schedule:run",
                  vec!["artisan".into(), "schedule:run".into()],
                  RunMode::Scheduled { schedule: Schedule::Cron("* * * * *".into()), overlap: OverlapPolicy::Skip }),
        ];
        if composer.contains("laravel/horizon") {
            out.push(draft(root, "Laravel Horizon", "artisan horizon",
                           vec!["artisan".into(), "horizon".into()], RunMode::Daemon { concurrency: 1 }));
        }
        if composer.contains("laravel/reverb") {
            out.push(draft(root, "Laravel Reverb", "artisan reverb:start",
                           vec!["artisan".into(), "reverb:start".into()], RunMode::Daemon { concurrency: 1 }));
        }
        out
    }
}

fn draft(root: &Path, label: &str, _shown: &str, args: Vec<String>, run_mode: RunMode) -> SuggestionDraft {
    SuggestionDraft {
        label: label.into(), source: "laravel/artisan".into(),
        name: label.to_lowercase().replace(' ', "-"),
        command: "php".into(), args, working_dir: root.to_path_buf(), run_mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_queue_and_scheduler_when_artisan_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("artisan"), "#!/usr/bin/env php").unwrap();
        let s = Laravel.detect(dir.path());
        assert!(s.iter().any(|d| d.args == vec!["artisan", "queue:work"]));
        assert!(s.iter().any(|d| matches!(d.run_mode, RunMode::Scheduled { .. })));
    }
    #[test]
    fn no_artisan_no_suggestions() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Laravel.detect(dir.path()).is_empty());
    }
    #[test]
    fn horizon_detected_from_composer() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("artisan"), "").unwrap();
        std::fs::write(dir.path().join("composer.json"), r#"{"require":{"laravel/horizon":"^5"}}"#).unwrap();
        let s = Laravel.detect(dir.path());
        assert!(s.iter().any(|d| d.args == vec!["artisan", "horizon"]));
    }
}
```
- [ ] **Step 2 — run → PASS** (`cargo test -p servicio-detect laravel`). Confirm `Schedule`/`OverlapPolicy` import paths from `servicio_core::worker` are correct.
- [ ] **Step 3 — commit:** `git add crates/servicio-detect/src/laravel.rs && git commit -m "feat(detect): Laravel detector"`

---

## Task 3: Python detector (TDD)
**Files:** `crates/servicio-detect/src/python.rs`

- [ ] **Step 1 — replace `python.rs`:**
```rust
use crate::{Detector, SuggestionDraft};
use servicio_core::worker::RunMode;
use std::path::Path;

pub struct Python;

impl Detector for Python {
    fn name(&self) -> &str { "python" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        let has_reqs = root.join("requirements.txt").exists() || root.join("pyproject.toml").exists();
        let has_venv = root.join(".venv/bin/python").exists() || root.join("venv/bin/python").exists();
        // pick an entry script
        let candidates = ["worker.py", "main.py", "app.py", "manage.py"];
        let entry = candidates.iter().find(|c| root.join(c).exists()).map(|s| s.to_string())
            .or_else(|| first_py(root));
        let Some(entry) = entry else {
            // python project but no obvious script
            if has_reqs || has_venv {
                return vec![SuggestionDraft {
                    label: "Python worker".into(), source: "python".into(), name: "python-worker".into(),
                    command: "python".into(), args: vec![], working_dir: root.to_path_buf(),
                    run_mode: RunMode::Daemon { concurrency: 1 },
                }];
            }
            return vec![];
        };
        vec![SuggestionDraft {
            label: format!("Python: {entry}"), source: "python".into(),
            name: entry.trim_end_matches(".py").replace(['/', '\\'], "-"),
            command: "python".into(), args: vec![entry.clone()],
            working_dir: root.to_path_buf(), run_mode: RunMode::Daemon { concurrency: 1 },
        }]
    }
}

fn first_py(root: &Path) -> Option<String> {
    let mut names: Vec<String> = std::fs::read_dir(root).ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".py"))
        .collect();
    names.sort();
    names.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_named_entry_script() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("requirements.txt"), "flask").unwrap();
        std::fs::write(dir.path().join("worker.py"), "print(1)").unwrap();
        let s = Python.detect(dir.path());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].args, vec!["worker.py".to_string()]);
        assert_eq!(s[0].command, "python");
    }
    #[test]
    fn no_python_no_suggestion() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Python.detect(dir.path()).is_empty());
    }
}
```
- [ ] **Step 2 — PASS** (`cargo test -p servicio-detect python`).
- [ ] **Step 3 — commit:** `git add crates/servicio-detect/src/python.rs && git commit -m "feat(detect): Python detector"`

---

## Task 4: Node + Procfile detectors (TDD)
**Files:** `crates/servicio-detect/src/node.rs`, `procfile.rs`

- [ ] **Step 1 — replace `node.rs`:**
```rust
use crate::{Detector, SuggestionDraft};
use servicio_core::worker::RunMode;
use std::path::Path;

pub struct Node;

impl Detector for Node {
    fn name(&self) -> &str { "node" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        let pkg = match std::fs::read_to_string(root.join("package.json")) {
            Ok(s) => s, Err(_) => return vec![],
        };
        let v: serde_json::Value = match serde_json::from_str(&pkg) { Ok(v) => v, Err(_) => return vec![] };
        let mut out = vec![];
        if let Some(scripts) = v.get("scripts").and_then(|s| s.as_object()) {
            for (name, _) in scripts {
                let n = name.to_lowercase();
                if n.contains("worker") || n.contains("queue") {
                    out.push(SuggestionDraft {
                        label: format!("npm run {name}"), source: "package.json".into(),
                        name: format!("node-{name}"), command: "npm".into(),
                        args: vec!["run".into(), name.clone()],
                        working_dir: root.to_path_buf(), run_mode: RunMode::Daemon { concurrency: 1 },
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_worker_script() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"),
            r#"{"scripts":{"worker":"node worker.js","build":"tsc"}}"#).unwrap();
        let s = Node.detect(dir.path());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].args, vec!["run".to_string(), "worker".to_string()]);
    }
}
```
- [ ] **Step 2 — replace `procfile.rs`:**
```rust
use crate::{Detector, SuggestionDraft};
use servicio_core::worker::RunMode;
use std::path::Path;

pub struct Procfile;

impl Detector for Procfile {
    fn name(&self) -> &str { "procfile" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        let body = match std::fs::read_to_string(root.join("Procfile")) {
            Ok(s) => s, Err(_) => return vec![],
        };
        let mut out = vec![];
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let Some((name, cmd)) = line.split_once(':') else { continue };
            let cmd = cmd.trim();
            let mut parts = cmd.split_whitespace();
            let Some(command) = parts.next() else { continue };
            let args: Vec<String> = parts.map(|s| s.to_string()).collect();
            out.push(SuggestionDraft {
                label: format!("Procfile: {}", name.trim()), source: "Procfile".into(),
                name: name.trim().to_string(), command: command.to_string(), args,
                working_dir: root.to_path_buf(), run_mode: RunMode::Daemon { concurrency: 1 },
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_each_line() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Procfile"), "web: node server.js\nworker: node worker.js\n").unwrap();
        let s = Procfile.detect(dir.path());
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].name, "worker");
        assert_eq!(s[1].command, "node");
        assert_eq!(s[1].args, vec!["worker.js".to_string()]);
    }
}
```
- [ ] **Step 3 — PASS** (`cargo test -p servicio-detect node procfile` — run both filters, or just `cargo test -p servicio-detect`).
- [ ] **Step 4 — commit:** `git add crates/servicio-detect/src/node.rs crates/servicio-detect/src/procfile.rs && git commit -m "feat(detect): Node + Procfile detectors"`

---

## Task 5: Crontab detector + detect_all dedup (TDD)
**Files:** `crates/servicio-detect/src/crontab.rs`, `crates/servicio-detect/src/lib.rs`

- [ ] **Step 1 — replace `crontab.rs`:**
```rust
use crate::{Detector, SuggestionDraft};
use servicio_core::worker::{RunMode, Schedule, OverlapPolicy};
use std::path::Path;

pub struct Crontab;

impl Detector for Crontab {
    fn name(&self) -> &str { "crontab" }
    fn detect(&self, root: &Path) -> Vec<SuggestionDraft> {
        let body = match std::fs::read_to_string(root.join("crontab")) {
            Ok(s) => s, Err(_) => return vec![],
        };
        let mut out = vec![];
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 6 { continue; }
            let expr = fields[..5].join(" ");
            let command = fields[5].to_string();
            let args: Vec<String> = fields[6..].iter().map(|s| s.to_string()).collect();
            out.push(SuggestionDraft {
                label: format!("crontab: {command}"), source: "crontab".into(),
                name: format!("cron-{command}").replace(['/', '.'], "-"),
                command, args, working_dir: root.to_path_buf(),
                run_mode: RunMode::Scheduled { schedule: Schedule::Cron(expr), overlap: OverlapPolicy::Skip },
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_cron_lines_to_scheduled() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("crontab"), "0 3 * * * python cleanup.py\n# comment\n").unwrap();
        let s = Crontab.detect(dir.path());
        assert_eq!(s.len(), 1);
        match &s[0].run_mode {
            RunMode::Scheduled { schedule: Schedule::Cron(e), .. } => assert_eq!(e, "0 3 * * *"),
            other => panic!("expected scheduled cron, got {other:?}"),
        }
        assert_eq!(s[0].command, "python");
        assert_eq!(s[0].args, vec!["cleanup.py".to_string()]);
    }
}
```
- [ ] **Step 2 — dedup test.** Add to `lib.rs` tests:
```rust
    #[test]
    fn detect_all_dedups_identical_commands() {
        let dir = tempfile::tempdir().unwrap();
        // Procfile + Python both could suggest the same script; ensure no exact dup remains.
        std::fs::write(dir.path().join("Procfile"), "worker: python worker.py\n").unwrap();
        std::fs::write(dir.path().join("worker.py"), "print(1)").unwrap();
        std::fs::write(dir.path().join("requirements.txt"), "x").unwrap();
        let all = detect_all(dir.path());
        // python suggests `python ["worker.py"]`; procfile suggests `python ["worker.py"]` — must dedup to one.
        let count = all.iter().filter(|s| s.command == "python" && s.args == vec!["worker.py".to_string()]).count();
        assert_eq!(count, 1);
    }
```
- [ ] **Step 3 — PASS** (`cargo test -p servicio-detect`).
- [ ] **Step 4 — commit:** `git add crates/servicio-detect/src/crontab.rs crates/servicio-detect/src/lib.rs && git commit -m "feat(detect): Crontab detector + detect_all dedup test"`

---

## Task 6: daemon `detect_workers` IPC + CLI (TDD integration)
**Files:** `crates/servicio-daemon/Cargo.toml`, `src/serve.rs`, `tests/serve_integration.rs`, `crates/servicio-cli/src/{client.rs,main.rs}`

- [ ] **Step 1 — dep.** `crates/servicio-daemon/Cargo.toml` `[dependencies]`: `servicio-detect = { path = "../servicio-detect" }`.
- [ ] **Step 2 — dispatch arm.** In `serve.rs` `dispatch`, before `other =>`:
```rust
        "detect_workers" => {
            let path = params.get("path").and_then(|p| p.as_str()).unwrap_or("");
            let suggestions = servicio_detect::detect_all(std::path::Path::new(path));
            match serde_json::to_value(suggestions) {
                Ok(v) => Frame::ok(id, v),
                Err(e) => Frame::err(id, "internal", &e.to_string()),
            }
        }
```
- [ ] **Step 3 — integration test.** Append to `tests/serve_integration.rs`:
```rust
#[tokio::test]
async fn detect_workers_finds_laravel_in_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().to_path_buf());
    let h = start(paths.clone(), "secret".into()).await;
    // fixture project with an artisan file
    let proj = dir.path().join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(proj.join("artisan"), "#!/usr/bin/env php").unwrap();
    let replies = hello_then(&paths.socket(), vec![
        Frame::Request { id: 1, method: "detect_workers".into(), params: json!({"path": proj.to_str().unwrap()}) },
    ]).await;
    match &replies[1] {
        Frame::Response { id: 1, result: Some(v), .. } => {
            let arr = v.as_array().unwrap();
            assert!(arr.iter().any(|s| s["source"] == "laravel/artisan"));
            assert!(arr.iter().any(|s| s["source"] == "generic")); // always present
        }
        other => panic!("unexpected: {other:?}"),
    }
    h.shutdown().await;
}
```
- [ ] **Step 4 — CLI.** In `crates/servicio-cli/src/client.rs` `impl Client`:
```rust
    pub async fn detect(&mut self, path: &str) -> Result<serde_json::Value> {
        self.request("detect_workers", json!({ "path": path })).await
    }
```
In `main.rs`, add `Detect { path: String }` to `Command` and an arm:
```rust
        Command::Detect { path } => {
            let v = client.detect(&path).await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
```
- [ ] **Step 5 — verify.** `cargo test -p servicio-daemon --test serve_integration detect_workers` → PASS. Then full `cargo test` + `cargo build --workspace` → green.
- [ ] **Step 6 — commit:** `git add crates/servicio-daemon crates/servicio-cli Cargo.lock && git commit -m "feat(daemon,cli): detect_workers IPC + servicio detect command"`

---

## Definition of Done (2c.3)
- `servicio-detect` crate: `SuggestionDraft`, `Detector`, six detectors, `detect_all` (dedup + Generic always present).
- Laravel (queue+schedule+horizon/reverb), Python (entry script), Node (worker/queue scripts), Procfile (per line), Crontab (cron→Scheduled), Generic.
- Daemon `detect_workers{path}` returns suggestions; `servicio detect <path>` prints them.
- `cargo test` + `cargo build --workspace` green.

## Out of scope
- GUI detect step (2c.5). Recursive/multi-dir scanning, monorepo heuristics, more frameworks.

## Self-review notes
- Spec §6/§7 covered. `SuggestionDraft` is serde so the daemon returns it directly — no ipc mirror type (keeps `servicio-ipc` lean). `RunMode`/`Schedule`/`OverlapPolicy` reused from `servicio-core`.
- Types consistent: `Detector`/`SuggestionDraft`/`detect_all` + `Laravel`/`Python`/`Node`/`Procfile`/`Crontab`/`Generic`; daemon `detect_workers` → `serde_json::to_value(Vec<SuggestionDraft>)`; `Client::detect`.
- Dedup key = (command, args, working_dir); Generic's empty command is unique so it always survives.
