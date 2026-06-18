# Servicio Phase 3.1 — OS-Service Install Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Checkbox steps.

**Goal:** Let the daemon install itself as a login-start OS service (launchd LaunchAgent on macOS, systemd user unit on Linux) via `servicio-daemon install-service` / `uninstall-service` / `service-status`, so workers run always-on across reboot — fully TDD'd without installing a real service in CI.

**Architecture:** A new `service.rs` in `servicio-daemon`: pure plist/unit string generators (unit-tested), plus install/uninstall/status that write the unit file to a (parameterized) platform dir and optionally invoke the loader (`launchctl`/`systemctl`) — the loader call is gated by a `load: bool` so tests never touch the real service manager. CLI subcommands wire it.

**Tech Stack:** Rust, std only (no new deps). Tests: `tempfile`.

**Builds on:** Phases 1–2c (merged). Spec: `docs/superpowers/specs/2026-06-18-servicio-phase3-packaging-design.md` §3.

---

## Task 1: pure plist/unit generators (TDD)
**Files:** `crates/servicio-daemon/src/service.rs` (new), `src/lib.rs`

- [ ] **Step 1 — failing tests.** Create `crates/servicio-daemon/src/service.rs`:
```rust
use std::path::Path;

/// Spec for the service definition: which exe to run + base dir.
pub struct ServiceSpec {
    pub label: String,
    pub exe: std::path::PathBuf,
    pub base: std::path::PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn spec() -> ServiceSpec {
        ServiceSpec { label: "com.servicio.daemon".into(), exe: PathBuf::from("/usr/local/bin/servicio-daemon"), base: PathBuf::from("/tmp/servicio") }
    }

    #[test]
    fn launchd_plist_has_run_at_load_and_program_args() {
        let p = launchd_plist(&spec());
        assert!(p.contains("<key>RunAtLoad</key>"));
        assert!(p.contains("<true/>"));
        assert!(p.contains("com.servicio.daemon"));
        assert!(p.contains("/usr/local/bin/servicio-daemon"));
        assert!(p.contains("serve"));
        assert!(p.contains("--base"));
        assert!(p.contains("/tmp/servicio"));
    }

    #[test]
    fn systemd_unit_has_execstart_and_wantedby() {
        let u = systemd_unit(&spec());
        assert!(u.contains("ExecStart=/usr/local/bin/servicio-daemon serve --base /tmp/servicio"));
        assert!(u.contains("Restart=always"));
        assert!(u.contains("WantedBy=default.target"));
    }
}
```
- [ ] **Step 2 — run, FAIL.**
- [ ] **Step 3 — implement generators** (above the test block):
```rust
/// macOS LaunchAgent plist: runs `<exe> serve --base <base>`, at login, kept alive.
pub fn launchd_plist(spec: &ServiceSpec) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>serve</string>
        <string>--base</string>
        <string>{base}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
"#,
        label = spec.label,
        exe = spec.exe.display(),
        base = spec.base.display(),
    )
}

/// systemd user unit: runs `<exe> serve --base <base>`, restart always, at login.
pub fn systemd_unit(spec: &ServiceSpec) -> String {
    format!(
        "[Unit]\nDescription=Servicio supervisor daemon\nAfter=default.target\n\n\
[Service]\nExecStart={exe} serve --base {base}\nRestart=always\nRestartSec=2\n\n\
[Install]\nWantedBy=default.target\n",
        exe = spec.exe.display(),
        base = spec.base.display(),
    )
}

#[allow(dead_code)]
fn _unused(_: &Path) {}
```
(remove the `_unused` stub if it warns; it's only to keep `Path` imported if unused — drop the `use std::path::Path;` import instead if simpler.)
- [ ] **Step 4 — export.** `src/lib.rs`: add `pub mod service;`.
- [ ] **Step 5 — PASS** `cargo test -p servicio-daemon service`.
- [ ] **Step 6 — commit:** `git add crates/servicio-daemon/src/service.rs crates/servicio-daemon/src/lib.rs && git commit -m "feat(daemon): launchd/systemd service-definition generators"`

---

## Task 2: install/uninstall/status (TDD, loader-gated)
**Files:** `crates/servicio-daemon/src/service.rs`

- [ ] **Step 1 — failing tests.** Add to `service.rs` tests:
```rust
    #[test]
    fn install_writes_unit_file_to_dir_without_loading() {
        let dir = tempfile::tempdir().unwrap();
        let path = install_to(&spec(), dir.path(), false).unwrap();
        assert!(path.exists());
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("servicio-daemon"));
        // status sees it installed
        assert!(is_installed(dir.path(), &spec().label));
        // uninstall removes it
        uninstall_from(dir.path(), &spec().label, false).unwrap();
        assert!(!path.exists());
        assert!(!is_installed(dir.path(), &spec().label));
    }
```
- [ ] **Step 2 — run, FAIL.**
- [ ] **Step 3 — implement** (in `service.rs`):
```rust
use std::io;
use std::path::PathBuf;

/// Filename for the unit in `dir` (platform-shaped).
fn unit_filename(label: &str) -> String {
    if cfg!(target_os = "macos") { format!("{label}.plist") } else { "servicio.service".to_string() }
}

fn unit_body(spec: &ServiceSpec) -> String {
    if cfg!(target_os = "macos") { launchd_plist(spec) } else { systemd_unit(spec) }
}

/// Write the unit file into `dir`. If `load`, invoke the platform loader.
pub fn install_to(spec: &ServiceSpec, dir: &Path, load: bool) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(unit_filename(&spec.label));
    std::fs::write(&path, unit_body(spec))?;
    if load { run_loader(&path, &spec.label, true); }
    Ok(path)
}

pub fn uninstall_from(dir: &Path, label: &str, load: bool) -> io::Result<()> {
    let path = dir.join(unit_filename(label));
    if load { run_loader(&path, label, false); }
    if path.exists() { std::fs::remove_file(&path)?; }
    Ok(())
}

pub fn is_installed(dir: &Path, label: &str) -> bool {
    dir.join(unit_filename(label)).exists()
}

/// Best-effort invoke launchctl/systemctl. Errors are ignored (status is informational).
fn run_loader(path: &Path, label: &str, enable: bool) {
    #[cfg(target_os = "macos")]
    {
        let _ = label;
        let arg = if enable { "load" } else { "unload" };
        let _ = std::process::Command::new("launchctl").arg(arg).arg("-w").arg(path).status();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = path;
        let action = if enable { "enable" } else { "disable" };
        let _ = std::process::Command::new("systemctl").arg("--user").arg(action).arg("--now").arg("servicio.service").status();
        let _ = label;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    { let _ = (path, label, enable); }
}

/// Default platform dir for the unit file.
pub fn default_service_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    { dirs_home().map(|h| h.join("Library/LaunchAgents")) }
    #[cfg(target_os = "linux")]
    { dirs_config().map(|c| c.join("systemd/user")) }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    { None }
}

#[cfg(target_os = "macos")]
fn dirs_home() -> Option<PathBuf> { std::env::var_os("HOME").map(PathBuf::from) }
#[cfg(target_os = "linux")]
fn dirs_config() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}
```
- [ ] **Step 4 — PASS** `cargo test -p servicio-daemon service`. (`install_to`/`uninstall_from` with `load=false` never touch launchctl.)
- [ ] **Step 5 — commit:** `git add crates/servicio-daemon/src/service.rs && git commit -m "feat(daemon): install/uninstall/status service file management (loader-gated)"`

---

## Task 3: CLI subcommands (TDD via lib fns)
**Files:** `crates/servicio-daemon/src/cli.rs`, `src/main.rs`, `src/lib.rs`

- [ ] **Step 1 — lib helper + test.** Add to `crates/servicio-daemon/src/lib.rs` a high-level helper the CLI + tests share:
```rust
use service::ServiceSpec;

/// Build the ServiceSpec for the current daemon exe + base dir.
pub fn service_spec(base: std::path::PathBuf) -> std::io::Result<ServiceSpec> {
    let exe = std::env::current_exe()?;
    Ok(ServiceSpec { label: "com.servicio.daemon".into(), exe, base })
}
```
Add a test in `lib.rs` (create a `#[cfg(test)] mod tests` if absent):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn install_then_status_then_uninstall_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let spec = service_spec(std::path::PathBuf::from("/tmp/servicio")).unwrap();
        let p = service::install_to(&spec, dir.path(), false).unwrap();
        assert!(p.exists());
        assert!(service::is_installed(dir.path(), &spec.label));
        service::uninstall_from(dir.path(), &spec.label, false).unwrap();
        assert!(!service::is_installed(dir.path(), &spec.label));
    }
}
```
- [ ] **Step 2 — run, FAIL** (`service_spec` missing) → implement Step 1 → PASS `cargo test -p servicio-daemon --lib install_then_status`.
- [ ] **Step 3 — CLI subcommands.** In `cli.rs`, add to the `Command` enum:
```rust
    /// Install the daemon as a login service (launchd/systemd).
    InstallService {
        #[arg(long)]
        base: Option<PathBuf>,
    },
    /// Remove the installed login service.
    UninstallService,
    /// Show whether the login service is installed.
    ServiceStatus,
