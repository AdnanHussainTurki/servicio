#!/usr/bin/env node
// Sync the app + daemon version to a single value (normally the git tag).
// Usage: node scripts/set-version.mjs 0.1.1   (a leading "v" is stripped)
//
// Writes the version into, from this script's location (apps/desktop/scripts):
//   - ../src-tauri/tauri.conf.json   → the Tauri app version (app_version command)
//   - ../../../Cargo.toml            → [workspace.package] version (daemon's
//                                       CARGO_PKG_VERSION, inherited by all crates)
//   - ../src-tauri/Cargo.toml        → the desktop app crate version
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const raw = process.argv[2];
if (!raw) {
  console.error('usage: set-version.mjs <version>');
  process.exit(1);
}
const version = raw.replace(/^v/, '');
if (!/^\d+\.\d+\.\d+/.test(version)) {
  console.error(`refusing to set a non-semver version: "${version}"`);
  process.exit(1);
}

const tauriConf = resolve(here, '../src-tauri/tauri.conf.json');
const rootCargo = resolve(here, '../../../Cargo.toml');
const appCargo = resolve(here, '../src-tauri/Cargo.toml');

// tauri.conf.json — targeted line replace to preserve the file's formatting.
{
  const text = readFileSync(tauriConf, 'utf8');
  const next = text.replace(/("version":\s*")\d+\.\d+\.\d+[^"]*(")/, `$1${version}$2`);
  if (next === text) {
    console.error(`warning: no version replaced in ${tauriConf}`);
  }
  writeFileSync(tauriConf, next);
}

// Cargo.toml files — replace only the first `version = "..."` (the package's own).
function setCargoVersion(path) {
  const text = readFileSync(path, 'utf8');
  const next = text.replace(/^version = "\d+\.\d+\.\d+[^"]*"/m, `version = "${version}"`);
  if (next === text) {
    console.error(`warning: no version line replaced in ${path}`);
  }
  writeFileSync(path, next);
}
setCargoVersion(rootCargo);
setCargoVersion(appCargo);

console.log(`set version → ${version}`);
