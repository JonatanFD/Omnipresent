# Omnipresent — Native GUI Integrations

How native graphical clients fit into Omnipresent. This builds on the design in
[`ARCHITECTURE.md`](ARCHITECTURE.md), the product scope in
[`../CLAUDE.md`](../CLAUDE.md), and the engineering rules in
[`../.claude/rules/constrains.md`](../.claude/rules/constrains.md). It defines
the **constraints** every native client must obey, the **features** they expose,
and the **phased plan** to build them.

Target clients: a **macOS** app (Swift / SwiftUI) and a **Windows** app
(C# / WinUI 3). Linux stays CLI-only for now (see "Out of scope").

## Foundation: a GUI is just another IPC client

The daemon already exposes everything a client needs over **local IPC** — a
JSON-lines request/response protocol (`omni-runtime/src/ipc.rs`) over the
platform's native primitive: a **Unix-domain socket** (mode `0600`) on macOS and
a **named pipe** (local clients only) on Windows
(`omni-runtime/src/ipc_transport.rs`). The `omni` CLI is nothing more than a thin
client of this surface.

```
  macOS:   SwiftUI app  ┐
  Windows: WinUI 3 app  ┼── JSON / local IPC ──> omni daemon (Rust)
  CLI:     omni binary  ┘                          (Input/Topology/Session/
                                                    Security/Transport)
```

A native GUI is therefore **another client of the same IPC**. It reuses all the
core logic and touches none of it. The protocol is language-agnostic (JSON over a
socket/pipe), so a client can be written in Swift, C#, or Rust without changing
anything below the IPC.

## Constraints

These are binding rules for any native client.

### Architecture

1. **A GUI is only an IPC client.** It never imports, links, or re-implements the
   core crates (Input, Topology, Session, Security, Transport, Runtime).

   > **Exception — the cross-platform client (`omni/`).** It links `omni-runtime`
   > and starts the daemon on a background thread inside its own process, so that
   > installing the app is the entire installation. This changes *where the daemon
   > runs*, not *who owns the state*: the window still speaks the same JSON-lines
   > IPC over the same socket, and still re-implements nothing. Constraints 2, 3
   > and 4 hold for it unchanged. See "The cross-platform client" below.
2. **The JSON-lines IPC protocol is the only contract.** Clients render state and
   send commands; they never duplicate business logic (trust decisions, layout
   math, edge crossings, fingerprinting).
3. **New capabilities are added by extending the IPC protocol**, never by
   side-channels. New `Request` / `Response` / `Event` variants and new fields are
   **backward-compatible**: new fields are optional (`#[serde(default)]`) and new
   variants are additive. The protocol carries a **version**, sent at the start of
   a session, so a client can detect an incompatible daemon and tell the user to
   update rather than misbehave.
4. **The daemon owns all state.** Clients never read or write `trust.json`,
   `config.json`, certificates, or the log directly — the daemon is the single
   writer, with the correct permissions. Everything goes through the IPC.

### Language (the one exception to "Rust only")

5. The rule *"the language of the project is Rust"* still holds for **everything
   below the IPC**. The **GUI layer is the only exception**: Swift/SwiftUI on
   macOS, C#/WinUI 3 on Windows, TypeScript/React on the cross-platform client.
6. Native frontends live **outside the Cargo workspace** (e.g.
   `clients/omni-mac/`, `clients/omni-windows/`), each with its own toolchain and
   its own CI job (Xcode on a macOS runner, `dotnet` on a Windows runner). They
   must not appear in `Cargo.toml`'s `members` or affect `cargo build`.

### Security (clients inherit the daemon's posture and may not weaken it)

7. **Discovery and pairing are addressing, not authorization.** Finding a peer by
   mDNS, or pasting a pairing code, only makes a machine *reachable*. It never
   bypasses the explicit **accept/reject** step or **TOFU**. The first connection
   from any peer still waits for a human decision.
8. **No privilege elevation.** A client inherits the IPC's local-only access
   control (owner-only socket / pipe that rejects remote clients). It never runs
   as root/admin.
9. **Key material is never shown or logged.** Certificate keys and session secrets
   stay inside the daemon. The certificate **fingerprint** may be shown — it is
   exactly what the user verifies.
10. **The accept prompt must show the peer's name and fingerprint clearly.** This
    is the human verification point for TOFU on a first connection; it cannot be
    auto-dismissed or hidden behind an open window (hence a tray/menu-bar entry,
    below).
11. **Clipboard stays opt-in.** A client only flips it via the IPC
    (`Clipboard { enabled }`); it never reads the OS clipboard itself.

### Product

12. **The GUI does not replace the CLI.** Both are equal clients of the same
    daemon and must coexist.
13. The GUI **surfaces `doctor`** (permissions/environment) so a user can fix, for
    example, a missing macOS Accessibility grant.

### User interface

The default — and the rule — is that **each platform follows its own operating
system's design guidelines to the letter**. Cross-platform visual consistency is
an explicit non-goal: the two apps are *meant* to look different, each at home on
its OS.

14. **Platform design language is mandatory.** macOS follows the **Apple Human
    Interface Guidelines (HIG)**; Windows follows the **Fluent Design System** (via
    WinUI 3). Each app must look and behave like a first-party app of its OS.
15. **Native components only** — for the two *native* clients. Build exclusively
    from the platform's stock controls (SwiftUI/AppKit on macOS, WinUI 3 on
    Windows). No custom-drawn widgets that imitate native ones, no third-party UI
    toolkits, no web views, no porting one platform's controls to the other.

    > The cross-platform client is deliberately outside this rule: it is a webview
    > app and cannot follow it. It trades per-platform fidelity for one codebase on
    > all three operating systems. Rules 17 and 19 — honouring system appearance,
    > and accessibility — still bind it.
