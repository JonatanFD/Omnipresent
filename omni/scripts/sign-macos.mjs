// Stabilizes the macOS bundle's code signature so the Accessibility permission
// survives a rebuild.
//
// Rust's linker applies an ad-hoc signature automatically so the binary can
// load, but its identifier is derived from the binary's hash — so every build
// gets a different one. macOS TCC tracks the Accessibility permission by that
// identifier, not by bundle ID or app name, which means a rebuild silently
// revokes the grant: the app still appears in System Settings → Accessibility,
// but `AXIsProcessTrusted()` returns false for the new binary.
//
// Re-signing the bundle ad-hoc with an explicit, stable identifier
// (`com.jonatanfd.omni`, matching `tauri.conf.json`) makes TCC recognize the
// app across rebuilds. It stays ad-hoc — no Developer ID certificate is
// required — so this runs locally and in CI without secrets.
//
//   node scripts/sign-macos.mjs           sign the release bundle
//   node scripts/sign-macos.mjs --debug   sign the debug bundle too
//
// No-op on non-macOS platforms: the bundle format is macOS-only and `codesign`
// does not exist elsewhere, so the script exits cleanly when there is nothing
// to sign.

import { execFileSync, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { platform } from "node:os";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const client = join(here, "..");
const tauriTarget = join(client, "src-tauri", "target");

/** The bundle identifier Tauri writes into Info.plist — the one TCC should see. */
const IDENTIFIER = "com.jonatanfd.omni";

/** The app bundle under a Tauri target directory, if one was built there. */
function bundleUnder(profile) {
  // Tauri lays out release bundles at target/release/bundle/macos and, with a
  // --target triple, at target/<triple>/release/bundle/macos. We check both so
  // local builds and CI cross-target builds are covered.
  const candidates = [
    join(tauriTarget, profile, "bundle", "macos", "Omnipresent.app"),
    join(tauriTarget, "aarch64-apple-darwin", profile, "bundle", "macos", "Omnipresent.app"),
    join(tauriTarget, "x86_64-apple-darwin", profile, "bundle", "macos", "Omnipresent.app"),
  ];
  return candidates.find(existsSync);
}

/** Runs `codesign` to re-sign the bundle with a stable identifier. */
function sign(bundle) {
  // --force overrides the linker's ad-hoc signature.
  // --deep signs the whole bundle (here just the main executable, but the flag
  //   keeps it correct if frameworks are added later).
  // --sign - is an ad-hoc signature — no certificate needed.
  // --identifier sets the stable TCC identity.
  execFileSync("codesign", [
    "--force",
    "--deep",
    "--sign",
    "-",
    "--identifier",
    IDENTIFIER,
    bundle,
  ], { stdio: "inherit" });
}

function verify(bundle) {
  // `codesign -dv` writes its output to stderr, not stdout, so spawnSync with
  // a merged pipe is used to read it. Print just the identifier line so the
  // build log shows the fix took effect.
  const result = spawnSync("codesign", ["-dv", bundle], { encoding: "utf8" });
  const out = (result.stdout || "") + (result.stderr || "");
  const id = out.match(/Identifier=(\S+)/);
  if (!id || id[1] !== IDENTIFIER) {
    throw new Error(`signature identifier is ${id?.[1] ?? "missing"}, expected ${IDENTIFIER}`);
  }
  console.log(`signed ${bundle} as ${id[1]}`);
}

if (platform() !== "darwin") {
  console.log("not macOS — nothing to sign");
  process.exit(0);
}

const profiles = ["release"];
if (process.argv.includes("--debug")) profiles.unshift("debug");

let signed = false;
for (const profile of profiles) {
  const bundle = bundleUnder(profile);
  if (!bundle) continue;
  sign(bundle);
  verify(bundle);
  signed = true;
}

if (!signed) {
  console.log("no macOS bundle found — run `bun run tauri build` first");
  process.exit(1);
}
