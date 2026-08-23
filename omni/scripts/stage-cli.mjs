// Builds the `omni` CLI and stages it where Tauri expects a sidecar binary.
//
// The app bundles the CLI so that installing the app installs the command too.
// Tauri looks for `src-tauri/binaries/<name>-<target triple>`, strips the triple
// when it bundles, and drops the result next to the app binary — which is where
// `cli.rs` looks for it at runtime.
//
// Node rather than a shell script because this has to run identically on macOS,
// Linux, and Windows, and the alternative is maintaining a .sh and a .ps1 that
// drift apart.

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const client = join(here, "..");
const repo = join(client, "..");

/** The triple to build for: whatever Tauri is bundling, else this machine's. */
function targetTriple() {
  // Tauri sets this for every build, including cross builds, so honouring it is
  // what makes `--target aarch64-apple-darwin` on an x64 runner produce an arm64
  // CLI rather than silently bundling the host's.
  const fromTauri = process.env.TAURI_ENV_TARGET_TRIPLE;
  if (fromTauri) return fromTauri;

  const version = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const host = version.match(/^host:\s*(\S+)$/m);
  if (!host) throw new Error("could not read the host triple from `rustc -vV`");
  return host[1];
}

const triple = targetTriple();
const isWindows = triple.includes("windows");
const suffix = isWindows ? ".exe" : "";

// Always build for an explicit target. Cargo puts a `--target` build under
// `target/<triple>/release`, and omitting it would leave the two cases writing
// to different paths for no reason.
console.log(`staging the omni CLI for ${triple}`);
execFileSync(
  "cargo",
  ["build", "--release", "--locked", "--package", "omni-cli", "--target", triple],
  { cwd: repo, stdio: "inherit" },
);

// Staged as `omni-cli`, not `omni`: a sidecar may not share the name of the
// Cargo package that bundles it, and this app's package is `omni`. The app
// installs it under the plain `omni` the user types (see `cli.rs`).
const built = join(repo, "target", triple, "release", `omni${suffix}`);
const binaries = join(client, "src-tauri", "binaries");
const staged = join(binaries, `omni-cli-${triple}${suffix}`);

mkdirSync(binaries, { recursive: true });
copyFileSync(built, staged);

console.log(`staged ${staged}`);
