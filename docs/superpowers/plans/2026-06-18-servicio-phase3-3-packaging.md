# Servicio Phase 3.3 — Universal Build + Updater + Signing Docs

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Checkbox steps. Mostly build-tooling/config/docs (not heavy TDD).

**Goal:** Make Servicio releasable: a universal macOS app (Intel + ARM), a fully-wired Tauri auto-updater (own signing keypair — already generated), and `RELEASING.md` documenting the one user-gated step (Apple code-signing/notarization).

**Architecture:** `prepare-sidecar.sh` builds the daemon for both arches + `lipo`s a universal sidecar; `tauri build --target universal-apple-darwin` makes a universal bundle. The Tauri updater plugin is added + configured with the generated public key; a guarded "Check for updates" action sits in Settings. Apple signing is config + docs only (blocked on the user's Developer cert).

**Env prep (DONE before this plan):** `rustup target add x86_64-apple-darwin` installed; updater keypair generated at `.secrets/servicio-updater.key(.pub)` (gitignored). Public key:
```
dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDVCRjc3RTI0QTcxODA1RDAKUldUUUJSaW5KSDczVzlDaGJFOHpudElISnFEeG1SYVZHWEdaWUc4V2ZTYTA2QVE1UnFtVStmdWQK
```

**Builds on:** 3.1, 3.2 (merged). Spec §5.

---

## Task 1: universal sidecar build script
**Files:** `apps/desktop/scripts/prepare-sidecar.sh`, `apps/desktop/package.json`

- [ ] **Step 1 — universal sidecar.** Update `apps/desktop/scripts/prepare-sidecar.sh` to also produce a universal sidecar. Keep the existing host-triple staging; add universal staging:
```bash
#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
BIN_DIR="$ROOT/apps/desktop/src-tauri/binaries"
mkdir -p "$BIN_DIR"
HOST="$(rustc -vV | sed -n 's/host: //p')"

if [ "${UNIVERSAL:-0}" = "1" ]; then
  cargo build --release -p servicio-daemon --target aarch64-apple-darwin --manifest-path "$ROOT/Cargo.toml"
  cargo build --release -p servicio-daemon --target x86_64-apple-darwin  --manifest-path "$ROOT/Cargo.toml"
  lipo -create \
    "$ROOT/target/aarch64-apple-darwin/release/servicio-daemon" \
    "$ROOT/target/x86_64-apple-darwin/release/servicio-daemon" \
    -output "$BIN_DIR/servicio-daemon-universal-apple-darwin"
  echo "staged universal $BIN_DIR/servicio-daemon-universal-apple-darwin"
else
  cargo build --release -p servicio-daemon --manifest-path "$ROOT/Cargo.toml"
  cp "$ROOT/target/release/servicio-daemon" "$BIN_DIR/servicio-daemon-$HOST"
  echo "staged $BIN_DIR/servicio-daemon-$HOST"
fi
```
Add npm scripts to `apps/desktop/package.json`:
```json
"prepare-sidecar:universal": "UNIVERSAL=1 bash scripts/prepare-sidecar.sh",
"build:universal": "npm run prepare-sidecar:universal && tauri build --target universal-apple-darwin"
```
- [ ] **Step 2 — verify the universal sidecar builds.** From `apps/desktop`: `npm run prepare-sidecar:universal` (downloads x86_64 std if needed — retry on timeout). Then `lipo -info src-tauri/binaries/servicio-daemon-universal-apple-darwin` → expect "Architectures ... x86_64 arm64". Report the lipo output.
- [ ] **Step 3 — commit:** `git add apps/desktop/scripts apps/desktop/package.json && git commit -m "feat(release): universal macOS sidecar build (lipo aarch64+x86_64)"`

---

## Task 2: Tauri updater plugin + config
**Files:** `apps/desktop/package.json`, `src-tauri/Cargo.toml`, `tauri.conf.json`, `src-tauri/src/main.rs`, `capabilities/default.json`, `src/api.ts`, `src/components/SettingsView.tsx`

- [ ] **Step 1 — deps.** From `apps/desktop`: `npm install @tauri-apps/plugin-updater`. In `src-tauri/Cargo.toml` `[dependencies]`: `tauri-plugin-updater = "2"`. (Retry on network timeout.)
- [ ] **Step 2 — config.** In `apps/desktop/src-tauri/tauri.conf.json`, add a `plugins.updater` block (create `plugins` if absent):
```json
"plugins": {
  "updater": {
    "active": true,
    "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDVCRjc3RTI0QTcxODA1RDAKUldUUUJSaW5KSDczVzlDaGJFOHpudElISnFEeG1SYVZHWEdaWUc4V2ZTYTA2QVE1UnFtVStmdWQK",
    "endpoints": ["https://example.com/servicio/{{target}}/{{arch}}/{{current_version}}"]
  }
}
```
(The `endpoints` URL is a placeholder the user replaces with their real update host.)
- [ ] **Step 3 — register plugin + capability.** In `src-tauri/src/main.rs` builder chain add `.plugin(tauri_plugin_updater::Builder::new().build())`. In `capabilities/default.json` permissions add `"updater:default"`.
- [ ] **Step 4 — Settings "check for updates".** Add to `src/api.ts`:
```ts
  checkUpdate: async (): Promise<string | null> => {
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const u = await check();
      return u ? `Update available: ${u.version}` : "Up to date";
    } catch (e) { return `Updater unavailable: ${String(e)}`; }
  },
```
In `SettingsView.tsx`, add a "Check for updates" button that calls `api.checkUpdate()` and shows the returned message inline. Guarded (the try/catch already makes it safe without an endpoint / outside Tauri).
- [ ] **Step 5 — verify.** `npm run test` (existing green) + `npm run build` clean. `cd src-tauri && cargo build` clean (plugin compiles; retry on timeout). Report.
- [ ] **Step 6 — commit:** `git add apps/desktop && git commit -m "feat(release): wire Tauri auto-updater (plugin, pubkey config, check-for-updates)"`

---

## Task 3: RELEASING.md + Apple signing config stub
**Files:** `docs/RELEASING.md` (new), `apps/desktop/src-tauri/tauri.conf.json`

- [ ] **Step 1 — signing config stub.** In `tauri.conf.json` `bundle.macOS` (create if absent), document-as-config:
```json
"macOS": {
  "minimumSystemVersion": "10.15"
}
```
(Leave `signingIdentity`/`entitlements` OUT so unsigned builds work; RELEASING.md explains adding them. Do not set an empty signingIdentity — that can break the build.)
- [ ] **Step 2 — RELEASING.md.** Create `docs/RELEASING.md`:
```markdown
# Releasing Servicio

## Universal (Intel + Apple Silicon) build
```bash
cd apps/desktop
npm run build:universal        # builds the universal sidecar + universal .app/.dmg
```
Output: `src-tauri/target/universal-apple-darwin/release/bundle/{macos,dmg}/`.

## Auto-updater
The updater is wired (public key in `tauri.conf.json` → `plugins.updater.pubkey`).
1. Set the real update host in `plugins.updater.endpoints`.
2. Sign update artifacts at build time by exporting the private key:
   ```bash
   export TAURI_SIGNING_PRIVATE_KEY="$(cat .secrets/servicio-updater.key)"
   export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""     # empty — key generated without a password
   npm run build:universal
   ```
   Tauri emits `*.sig` files + a `latest.json`-style manifest to host at the endpoint.
   **Keep `.secrets/servicio-updater.key` private (gitignored). Losing it breaks updates.**

## Apple code-signing + notarization (required for public distribution — needs an Apple Developer account)
Unsigned builds run locally after a right-click→Open / `xattr -dr com.apple.quarantine`.
To ship signed + notarized:
1. Obtain a "Developer ID Application" certificate (Apple Developer Program, $99/yr).
2. Export signing env vars before building (Tauri reads these):
   ```bash
   export APPLE_CERTIFICATE="<base64 .p12>"
   export APPLE_CERTIFICATE_PASSWORD="<p12 password>"
   export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
   export APPLE_ID="you@example.com"
   export APPLE_PASSWORD="<app-specific password>"
   export APPLE_TEAM_ID="TEAMID"
   npm run build:universal
   ```
   Tauri signs the `.app` and notarizes the `.dmg` automatically when these are set.
3. Verify: `spctl -a -t exec -vv <app>` and `xcrun stapler validate <dmg>`.

## Service install (end users)
After install, enable always-on via the GUI Settings "Start on login" toggle, or:
```bash
servicio-daemon install-service
```
```
- [ ] **Step 3 — commit:** `git add docs/RELEASING.md apps/desktop/src-tauri/tauri.conf.json && git commit -m "docs(release): RELEASING.md + macOS bundle config"`

---

## Task 4: universal release build verification
- [ ] **Step 1 — build the universal app.** From `apps/desktop`: `npm run build:universal` 2>&1 | tail -15 (heavy — both arches + bundle; retry on network timeout). Expect it to finish with `servicio.app` + `servicio_*.dmg` under `src-tauri/target/universal-apple-darwin/release/bundle/`.
- [ ] **Step 2 — verify universal.** `lipo -info src-tauri/target/universal-apple-darwin/release/bundle/macos/servicio.app/Contents/MacOS/servicio` → expect x86_64 + arm64. Same for the bundled `servicio-daemon`. Report.
- [ ] **Step 3 — record.** Note in the commit message / report that the universal `.dmg` built and is fat-binary. (No code change; if the build needs a tweak to succeed, make the minimal fix and commit it.)

---

## Definition of Done (3.3 / Phase 3 / ship-ready)
- `npm run build:universal` produces a universal (x86_64+arm64) `.app`/`.dmg` with a universal daemon sidecar.
- Tauri updater wired (pubkey configured, plugin registered, guarded "check for updates" in Settings).
- `RELEASING.md` documents universal build, updater signing, and the Apple sign/notarize steps.
- Engine/GUI tests green; the app is **unsigned-ship-ready** (signed public distribution awaits the user's Apple cert).

## Out of scope
- A hosted update endpoint + CI pipeline. Windows/Linux release artifacts. Actual Apple signing (user cert).

## Self-review notes
- Spec §5 covered. Updater fully wired with the real generated pubkey; private key gitignored + documented. Universal via lipo + `tauri build --target universal-apple-darwin`. Apple signing left as documented env-var config (no empty signingIdentity that would break unsigned builds). `RELEASING.md` is the single source for the user-gated signing step.