16. **Customization is layout only.** The single permitted modification is
    *arranging* native components to present omni's information. No restyling of
    controls, no custom themes, colors, typography, spacing, icons, or animations —
    nothing that overrides the system's own look.
17. **System appearance and settings are honored automatically.** Light/dark mode,
    accent color, Dynamic Type / text scaling, reduced motion, high contrast, and
    right-to-left must all just work. Using unmodified native components is exactly
    how that comes for free.
18. **Structural conventions follow each OS.** Window chrome, the macOS menu bar vs
    the Windows menu/command bar, the macOS menu-bar extra vs the Windows system
    tray, standard sheets/dialogs, the settings surface (macOS Settings window /
    Windows settings page), and notifications (User Notifications / Windows toasts)
    each follow their platform's norm — never a port of the other's.
19. **Accessibility and localization are not optional.** Full keyboard operability,
    screen-reader labels (VoiceOver / Narrator), and adequate contrast are
    required; unmodified native components are the way to get them. User-facing
    strings are localizable.

The same rule reaches the visual layout editor: it uses each platform's native
drag-and-drop idioms, not a bespoke canvas. Shared identity comes from **behavior**
(same features over the same protocol), not from pixels.

### Testing & stability

Stability is a priority: an interface that is not tested is not done. This extends
the project's TDD rule to the native clients, which live outside the Cargo
workspace and would otherwise escape it.

20. **Every interface is tested.** Each client of the daemon — the CLI and each
    native GUI — carries its own automated tests. At minimum: unit tests for the
    IPC client and the protocol's (de)serialization, plus integration/UI tests for
    the critical flows (connect, accept/reject, layout, clipboard, and the
    reconnection state the UI surfaces). macOS uses XCTest / XCUITest; Windows uses
    a standard .NET test framework (xUnit / NUnit / MSTest) with WinAppDriver for
    UI flows; the Rust side keeps its existing `cargo test` gate.
21. **Tests run in CI per platform and gate every change.** Each client's CI job
    runs its tests on every change, mirroring the Rust quality gate (fmt · lint ·
    test). A change does not merge with failing or missing tests, and stability is
    favored over feature velocity — no flaky or untested interface ships.

## The cross-platform client (`omni/`)

A third client, alongside the two native ones: **Tauri 2 + React + TypeScript**,
targeting macOS, Windows and Linux from one codebase. It exists to answer a
different question from the native apps — not "what looks most at home on this
OS", but "what can a non-technical user install in one step".

