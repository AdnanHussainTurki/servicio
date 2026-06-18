# Releasing Servicio

## Cut a release via the workflow (recommended)
Tag `vX.Y.Z` and push — the [release workflow](../.github/workflows/release.yml) builds
the universal `.dmg` on macOS and creates a GitHub Release (with auto-generated notes):
```bash
git tag v0.1.0 && git push origin v0.1.0
```
The published `.dmg`/`.app` are **unsigned** (no Apple cert in CI). To produce a signed +
notarized release, add the `APPLE_*` env vars below as repository secrets and export them
in the build step (see the signing section). The steps below cover building locally.

## Prerequisites
- `rustup target add x86_64-apple-darwin` (for the Intel half of the universal build).
- Use the **rustup** toolchain for the universal build, not a Homebrew `rust` — only rustup
  ships the cross-arch std. If `cargo`/`rustc` resolve to `/opt/homebrew/bin`, prefix the
  build with the rustup shim: `PATH="$HOME/.cargo/bin:$PATH" npm run build:universal`.

## Universal (Intel + Apple Silicon) build
```bash
cd apps/desktop
PATH="$HOME/.cargo/bin:$PATH" npm run build:universal   # universal sidecar + universal .app/.dmg
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

## Error reporting (Sentry)
Set `SERVICIO_SENTRY_DSN` in the daemon's environment to enable crash/error reporting:
```bash
SERVICIO_SENTRY_DSN="https://...@sentry.io/123" servicio-daemon serve
```
When set, daemon panics and ERROR-level events (e.g. worker spawn failures) are sent to Sentry.
Unset = disabled (no-op). The GUI passes the daemon's environment through when it spawns the sidecar.
