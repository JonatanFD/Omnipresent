// Verifies that the macOS deliverable carries a valid, stable code signature.
//
// Two things must hold, and both have been broken in a shipped release:
//
//   1. The signature must be a real bundle signature. Rust's linker applies an
//      ad-hoc signature to the executable alone, which leaves `Info.plist` and
//      the bundle resources unsealed. `codesign --verify` rejects that with
//      "code has no resources but signature indicates they must be present" —
//      and a quarantined app that fails Gatekeeper's check is reported to the
//      user as "damaged and can't be opened", a dead end with no way to open it.
//
//   2. The identifier must be `com.jonatanfd.omni`, not the linker's
//      hash-derived one. macOS TCC tracks the Accessibility permission by that
//      identifier, so a hash that changes every build silently revokes the
//      grant: the app still appears in System Settings → Accessibility, but
//      `AXIsProcessTrusted()` returns false.
//
// `bundle.macOS.signingIdentity` in tauri.conf.json is what makes both true —
// Tauri signs the bundle before packaging it. This checks the .dmg rather than
// the intermediate .app on purpose: the bundle was once signed *after* the .dmg
// had already been built from it, so the fix never reached anyone.
//
//   node scripts/verify-macos.mjs path/to.dmg    verify inside a .dmg
//   node scripts/verify-macos.mjs path/to.app    verify a bundle directly
//   node scripts/verify-macos.mjs                find the built bundle
//
// The signature stays ad-hoc — no Developer ID certificate, so this runs in CI
// without secrets. Gatekeeper still refuses a quarantined download, but with
// "Apple could not verify..." and an Open Anyway path, rather than "damaged".
// Removing that prompt entirely needs notarization.
//
// No-op on non-macOS platforms: `codesign` does not exist there.

import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { platform, tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const tauriTarget = join(here, "..", "src-tauri", "target");

/** The bundle identifier Tauri writes into Info.plist — the one TCC should see. */
const IDENTIFIER = "com.jonatanfd.omni";

// Throws rather than exiting: a failure found inside a mounted .dmg still has
// to unwind through the `finally` that detaches it, and `process.exit` would
// skip that and leave the volume mounted.
function fail(message) {
  throw new Error(message);
}

/** The app bundle under a Tauri target directory, if one was built there. */
function builtBundle() {
  // Tauri lays out release bundles at target/release/bundle/macos and, with a
  // --target triple, at target/<triple>/release/bundle/macos. We check both so
  // local builds and CI cross-target builds are covered.
  return [
    join(tauriTarget, "release", "bundle", "macos", "Omnipresent.app"),
    join(tauriTarget, "aarch64-apple-darwin", "release", "bundle", "macos", "Omnipresent.app"),
    join(tauriTarget, "x86_64-apple-darwin", "release", "bundle", "macos", "Omnipresent.app"),
  ].find(existsSync);
}

/** Asserts the bundle is sealed, valid, and carries the stable identifier. */
function verify(bundle) {
  // --deep --strict is what Gatekeeper effectively runs. A linker-signed
  // executable inside an unsealed bundle fails here.
  const strict = spawnSync("codesign", ["--verify", "--deep", "--strict", bundle], {
    encoding: "utf8",
  });
  if (strict.status !== 0) {
    fail(`signature is not valid: ${(strict.stderr || "").trim()}`);
  }

  // `codesign -dv` writes to stderr, not stdout, so both streams are read.
  const shown = spawnSync("codesign", ["-dvvv", bundle], { encoding: "utf8" });
  const out = (shown.stdout || "") + (shown.stderr || "");

  const id = out.match(/Identifier=(\S+)/)?.[1];
  if (id !== IDENTIFIER) {
    fail(`identifier is ${id ?? "missing"}, expected ${IDENTIFIER} — the Accessibility permission would not survive an update`);
  }

  // "Sealed Resources=none" means only the executable is covered: the exact
  // shape that Gatekeeper reports to the user as a damaged app.
  if (/Sealed Resources=none/.test(out)) {
    fail("the bundle resources are not sealed — Gatekeeper would report this app as damaged");
  }

  console.log(`verified ${bundle}: valid, sealed, identifier ${id}`);
}

/** Mounts a .dmg read-only, runs `body` against the .app inside, then detaches. */
function withMountedDmg(dmg, body) {
  const mount = mkdtempSync(join(tmpdir(), "omni-verify-"));
  execFileSync(
    "hdiutil",
    ["attach", "-nobrowse", "-readonly", "-mountpoint", mount, dmg],
    { stdio: "pipe" },
  );
  try {
    const app = readdirSync(mount).find((entry) => entry.endsWith(".app"));
    if (!app) fail(`no .app inside ${dmg}`);
    body(join(mount, app));
  } finally {
    spawnSync("hdiutil", ["detach", mount], { stdio: "pipe" });
    rmSync(mount, { recursive: true, force: true });
  }
}

if (platform() !== "darwin") {
  console.log("not macOS — nothing to verify");
  process.exit(0);
}

const target = process.argv[2] ?? builtBundle();
for (const [bad, message] of [
  [!target, "no macOS bundle found — run `bun run tauri build` first"],
  [target && !existsSync(target), `${target} does not exist`],
]) {
  if (bad) {
    console.error(`verify-macos: ${message}`);
    process.exit(1);
  }
}

try {
  if (target.endsWith(".dmg")) withMountedDmg(target, verify);
  else verify(target);
} catch (error) {
  console.error(`verify-macos: ${error.message}`);
  process.exit(1);
}