### Why it embeds the daemon

The native clients assume a daemon is already installed and running. That is two
installations for the user: the daemon, then the app. This client removes the
second step by **linking `omni-runtime` and starting the daemon on a background
thread inside its own process** (`omni/src-tauri/src/daemon.rs`). Installing the
app installs everything — including the `omni` command, which is bundled as a
Tauri sidecar and can be put on the PATH from System.

Four consequences follow, and they are all deliberate:

- **The window still talks over IPC**, to the daemon in its own process, exactly
  as the CLI does. The core is used unmodified and owns all state, so constraints
  2–4 are untouched. It also means the window works unchanged against a daemon
  that was already running — if one owns the socket, the embedded one stands down
  and the window simply talks to the one that is there.
- **The daemon has to be startable, not only stoppable.** It lives on a thread of
  this process, so a `Supervisor` owns that thread: stopping the daemon without a
  way to start it again would leave the app running with nothing to talk to. The
  supervisor also distinguishes a daemon that failed from one that stood down for
  another that was already there — the second is normal and is not reported.
- **Closing the window must not stop input sharing**, so the app lives in the
  **system tray**: closing hides the window, and Quit is an explicit tray action.
  A closed window is the usual state when a peer asks for control, so the tray
  menu carries the waiting requests and a notification announces each new one.
- **Quitting the app stops the daemon**, because they are one process. This is the
  honest reading of what the user asked for by choosing Quit — and it stops it
  through its own IPC, so sessions close and the socket is released rather than
  being cut off. A daemon this app did not start is left running.

### Why `doctor` can live here and not in the native clients

`omni doctor` is not an IPC request: the CLI runs the checks in its own process
against `omni_runtime::doctor`. A client that speaks only IPC therefore cannot
offer it without an additive `Doctor` message. This one links the runtime, so it
runs the same checks the same way (`omni/src-tauri/src/diagnostics.rs`).

This is the one capability the embedding buys that is not just packaging, and it
is worth having: in a terminal a missing Accessibility permission is one command
away from being explained, while in a window it shows up only as a machine that
mysteriously cannot drive anything.

### Layout

The macOS app's four sections, in the same order — General, Connections, System,
Update — with the daemon status shown in the sidebar and the count of pending
requests badged on Connections. Shared identity comes from behaviour and
information architecture, not from pixels.

It adds a fifth, **Doctor**, which is the one deliberate departure from that
parity: it exists because this client *can* run the checks and the others
cannot (see below). It gives a verdict before the detail, and badges a failing
check on the sidebar — an unchecked machine is reported as unchecked, never as
healthy, since a green light nobody earned is the one answer no user would think
to question. When the native clients get `doctor` over an additive `Doctor`
request, they should adopt the same pane and the parity is restored.

Every `omni` subcommand is reachable from the window except two. `update` needs
the Tauri updater plugin, a signing key and a release endpoint, and the pane says
so rather than offering a button that does nothing. `uninstall` deletes a binary,
which is the wrong shape for a bundled app — removing one is the OS's job.

### Toolchain

React 19, Vite, Tailwind CSS v4, shadcn/ui (Base UI primitives) and zustand for
state. It sits **outside the Cargo workspace**: `omni/src-tauri` declares its own
`[workspace]`, so `cargo build --workspace` at the repo root never pulls in Tauri
or the webview toolchain (constraint 6).

Its quality gate (constraint 21) runs on Linux, macOS and Windows in the
`Desktop client` workflow: the window's tests, a typecheck, `cargo fmt`, clippy
with warnings denied, the Rust tests, and a packaging build that also proves the
CLI sidecar stages. The path filter includes `crates/**`, because this client
depends on the core by path and a change there can break it without touching
anything under `omni/`.

## IPC evolution: a live event channel

The current IPC is request → response with no push. A responsive, *lightweight*
GUI needs live state and instant notification of incoming requests. Polling (the
GUI asking `Status` every second forever) wastes work while idle and still lags;
an **event channel** does nothing until something changes and notifies instantly,
so it is both lighter at runtime and better UX.

