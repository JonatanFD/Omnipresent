# Omnipresent — desktop client

The cross-platform GUI: one codebase for macOS, Windows and Linux, built with
Tauri 2, React and TypeScript.

It exists to answer a different question from the two native clients in
[`../clients/`](../clients). They ask "what looks most at home on this OS"; this
one asks "what can someone install in a single step".

## What makes it different

**The daemon runs inside it.** The native clients assume a daemon is already
installed and running, which is two installations for the user. This one links
`omni-runtime` and starts the daemon on a background thread of its own process
([`src-tauri/src/daemon.rs`](src-tauri/src/daemon.rs)), so installing the app
installs everything.

That changes *where* the daemon runs, not *who owns the state*. The window still
speaks the same JSON-lines IPC over the same socket the CLI uses, and
re-implements none of the decisions the daemon owns — trust, layout maths,
fingerprinting. It works unchanged against a daemon that was already running: if
one owns the socket, the embedded one stands down and the window talks to that.

**The `omni` command ships with it.** The CLI is bundled as a Tauri sidecar and
can be copied onto the PATH from *System → Command line*, into the same location
[`install.sh`](../install.sh) and [`install.ps1`](../install.ps1) use, honouring
the same `OMNI_INSTALL_DIR` override.

**It lives in the tray.** Sharing a keyboard and mouse is a background job, so
closing the window hides it rather than quitting. A connection request usually
arrives when the window is closed, so it is answerable from the tray menu and
announced with a notification. Quit is an explicit tray action — and because the
daemon is in this process, it stops input sharing. A daemon this app did not
start is left running.

## Layout

**General**, **Connections**, **System**, **Doctor**, **Update**. The first four
sections and their order come from the macOS app; **Doctor** is this client's
own, because only this client can run those checks (see below). Shared identity
comes from behaviour and information architecture, not from pixels — being a
webview app, it cannot follow the "native components only" rule the other two
clients do, and is deliberately exempted from it in
[`../docs/NATIVE_INTEGRATIONS.md`](../docs/NATIVE_INTEGRATIONS.md).

**Doctor** answers "is everything ok" — a verdict first, then each check with
what was found and how to fix it, failures sorted to the top. A failing check is
badged on the sidebar entry, because nobody opens Doctor on a machine they
believe is working, and that is exactly the machine that is quietly target-only.

Every `omni` subcommand is reachable from the window except `update` (needs the
updater plugin and a release endpoint) and `uninstall` (removing a bundled app is
the OS's job, not the app's).

## Developing

Requires [Bun](https://bun.sh), a stable Rust toolchain, and on Linux the
webview dependencies (`libwebkit2gtk-4.1-dev`, `libappindicator3-dev`,
`librsvg2-dev`, `patchelf`).

```sh
bun install
bun run tauri dev      # the app, with the frontend hot-reloading
```

The quality gate, which is what CI runs:

```sh
bun run test                      # the window
bun run build                     # typecheck and bundle the frontend
bun run stage-cli                 # see below — cargo will not compile without it
cd src-tauri
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test                        # the embedded daemon and its bridge
```

`stage-cli` is needed before **any** cargo command here, not just packaging:
Tauri's build script checks that every `externalBin` exists, so the crate does
not compile until the sidecar has been staged. `bun run tauri dev` and
`tauri build` do it for you; a bare `cargo test` does not.

Packaging also builds and stages the CLI sidecar
([`scripts/stage-cli.mjs`](scripts/stage-cli.mjs)):

```sh
bun run tauri build
```

A development build has no staged sidecar, so *Install* under **System → Command
line** is disabled and says why.

## Where things are

| Path | What it is |
| --- | --- |
| `src-tauri/src/daemon.rs` | Starting, stopping and supervising the embedded daemon |
| `src-tauri/src/ipc.rs` | The pass-through to the daemon's IPC, and the live subscription |
| `src-tauri/src/diagnostics.rs` | `omni doctor`, run in-process |
| `src-tauri/src/cli.rs` | The bundled `omni` command and installing it on PATH |
| `src-tauri/src/tray.rs` | The tray icon, and answering requests from it |
| `src-tauri/src/notify.rs` | The one notification worth interrupting for |
| `src/lib/ipc.ts` | The TypeScript mirror of the daemon's IPC types |
| `src/stores/daemon-store.ts` | The daemon's last snapshot, and every command |
| `src/views/doctor-view.tsx` | The health verdict and the checks behind it |
| `src/views/` | One file per section |

It sits **outside the Cargo workspace**: `src-tauri` declares its own
`[workspace]`, so `cargo build --workspace` at the repo root never pulls in Tauri
or the webview toolchain.