```
In `main.rs`, add match arms:
```rust
        Command::InstallService { base } => {
            use servicio_daemon_lib::{service, service_spec};
            use servicio_daemon_lib::paths::Paths;
            let base = base.unwrap_or_else(Paths::default_base);
            let dir = service::default_service_dir()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Unsupported, "service install not supported on this OS"))?;
            let spec = service_spec(base)?;
            let path = service::install_to(&spec, &dir, true)?;
            println!("installed service: {}", path.display());
        }
        Command::UninstallService => {
            use servicio_daemon_lib::service;
            let dir = service::default_service_dir()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Unsupported, "not supported on this OS"))?;
            service::uninstall_from(&dir, "com.servicio.daemon", true)?;
            println!("uninstalled service");
        }
        Command::ServiceStatus => {
            use servicio_daemon_lib::service;
            match service::default_service_dir() {
                Some(dir) => {
                    let installed = service::is_installed(&dir, "com.servicio.daemon");
                    println!("{{\"installed\": {installed}}}");
                }
                None => println!("{{\"installed\": false, \"supported\": false}}"),
            }
        }
```
(Keep the existing `serve`/`add`/`list` arms. `PathBuf` is imported in cli.rs.)
- [ ] **Step 4 — verify.** `cargo test -p servicio-daemon` → all pass. `cargo build --workspace` → clean. Manual (do NOT run in CI/agent — it installs a real LaunchAgent): the user can run `servicio-daemon service-status` to see `{"installed": false}`.
- [ ] **Step 5 — commit:** `git add crates/servicio-daemon && git commit -m "feat(daemon): install-service/uninstall-service/service-status CLI"`

---

## Task 4: servicio CLI passthrough + verify
**Files:** `crates/servicio-cli` is the *client* (talks over socket) — service install is a *daemon-local* operation, so it stays on `servicio-daemon`, NOT `servicio`. No client change needed.

- [ ] **Step 1 — confirm scope.** The `servicio` client controls a running daemon over the socket; installing a service is local filesystem + launchctl, done by `servicio-daemon` directly. So no `servicio` (client) subcommand. Document this in the commit / DoD. Nothing to implement here beyond confirming `cargo build --workspace` + `cargo test` are green from Task 3.
- [ ] **Step 2 — full verify.** `cargo test` (workspace) + `cargo build --workspace` green.
- [ ] **Step 3 — README note.** Add to `README.md` a line under Phase 3 / usage:
```markdown
### Always-on (install as a login service)