Plan: add a `Request::Subscribe` that keeps the connection open, after which the
daemon **pushes `Event` lines** as state changes — session opened/closed, active
target changed, an incoming request arrived or was resolved, a discovered peer
appeared/disappeared, clipboard toggled. This is a small, well-scoped Rust change
in the daemon, written test-first, and it benefits the CLI too (e.g. a future
`omni watch`).

## Connection experience (no IPs in the user's face)

The user should rarely, if ever, see an IP. Three mechanisms, all resolving to an
address **under the hood** (shown only in an "advanced/details" view):

1. **Local-network discovery (mDNS / Bonjour).** The daemon advertises itself and
   browses for other omni daemons on the LAN, so the GUI lists peers by **friendly
   name** ("Jonatan's MacBook") and the user clicks to connect — no IPs, no codes.
   This is the product's primary case (machines side by side). A pure-Rust mDNS
   crate (e.g. `mdns-sd`) keeps the dependency tree free of C libraries. Clicking a
   discovered peer still triggers the normal accept/TOFU flow (constraint 7).
2. **Pairing code (reversible, fingerprint-bound).** For when discovery does not
   apply or the address is shared out of band (chat, voice). A machine shows its
   code; the other pastes it and connects. The code **encodes the address plus
   enough of the machine's certificate fingerprint to verify it**, so the dialer
   can reject a man-in-the-middle presenting a different certificate — turning the
   copy/paste into an out-of-band authenticated channel. It is *not* a one-way
   hash (a hash cannot be dialed); it is a compact **encoding**. Format: short
   **alphanumeric** in groups, Crockford base32 (no ambiguous `0/O/1/I`), e.g.
   `OMNI-7F3K-9Q2M-…`. The fingerprint verifies the cert but **does not** replace
   the accept step (constraint 7).
3. **Friendly names + saved peers.** Every machine has an editable name. After the
   first pairing the peer is remembered (the trust store already persists it), so
   the user reconnects by name and never sees an address again.

These connection features are mostly **daemon + protocol** work (Rust), and they
also enrich the CLI (`omni discover`, `omni code`, `omni connect-code <code>`).

> **Scope:** discovery and codes target the **LAN** case. Connecting **across the
> internet** (NAT traversal, relays) is a separate, larger effort and is out of
> scope for now.

## Feature inventory

Where each feature lives: **[D]** daemon, **[P]** IPC protocol, **[G]** GUI.

### Connection
- mDNS discovery: advertise + list LAN peers. **[D][P][G]**
- Pairing code: generate mine / connect by pasting one; fingerprint-bound and
  verified. **[D][P][G]**
- Editable friendly name per machine. **[D][P][G]**
- Reconnect by name from saved peers. **[P][G]**

### Session / control
- Live state (sessions, role, which peer has input) via the event channel.
  **[D][P][G]**
- Accept/reject popup showing **name + fingerprint**. **[P][G]**
- Connect / disconnect / forget a peer. **[P][G]**

### Layout
- Visual edge editor (a friendlier `omni layout`). **[G]** — already covered by the
  current protocol.

### System
- Clipboard opt-in toggle. **[P][G]** — protocol already exists.
- `doctor` (permissions/environment) in the UI. **[P][G]** — already exists.
- Version indicator + "incompatible daemon, please update" notice. **[P][G]**

## Layout editor: scope

Topology currently models the desktop as **edge links** — each peer sits *past*
the left/right/top/bottom edge of another — not as free 2-D pixel positions. The
visual editor therefore maps a drag to **one of the four edges** (it is a nicer
`omni layout <host> <edge>`), which covers the common case ("put the Mac to the
left of Windows"). A free 2-D arrangement with arbitrary offsets would require
changes in Topology and the protocol and is deferred (see "Out of scope").

## App model

A **window plus a tray / menu-bar entry**:
- The **window** hosts the peer list, the visual layout editor, settings, and the
  pairing-code / discovery views.
- The **tray / menu-bar** entry gives quick status and, crucially, delivers the
  **accept/reject prompt even when the window is closed** — a security decision
  point cannot depend on the window being open (constraint 10).

## Client implementation strategy

How to reconcile a Rust core with native UIs in Swift and C#. There are three
shapes: (1) two separate native apps that each speak the IPC; (2) a shared Rust
client library exposed over a C ABI and called from Swift/C# via FFI (UniFFI,
interoptopus, csbindgen); (3) one cross-platform Rust GUI (rejected — not native).

