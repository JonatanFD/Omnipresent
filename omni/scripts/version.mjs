// Keeps the desktop client's version equal to the daemon's.
//
// The client ships the daemon inside it and the CLI beside it, all built from
// the crates next door. One install therefore has one version, and the three
// files below must agree with the workspace or the app reports a number that
// belongs to nothing. They cannot inherit it: `omni/src-tauri` is deliberately
// its own Cargo workspace, so `version.workspace = true` is not available.
//
//   node scripts/version.mjs           check, and fail if they disagree
//   node scripts/version.mjs --write   rewrite them from the workspace version
//
// The check runs before every packaging build and in CI, so a bundle carrying
// the wrong version cannot be produced, let alone released.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const client = join(here, "..");
const repo = join(client, "..");

/** The version the daemon and CLI are built at — the single source of truth. */
function workspaceVersion() {
  const manifest = readFileSync(join(repo, "Cargo.toml"), "utf8");
  // Anchored to [workspace.package] so a dependency's version cannot match.
  const section = manifest.split(/^\[workspace\.package\]$/m)[1];
  const version = section?.match(/^version\s*=\s*"([^"]+)"/m);
  if (!version) throw new Error("no version under [workspace.package] in the root Cargo.toml");
  return version[1];
}

/**
 * The files that carry the client's version, each with how to read and rewrite
 * it. Narrow patterns on purpose: a blanket replace would also rewrite the
 * version of every dependency that happens to sit nearby.
 */
const files = [
  {
    path: join(client, "src-tauri", "Cargo.toml"),
    label: "src-tauri/Cargo.toml",
    // The first `version = "..."` after [package], before any other section.
    pattern: /(^\[package\][\s\S]*?^version\s*=\s*")([^"]+)(")/m,
  },
  {
    path: join(client, "src-tauri", "tauri.conf.json"),
    label: "src-tauri/tauri.conf.json",
    pattern: /("version"\s*:\s*")([^"]+)(")/,
  },
  {
    path: join(client, "package.json"),
    label: "package.json",
    pattern: /("version"\s*:\s*")([^"]+)(")/,
  },
];

const write = process.argv.includes("--write");
const expected = workspaceVersion();
const wrong = [];

for (const file of files) {
  const text = readFileSync(file.path, "utf8");
  const found = text.match(file.pattern);
  if (!found) throw new Error(`could not find a version in ${file.label}`);

  const actual = found[2];
  if (actual === expected) continue;

  if (write) {
    writeFileSync(file.path, text.replace(file.pattern, `$1${expected}$3`));
    console.log(`${file.label}: ${actual} -> ${expected}`);
  } else {
    wrong.push(`  ${file.label}: ${actual}`);
  }
}

if (wrong.length > 0) {
  console.error(
    `The desktop client's version does not match the daemon it ships.\n` +
      `  workspace Cargo.toml: ${expected}\n${wrong.join("\n")}\n\n` +
      `Run \`bun run sync-version\` to bring them in line.`,
  );
  process.exit(1);
}

console.log(write ? `all files at ${expected}` : `version ${expected} is consistent`);