```bash
cargo run -p servicio-daemon -- install-service     # launchd (macOS) / systemd --user (Linux)
cargo run -p servicio-daemon -- service-status
cargo run -p servicio-daemon -- uninstall-service
```
The daemon then starts at login and keeps your `autostart` workers running across reboots.
```
- [ ] **Step 4 — commit:** `git add README.md && git commit -m "docs: install-service usage"`

---

## Definition of Done (3.1)
- `launchd_plist`/`systemd_unit` generate correct definitions (unit-tested).
- `install_to`/`uninstall_from`/`is_installed` manage the unit file in a parameterized dir;
  loader (`launchctl`/`systemctl`) gated by `load` (false in tests → CI installs nothing).
- `servicio-daemon install-service`/`uninstall-service`/`service-status` wired; `--base` flag.
- `cargo test` + `cargo build --workspace` green; README documents usage.

## Out of scope
- GUI integration (3.2). Universal build + updater (3.3). Windows Service. System LaunchDaemon.

## Self-review notes
- Spec §3 covered. No new deps (std + tempfile). Loader-gating keeps tests from installing a
  real service. Types consistent: `ServiceSpec`, `launchd_plist`/`systemd_unit`,
  `install_to`/`uninstall_from`/`is_installed`/`default_service_dir`, `service_spec`, CLI
  `InstallService`/`UninstallService`/`ServiceStatus`.
- Service install is daemon-local → on `servicio-daemon`, not the `servicio` client (Task 4).