The choice between (1) and (2) turns on **how much logic the client carries**, and
by design ours carries almost none: the daemon does all the work (networking,
trust, discovery, pairing, reconnection), so a client only opens the socket/pipe,
writes a JSON line, reads JSON lines, and renders (constraint 2). That makes
**option (1) the right call** — FFI's cost (xcframework/DLL build wiring,
async event streams across the FFI boundary, manual memory management) is not
worth sharing such a thin client.

The decision, then:

1. **All logic lives in the daemon (Rust).** This includes the new connection
   features: the pairing code is **generated and resolved in the daemon**
   (`GenerateCode` / `ConnectByCode`), so a client only passes the string through;
   discovery, fingerprint verification, and reconnection are likewise daemon-side.
   A client never encodes/decodes or decides anything.
2. **The thin IPC client is written natively per platform** — it is small and
   first-class on both: a Unix-domain socket + `Codable` on macOS (Swift), a
   `NamedPipeClientStream` + `System.Text.Json` on Windows (C#).
3. **Protocol types are generated from Rust to avoid drift.** The only thing
   clients share is the *shape* of `Request` / `Response` / `Event`. Rather than
   hand-maintaining it in three languages, generate the native types from the
   definitions in `omni-runtime/src/ipc.rs` — `typeshare` for Swift, and
   `schemars` (Rust → JSON Schema) + quicktype (or a typeshare fork) for C#. The
   Rust structs stay the single source of truth, with no binary coupling.
4. **FFI (a shared Rust client lib) is reserved for later** — only if client-side
   logic ever grows non-trivial. The constraints above are designed to keep that
   from happening (the daemon is what thinks).

In one line: it is not "Swift + C#" *versus* "Rust" — it is **logic in Rust
(daemon), UI native per platform, protocol types codegen'd from Rust**; the thin
client is reimplemented per platform because that is cheap, and is not shared over
FFI because there is too little logic to share.

## Phased plan

1. **Event channel + protocol version (daemon, Rust).** `Request::Subscribe`, an
   `Event` enum, and a version handshake. Define these types so they are
   **codegen-friendly** (e.g. derive `schemars`/`typeshare` on the protocol
   types) so native clients can generate matching structs. Test-first. Unblocks
   both GUIs and improves the CLI.
2. **Connection backend (daemon, Rust).** mDNS advertise/browse, machine name in
   config, pairing-code generate/parse with fingerprint verification, and the IPC
   surface for all three. Mirrored in the CLI.
3. **macOS app (Swift / SwiftUI).** Unix-socket client; menu-bar + window;
   discovery and pairing-code views; visual edge editor; accept/reject; doctor.
   Packaging (signing, notarization) as a sub-task.
4. **Windows app (C# / WinUI 3).** Named-pipe client; tray + window; same views.
   Packaging (signing) as a sub-task. **Status:** first implementation landed in
   `clients/omni-windows/` (thin `Omni.Ipc` client, `Omni.App.Core` view models,
   `Omni.App` Fluent window; 27 unit tests; Windows CI). The window-based views
   (status, accept/reject, connect/disconnect, peers, layout, clipboard, version
   notice) work; the **tray entry**, **discovery/pairing** (await Phase 2), and
   **doctor** (awaits a new IPC request) are still to do. Brought forward ahead of
   Phase 3 because the owner is doing the macOS app.

## Out of scope (for now)

- **Free 2-D layout** with arbitrary offsets (needs Topology + protocol changes).
- **Internet connectivity** (NAT traversal / relays); discovery and codes are
  LAN-oriented.
- **Linux GUI.** Linux stays CLI-only; a native or cross-platform Linux client is
  deferred.
