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
