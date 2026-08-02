# Omnipresent — Project Status

A snapshot of what exists today and what comes next. For the *why* behind the
module boundaries, see [`ARCHITECTURE.md`](ARCHITECTURE.md); for product scope
and rules, see [`../CLAUDE.md`](../CLAUDE.md) and
[`../.claude/rules/constrains.md`](../.claude/rules/constrains.md).

_Last updated: 2026-08-02 (**the keyboard now follows the cursor onto the other
machine.**

There is one cursor, and whichever machine it is sitting on is the machine every
keyboard and mouse should be working on — whichever computer they happen to be
plugged into. Half of that was true: a machine learned when it had been taken
over, so it stopped sending its keystrokes away and took them back to its own
screen. Nothing ever told it the cursor had left again. So after the cursor came
back to the Mac, the PC's keyboard went on typing on the PC — the pointer was on
one machine and the letters appeared on the other.

Both halves of a hand-over now travel. A `CursorWarp` already said *you* have the
cursor; a `CursorReturned` says *I* do, and it is sent the moment a cursor crosses
back onto its own machine's screen. A machine that receives one points its input
at the sender (`SessionManager::follow_peer`) instead of at its own desktop, and
adopts the position the message carries, so the one shared cursor goes on being
tracked from where it actually is. Exactly one machine holds the cursor at any
moment, and every keyboard in the room types on it.

The version before had made control reversible for the mouse, which is what
turned this up: until then whoever crossed first stayed in control for good, so
the keyboard was never left behind. Both machines need this version — a peer on
an older one cannot answer a message it does not know.

One rough edge remains, and it is not new: a machine that is *not* holding the
cursor still keeps its own copy of where that cursor is, and only hears about it
at a hand-over. Move the Mac's mouse around the Mac for a while and then reach
for the PC's mouse, and the pointer jumps once to where the PC last knew it
before carrying on normally. Making the machine that holds the cursor the only
one that tracks it — with the others sending plain movement rather than
positions — is the fix, and it is on the list below.

Earlier the same day: **control began working in both directions, and
scrolling at each machine's own speed.**

Whichever machine's mouse the user reaches for is the one that drives. That was
the intent all along — both ends capture, and either one's cursor can cross onto
the other — but in practice only the machine that dialed could actually control
the other. Pushing a Mac's cursor onto a PC moved the pointer there and nothing
more: no clicks, no keystrokes. Two things were missing.

**Windows swallowed the input it was sent.** Its low-level hooks check the
"injected" flag before *converting* an event, so the daemon never captured its
own output — but they then swallowed **every** event while suppressed, including
the ones a peer was sending to this machine. So a PC that had crossed onto a Mac
sat suppressed, and from that moment the Mac's clicks and keys were eaten by the
PC's own hook before any application saw them. The pointer still moved, because
`SetCursorPos` repositions it whether or not the resulting message survives —
which is exactly what "it moves but does nothing" looked like. An injected event
is now left alone by both halves of the hook. macOS never had the fault: its tap
checks for its own marker before it decides to drop anything, which is why
Windows → Mac always worked.

**Nothing said who was in control.** Each machine decided for itself when its
cursor crossed, so both could believe they were driving the other at once — each
withholding input from its own desktop while sending it away, so neither acted on
what it received. A `CursorWarp` is only ever sent at the moment a cursor crosses
onto a peer, so it now carries that meaning: a machine that receives one gives up
whatever it was driving (`SessionManager::yield_control`) and takes its input
back to its own screen. Exactly one of the two ends up in control, and it is the
one whose user just moved.

**Scrolling had two faults on Windows.** A precision touchpad or a
high-resolution wheel reports a fraction of a notch at a time, and capture
divided by `WHEEL_DELTA` before doing anything else — so every one of those
floored to zero and scrolling with them did nothing at all. And neither end read
the machine's own "lines to scroll per notch" setting: a notch travelled as one
line whatever the user had chosen, then Windows applied the setting *again* on
injection, so anything arriving scrolled several times too far. Both ends now use
it — multiplied in on capture, divided out on injection, since Windows puts it
straight back — and the conversion scales before it divides, carrying whatever
does not divide evenly into the next event. The arithmetic lives in one place
(`scroll::Scaled`) and is tested on every platform. macOS was already right: its
fixed-point line delta has that Mac's own scrolling speed applied to it.

Earlier: **a QUIC security advisory closed.**
`quinn-proto` is on 0.11.15, which fixes GHSA-4w2j-m93h-cj5j — a peer that
sends stream fragments while leaving out earlier parts makes the receiver's
reassembly buffer grow without bound, exhausting memory (CVSS 7.5). Reaching it
takes a peer that already completed the mTLS handshake and passed TOFU and the
allowlist, so an anonymous machine on the network cannot, but a peer that turns
hostile or is compromised can. It applies squarely here: the clipboard's bulk
streams are read sequentially, which is the shape the advisory describes.

Earlier the same day: **scrolling now runs at the speed each machine's own
owner chose.** Scrolling travelled as pixels, and the two sinks disagreed about
what to do with them: Windows converted back to wheel notches and let Windows
scale them by the user's "lines to scroll per notch", while macOS injected the
pixels as final. So a notch sent from a PC moved a Mac by ten literal pixels —
a fraction of what one notch should do — and the Mac's own scrolling-speed
setting never got a say. The wire now carries **lines** (in thousandths, so a
trackpad's fractions survive) instead of pixels: a machine reports *how far the
wheel turned* and the machine receiving it decides *how much that should
scroll*, using its own settings. macOS injects with `ScrollEventUnit::LINE` and
its source reads the fixed-point **line** delta rather than the pixel delta,
which had this Mac's speed already applied — a decision that belongs to the
machine on the other end. One shared `ScrollAccumulator` holds back movement
shorter than a whole line on all three platforms, replacing the same remainder
arithmetic written out separately in each sink.

**Copying a big image no longer drops the
connection.** Clipboard payloads shared the reliable control stream with
signalling, and the peer task wrote them itself — so while a screenshot's
several megabytes went out, that task ran nothing else: it sent no heartbeat,
read none from the peer, and refreshed no deadline. After eight seconds each
side concluded the other had died and tore the session down, which is exactly
what a user saw on copying a large image. The receiver had the same problem,
reading the whole frame inline. Bulk payloads now travel on **their own QUIC
stream**, one per transfer, written and read by tasks of their own: the peer
loop never waits on a large payload in either direction, so heartbeats keep
flowing however big the copy is. Sending is sequential per peer, so a newer copy
cannot overtake an older one still in flight. With signalling no longer sharing
the stream, its frame limit drops back to 64 KiB — it had been inflated to 64
MiB purely to let clipboard through — and the bulk stream carries its own limit.
The size caps themselves were reviewed and left alone: they were never the
cause, and an oversized payload was already skipped with a warning rather than
breaking anything.

Earlier: a **cross-platform audit** and its fixes: display
geometry, modifier keys, local IPC, input, packaging, and the **Windows GUI**
brought to layout parity with the macOS app.

**The cursor no longer changes speed when it crosses.** A screen's size is
reported in whatever unit its OS uses, and those units are not the same size: a
Retina Mac calls itself 1512 wide in points, a DPI-aware Windows machine calls a
4K panel 3840 wide in pixels. Movement was applied to the peer's screen one unit
for one, so the cursor crawled on one machine and raced on the other. A movement
is now rescaled to cover the same *fraction* of the screen it is on, and a
movement that was not zero never rounds away to nothing.

**A second monitor no longer hands control to a peer.** Only the primary display
counted, so a cursor moved onto another monitor was recorded as pinned to the
primary's edge — and the next nudge that way looked exactly like the user pushing
past the edge, so control jumped to whichever peer sat there. Both adapters now
report the whole desktop (Windows' virtual screen, the union of macOS's active
displays) and translate between the OS coordinates and a 0-based desktop space at
the boundary, so a monitor above or to the left — where Windows uses negative
coordinates — is handled without the rest of the system knowing.

Re-reading the geometry when displays change is still to do: docking or
unplugging a monitor needs a daemon restart to be picked up.

**Arm64 Windows is no longer left out.** The GUI project declared `win-arm64`
but the release only ever built x64, `omni update` had no triple for it, and
`install.ps1` hard-coded x86_64 — so an Arm Windows machine got nothing, while
Apple silicon and Intel were both covered. Both the CLI and the GUI now ship for
x64 and arm64, and the installer picks by machine. The macOS app's deployment
target dropped from 26.5 to 14.0, which is what its APIs actually need
(`@Observable`, `ContentUnavailableView`) rather than the version it happened to
be created on.

**Command and Control can be swapped per peer.** Copy is Command-C on a Mac and
Control-C everywhere else, and each machine faithfully sends the key it was
given — so a Mac driving a PC sent Windows-C and a PC driving a Mac sent
Control-C, neither of which copies. Nothing in the protocol says what a peer runs
on, and plenty of people want the keys left alone, so this is a per-host choice:
`omni modifiers <host> meta-control`. The controller relabels events on the way
out (both the key code and the modifier set carried with it, or the two would
disagree), so the target injects what it receives and needs to know nothing. Off
by default.

**A shutdown deadlock, found while verifying the rest.** After `omni stop` the
daemon logged "shutting down" and then never finished exiting: a subscription
task sat reading from a client that was itself waiting for the daemon to close
the connection, so neither moved. Any client that stays subscribed hits it —
which is exactly what both native GUIs do — leaving `omni stop` reporting
success while the daemon kept its socket and a later `omni start` said it was
already running. Subscriptions now watch a shutdown signal and end on their own.
The daemon integration test used to hang forever on Windows because of it; it
now finishes in a third of a second.

**Owner-only IPC on Windows.** The Unix socket is created `0600`, but the named
pipe was created with the system default security — weaker than the promise that
only the owner can command the daemon. It now carries a protected DACL with a
single entry for the SID of the user the daemon runs as. The CLI also retries a
*busy* pipe for a moment instead of reporting "the daemon is not running": the
daemon arms the next instance only after a client takes the current one, so two
clients arriving together could briefly collide and get a misleading answer.

**`omni start` now really detaches on Windows.** The daemon inherited its
parent's console, and Windows sends the console-close event to every process
attached, so closing the terminal killed the daemon — the opposite of what the
command promises. It gets its own process group and no console.

The TLS private key is now locked down *before* its bytes are written on both
platforms, rather than written and then tightened.

**Input, both directions.** A wheel notch now means the same amount of scrolling
whichever way a session runs: the unit lives in the shared vocabulary as
`PIXELS_PER_WHEEL_NOTCH` instead of Windows and Linux each picking their own
number with nothing tying it to what macOS reports. **Keypad Enter** survives the
trip — Windows puts it on `VK_RETURN` and separates it with the extended bit,
which capture ignored and injection had no entry for, so it used to vanish
entirely. **Caps Lock** is treated as the latch it is: its state comes from the
macOS event flags rather than being inferred from presses, and a change sends a
tap so the other machine flips its own latch (it used to arrive as two taps and
leave the other side stuck on). Injecting Caps Lock *into* macOS still does
nothing — only IOKit HID can move that latch — which is now documented in the
adapter rather than left to be discovered.

**Windows injection and capture robustness.** The Windows sink ignored the
`modifiers` each key event carries and relied on the OS state built from received
key-downs; since input rides unreliable datagrams, one lost packet produced the
wrong chord or left a modifier stuck down for good. It now compares what it has
injected against what the controller had held and makes up the difference before
pressing the key — the self-correcting behaviour macOS already had from stamping
flags on every event. And because Windows silently removes a low-level hook whose
callback overran `LowLevelHooksTimeout` without telling anyone (capture stops,
`poll` still answers "nothing right now", and `omni status` goes on claiming
capture is running), the hook thread now re-arms both hooks on a timer and gives
up the event channel if that fails, so the daemon stops advertising capture that
is not happening. macOS handles the same situation through its explicit
`TapDisabledByTimeout` event.

**The Windows GUI** was brought to layout parity with the macOS app, and three
client defects fixed along the way. The panes now match
`MainView.swift` section for section: no title bar of its own, the daemon's state
as a dot on the General navigation entry, a badge with the waiting-request count
on Connections, the section name as the detail header, the same section order and
labels in every pane, one edge picker per peer that applies immediately (the host
field and "Place" button are gone), and a centred "Daemon Not Running" pane when
there is no daemon and nothing remembered. Two new components — `LabeledRow` and
`LayoutRow` — carry the row layouts, and the parity contract is written down in
`clients/omni-windows/ARCHITECTURE.md`. The defects: the app looked for
`omni.exe` only under `C:\Program Files\Omnipresent` and `~/.cargo/bin`, so
`Start daemon` and `Update` failed for everyone who installed with the documented
`install.ps1` (which writes to `%LOCALAPPDATA%\Programs\omni`) — a new
`OmniBinaryLocator` checks `OMNI_INSTALL_DIR`, the installer's default, a
machine-wide install, a cargo install, and finally `PATH`; the run loop caught
only `OmniDaemonException`, so an unexpected error (a decode failure from an
event a newer daemon added — which the IPC contract calls backward-compatible)
escaped and froze the window on stale state forever; and `ReconnectNow` was an
empty method, so the UI sat out the reconnect delay after starting the daemon.
Earlier: the **Windows GUI client** was reorganized into a
modular architecture matching the macOS app, and shipped — with a Windows
uninstall fix — as **v0.5.0**. The monolithic main window split into four views
(status, connections, peers, settings) behind a `NavigationView` + `Frame`, and
the duplicated connection markup collapsed into three reusable card components
(peer, session, pending request) — same behaviour, far less XAML, ready to reuse
in later views. Alongside it, an `omni uninstall` fix for Windows: the OS locks a
running `.exe`, so the binary could not delete itself and lingered after the
config was already gone; uninstall now writes a short detached cleanup script
that waits for the process to exit and then removes the binary (Unix still
deletes in place, since the inode survives). The v0.5.0 release tag also carries a
`Cargo.lock` refresh so the `--locked` release build reports 0.5.0 instead of
failing on a stale lock. Earlier: the **Windows GUI client (WinUI 3)** landed its
first
implementation under `clients/omni-windows/` — a thin IPC client over the daemon's
named pipe, MVVM view models, and a Fluent window that shows live status and drives
accept/reject, connect/disconnect, peers, layout, and the clipboard toggle. Three
.NET projects (`Omni.Ipc`, `Omni.App.Core`, `Omni.App`) with 27 unit tests, plus a
per-platform CI job. To make this possible the daemon's Windows **named-pipe name**
is now a stable SHA-256 of the state directory (it was an unspecified `DefaultHasher`
that a non-Rust client could not reproduce); Rust and C# assert the same vector so
they never drift. The tray entry, mDNS/pairing discovery, and `doctor` in the UI are
still to come. Earlier: double-click now works when injecting on macOS. The
sink hardcoded each mouse event's `kCGMouseEventClickState` to 1, so a quick
two-click sequence from a remote controller (Windows → Mac) arrived as two
single clicks; it now tracks the previous click and stamps the running
single/double/triple count. Earlier the same day: input path tuned for latency
under network congestion. Cursor motion is sent as absolute positions on
unreliable
datagrams; on a congested or busy link a backlog of stale positions used to
queue and replay, so the remote cursor visibly lagged. The peer task now
**coalesces** a run of queued positions down to the most recent before sending
(clicks, keys, and scrolls are order-sensitive, so they are never dropped and
they break a run), quinn's datagram send buffer is shallow (it drops the oldest
stale positions to admit a fresh one), and the connection uses the **BBR**
congestion controller to keep the bottleneck queue — and the added latency —
small. Automatic reconnection on a dropped link is the next piece. Earlier the
same day: clipboard **image** sharing now works between two machines. Images
travel on the reliable control stream, but that stream capped
every frame at 64 KiB — fine for tiny signalling, far too small for a
screenshot's raw RGBA bytes — so the receiver rejected the frame as
`ControlFrameTooLarge` and tore the session down the moment an image was copied.
The control-frame limit now admits a full clipboard payload (`MAX_CLIPBOARD_BYTES`
plus framing overhead), so text and images both sync. Validated on Windows and
macOS; Linux clipboard image sync is still unverified (see "Not yet done").
Earlier: clipboard sharing can be toggled at runtime with
`omni clipboard on|off` — the daemon flips the opt-in guard, wakes or parks the
polling task, and persists the choice to the config so it survives a restart;
`omni status` shows the current state. The poll task now parks for free while
sharing is off, so the opt-in default costs nothing. Earlier the same day: a
TOFU stale-pin fix from a Windows↔macOS run:
accepting a host whose certificate rotated now **replaces** its pin instead of
appending a second one, and the trust store collapses any duplicate host pins
on load. Before this, a stale pin could shadow the current one, so dialing a
peer that had reconnected the other way refused its certificate
(`ApplicationVerificationFailure`); the connect error now points at
`omni peers remove`. Earlier the same day: a high-DPI fix making the Windows
process per-monitor DPI aware so screen size, captured deltas, and the parked
cursor all use real pixels — without it a scaled 2K/4K laptop dragged the
remote cursor into a corner and left the keyboard stuck on the remote; plus
cursor-visibility fixes — the macOS sink warps the cursor so a remote-driven
move stays drawn, and the Windows source parks the local cursor instead of
hiding it with the OS-global, crash-persistent `SetSystemCursor`.)_

## Where we are

The project is **feature-complete for a first end-to-end build, on all three
target platforms**. Every crate is implemented: the shared-kernel **Protocol**,
**Topology** (virtual desktop and edge crossings), **Security** (allowlist +
TOFU trust policy), **Session** (lifecycle, roles, input routing), **Input**
(real macOS, Linux, *and Windows* adapters), **Transport** (real QUIC adapter),
the **Runtime** daemon that wires them all together, and the **CLI** (`omni`)
that drives it over local IPC. The full pipeline — capture → route → QUIC
datagram → inject, with TOFU handshake and accept/reject flow — exists in code
and builds on macOS, Linux, and Windows; what it has *not* had yet is a live
two-machine run (see "Not yet done").

The secure channel is **QUIC** (TLS 1.3 over UDP), via `quinn` + `rustls` —
adopted in place of the originally planned DTLS 1.3 (see "Open decisions").

Local IPC is a **Unix-domain socket** on macOS/Linux and a **named pipe** on
Windows, behind one transport abstraction; sessions now exchange
**heartbeats** and drop a silently-dead peer; the screen arrangement is
**configurable** (`omni layout`) rather than a fixed left/right chain; and a
**daemon-level integration test** drives two daemons through the real QUIC +
IPC path. A **GitHub Actions** workflow runs the quality gate on all three
platforms.

The whole workspace builds clean under `cargo fmt`, `cargo clippy -D warnings`,
and `cargo test` (189 tests), including on Windows.

## Crate status

| Crate            | Status        | What's there                                                                 |
| ---------------- | ------------- | ---------------------------------------------------------------------------- |
| `omni-protocol`  | **Implemented** | Ids, input events, control messages (incl. screen sizes and the `CursorWarp`/`CursorReturned` handover pair), clipboard payloads (`ClipboardData`/`ClipboardImage`, size-capped + overflow-checked), and the postcard wire codec. Scrolling is measured in lines, not pixels, so the machine receiving it sets the speed. 36 tests. |
| `omni-topology`  | **Implemented** | Virtual desktop layout, edge crossings, and the `LayoutStore` port. 13 tests. |
| `omni-security`  | **Implemented** | Allowlist + TOFU trust policy, `TrustStore`/`CertProvider` ports, self-signed identity generation. 15 tests. |
| `omni-session`   | **Implemented** | Session lifecycle, dynamic roles, active-target routing, `SessionEvents` port. `yield_control` gives way when a peer takes over and `follow_peer` sends this machine's input after a cursor that has gone back to the peer — the two halves that keep exactly one machine holding the cursor, and every keyboard pointed at it. 18 tests. |
| `omni-input`     | **Implemented** | Ports, in-memory adapters, permission diagnostics, and the real OS adapters: macOS (CGEvent tap + post; the sink warps the cursor so a remote-driven move stays visible and stamps the click-count so double/triple clicks register), Linux (evdev + uinput), and Windows (low-level hooks + SendInput; the local cursor is parked, not hidden). Cursor **hiding** on suppression where it is safe and self-restoring (macOS `CGDisplayHideCursor`, Linux X11 empty-cursor on the root window) and true Linux cursor-position query (`XQueryPointer`). A shared `ScrollAccumulator`, over the `Scaled` fraction-carrying primitive, converts the wire's lines into each OS's own scroll unit and back — on Windows through that machine's own "lines to scroll per notch", so a fraction of a notch is never floored away and the setting is applied exactly once. The Windows hooks leave injected events alone in both respects, so a peer driving this machine is not swallowed by its own suppression. 46 tests. |
| `omni-clipboard` | **Implemented** | Opt-in clipboard sharing (text + images) over a ports-and-adapters design: `arboard` adapter, in-memory mock, echo-loop guard, strict opt-in toggle queryable at runtime. 8 tests. |
| `omni-transport` | **Implemented** | `SecureChannel` port, framing, loopback channel, and the real QUIC adapter (quinn + rustls, mTLS, TOFU verifiers, datagrams, control stream, and a bulk stream per clipboard transfer so a large payload cannot delay the heartbeats that keep a session alive). The input path is tuned for latency: a shallow datagram send buffer (drops oldest stale positions) and the BBR congestion controller. 15 tests. |
| `omni-runtime`   | **Implemented** | The daemon: config/paths, persistent identity, trust store, rate limiter, cross-platform IPC (Unix socket / Windows named pipe), heartbeats, configurable layout, opt-in clipboard sync on its own bulk stream, handed to a per-peer sender task so the loop that keeps the session alive never waits on a large write (toggleable at runtime and persisted), doctor checks, and the composition root that runs the pipelines. The peer task coalesces a backlog of queued cursor positions to the latest one (keeping clicks/keys/scrolls intact) so a congested link does not make the cursor lag. A live IPC event channel (`Subscribe` → `Event` snapshots, push not poll) and a protocol-version handshake (`Hello`) back native GUI clients. 35 tests + two integration tests. |
| `omni-cli`       | **Implemented** | The full `omni` binary: start/stop/status, doctor, connect/disconnect, accept/reject, peers (+ remove), layout, clipboard on/off, uninstall, over the daemon's Unix socket. |

### What `omni-protocol` provides

- **Identifiers** (`ids`): `MachineId`, `PeerId`, `SessionId`, and `Fingerprint`
  (a 32-byte SHA-256 digest that renders as lowercase hex for TOFU pinning).
- **Input events** (`input`): a platform-neutral `InputEvent` with `Key`,
  `Motion`, `Button`, and `Scroll` variants; `KeyCode` (USB HID usage codes),
  packed `Modifiers`, `MouseButton`, `MouseDelta`, `ScrollDelta` (measured in
  `MILLILINES_PER_LINE` units — thousandths of a line, so the receiving machine
  applies its own scrolling speed and a trackpad's fractions are not rounded
  away).
- **Control messages** (`control`): `ControlMessage` (`ConnectRequest`, `Accept`,
  `Reject`, `Disconnect`, `Heartbeat`, `CursorWarp`, `CursorReturned`) and
  `RejectReason`. The last two are the two halves of a handover, each sent only
  at the moment a cursor crosses: `CursorWarp` means "you have the cursor now",
  `CursorReturned` means "I do, and here is where it landed on my screen". Either
  one tells the receiver where to send its keyboard and mouse. New variants go at
  the end of the enum, because postcard encodes one as its position in the list.
- **Wire codec** (`wire`): the `Message` envelope (`Input`, `Control`,
  `Clipboard`) plus `encode`/`decode` over
  [postcard](https://docs.rs/postcard) — a compact varint binary format chosen
  for small datagrams and low-latency (de)serialization. Truncated or empty
  input is rejected.
- **Handshake payloads**: `ConnectRequest` carries the initiator's screen size
  and `Accept` carries the target's machine id and screen size, so each side can
  place the other in its virtual desktop layout.
- **Clipboard payloads** (`clipboard`): `ClipboardData` (text or `ClipboardImage`)
  with overflow-checked dimension validation and a 64 MiB payload cap, so a
  malformed or oversized payload is rejected before it is sent or applied.

### What `omni-topology` provides

- **Geometry** (`geometry`): `Screen`, `Point`, and `Edge` (with `opposite` and
  orientation helpers).
- **Layout** (`layout`): `Machine` and `VirtualLayout` — an edge-link arrangement
  where each machine knows the neighbor past each edge (kept symmetric). `advance`
  moves the cursor by a `MouseDelta` and either stays on screen, clamps at a
  neighborless edge, or crosses onto the neighbor's opposite edge, mapping the
  position along the shared edge proportionally so crossings stay seamless across
  differently sized screens.
- **Store** (`store`): the `LayoutStore` port plus an in-memory adapter.

### What `omni-security` provides

- **Trust policy** (`trust`): `AllowList`, `PeerIdentity`, and a pure `evaluate`
  function returning a `TrustDecision` — allowlist gate first, then TOFU (unseen
  → `TrustOnFirstUse`, matching pin → `Trusted`, changed pin →
  `FingerprintMismatch`). `TrustAuthority` applies it against a store and records
  approvals (`accept`/`forget`).
- **Store** (`store`): the `TrustStore` port (allowlist + pinned fingerprints)
  plus an in-memory adapter.
- **Identity** (`identity`): the `CertProvider` port and `LocalIdentity`, whose
  `Debug` redacts key and certificate bytes so material never leaks into logs.
  Real certificate handling is deferred to Transport's QUIC adapter, which feeds
  this material into rustls and enforces TOFU via a custom certificate verifier.

### What `omni-session` provides

- **Sessions and roles** (`session`): `Role` (reversible Controller/Target),
  `Session`, `ActiveTarget` (`Local` vs `Remote(peer)`), and `SessionManager` —
  establishes and closes sessions, reverses roles, and switches the active target
  in response to Topology `Crossing`s (crossing onto a peer routes input there;
  crossing back home routes it local). `yield_control` brings input home because
  a peer has taken over, and `follow_peer` sends input to a peer that has taken
  the cursor back onto its own screen — the two halves of a handover, which is
  what keeps control in one place and every keyboard aimed at the machine
  holding the cursor. Target-change events are deduplicated.
- **Events** (`events`): the `SessionEvents` port (lifecycle, role, and
  active-target changes) plus a recording adapter for tests.

### What `omni-input` provides

- **Ports** (`port`): `InputSource` (non-blocking `poll` to capture) and
  `InputSink` (`inject` to synthesize), each with an associated error type so
  real OS adapters can report failures.
- **Scrolling** (`scroll`): `ScrollAccumulator` turns the wire's milli-lines
  into the whole lines, notches, or wheel clicks an OS accepts, holding back
  what falls short of one so slow scrolling adds up instead of vanishing;
  `millilines_from_lines` converts a platform's fractional line count the other
  way, never rounding a real movement away to nothing. Pure and shared by all
  three sinks, so the arithmetic is tested on every platform.
- **In-memory adapters** (`memory`): `QueuedSource` replays a scripted sequence
  of events; `RecordingSink` records what is injected. Together they stand in for
  hardware and exercise the capture→send and receive→inject pipelines.
- **Suppression**: `InputSource::set_suppressed` — while input is routed to a
  remote machine the source still reports events but withholds them from the
  local OS, so input never acts on two machines at once.
- **macOS adapters** (`macos`): `MacosSource` captures through a CGEvent tap on
  a dedicated run-loop thread (suppression drops the event before the OS acts;
  the tap re-enables itself if the OS disables it) and `MacosSink` injects with
  `CGEventPost`, stamping its events so the tap never re-captures them. Each
  move also warps the cursor (`CGWarpMouseCursorPosition`) to the same spot,
  because macOS otherwise blanks the cursor for purely synthesized motion;
  that keeps the remote-driven cursor visible. Needs the Accessibility
  permission — never root. A `kVK ↔ HID` keymap covers the full ANSI layout.
- **Linux adapters** (`linux`): `LinuxSource` reads keyboards and mice from
  `/dev/input` (one thread per device; suppression = `EVIOCGRAB`), `LinuxSink`
  injects through a uinput virtual device that the capture side knows to skip.
  Needs only `input`-group membership — never root. A `KEY_* ↔ HID` keymap
  mirrors the macOS one. (Compile-checked against a Linux target; needs live
  hardware to exercise.)
- **Windows adapters** (`windows`): `WindowsSource` captures through
  `WH_KEYBOARD_LL` / `WH_MOUSE_LL` low-level hooks on a dedicated message-loop
  thread (suppression swallows the event before the OS acts; mouse motion is
  turned into relative deltas, with the cursor parked at screen centre while
  controlling a remote so deltas never stall at an edge), and `WindowsSink`
  injects with `SendInput`, stamping events so the hooks never re-capture them.
  Needs no elevation for ordinary windows — only to drive administrator windows
  (User Interface Privilege Isolation). A `VK_* ↔ HID` keymap mirrors the other
  platforms. The process declares **per-monitor DPI awareness (v2)** at startup
  (`platform::prepare_process`), so on a scaled high-DPI display the hook
  deltas, `SetCursorPos`/`GetCursorPos` parking, and `GetSystemMetrics` screen
  size all share one physical-pixel space — without it the mismatch biased
  every delta and pinned a remote-controlled cursor to a corner. (Built and
  unit-tested on Windows; needs live hardware to exercise the full pipeline.)

### What `omni-transport` provides

- **Secure channel** (`channel`): the `SecureChannel` port — an established,
  per-peer connection that sends and receives datagram payloads. QUIC provides
  the cryptography, so the port deals only in already-protected bytes. A
  `LoopbackChannel` pair stands in for a real connection in tests.
- **Message framing** (`transport`): `Transport` encodes a Protocol `Message` and
  sends it as one (unreliable) datagram, and decodes received datagrams back,
  surfacing channel vs codec failures via `TransportError`.
- **QUIC adapter** (`quic` + `tls`, with the `policy` port): `QuicEndpoint` owns
  one UDP socket that both dials and listens (roles are dynamic), `QuicConnection`
  is the production `SecureChannel` (unreliable datagrams for input), and
  `ControlStream` frames signalling over one reliable bidirectional stream.
  Mutual TLS 1.3 is mandatory; custom rustls certificate verifiers enforce the
  `HandshakePolicy` port (implemented by the Runtime over Security's trust store),
  so an unauthorized or fingerprint-changed peer never completes the handshake.
  `BulkSender`/`BulkReceiver` carry clipboard-sized payloads on a QUIC stream of
  their own, one per transfer, so a large one cannot delay the heartbeats that
  keep a session alive — nor the next transfer. Signalling and bulk each have
  their own frame limit. Exercised by live two-endpoint tests over localhost.

### What `omni-runtime` provides

- **Paths & config** (`config`): everything lives in one directory named `omni`
  (`~/.config/omni` on Linux, `~/Library/Application Support/omni` on macOS,
  `%APPDATA%\omni` on Windows — the platform config directory, overridable with
  `OMNI_CONFIG_DIR`): `config.json` (UDP port, default 4733; optional
  screen-size override; per-host edge placements; per-host modifier swaps),
  certificate + key, `trust.json`, the IPC socket (or named pipe on Windows),
  and the log. Both native clients reproduce this derivation, so the names here
  are part of the contract rather than an implementation detail.
- **Identity** (`identity`): generates a self-signed certificate on first run
  (via `rcgen`), persists it with `0600` permissions, reloads it afterwards —
  so the machine's fingerprint is stable across restarts.
- **Trust store** (`trust`): the persistent TOFU store behind Security's
  policy and Transport's `HandshakePolicy` — thread-safe, JSON-backed, used by
  both the QUIC verifiers (reject unknown/changed certs at the handshake) and
  the accept/reject flow.
- **Rate limiter** (`ratelimit`): a token bucket capping injected events per
  session (default 2 000 events/s, burst 4 000) so a misbehaving peer cannot
  flood the local OS.
- **Identity, keys & secrets** (`identity`, `secure`): the self-signed
  certificate is generated once and reused; the private key is written
  owner-only — mode `0600` on Unix, an inheritance-stripped owner-only ACL on
  Windows.
- **IPC** (`ipc`, `ipc_transport`): the JSON-lines request/response protocol
  the CLI speaks (status, connect, accept, peers, layout, ...), over one
  transport abstraction — a Unix-domain socket on macOS/Linux, a per-state-dir
  named pipe (local clients only, first-instance claimed) on Windows. Status
  reports whether input capture is live, so a target-only daemon is visible.
- **Heartbeats**: each session sends a `Heartbeat` every 2 s and tears itself
  down if nothing arrives from the peer within 8 s, so a silently-dropped peer
  is noticed without waiting for QUIC's own idle timeout.
- **Layout** (`omni layout`): per-host edge placements, applied live to an open
  session and persisted to `config.json` for next time, so machines can be
  arranged on any edge instead of a fixed left/right chain.
- **Doctor** (`doctor`): environment checks behind `omni doctor` — the
  platform's input-permission diagnostics (Accessibility on macOS; evdev and
  uinput access on Linux) plus screen-size and state-directory checks.
- **The daemon** (`daemon`): the composition root. A capture thread polls the
  OS input source and advances the virtual cursor through Topology; an edge
  crossing flips Session's active target, suppresses local input, and tells the
  peers: a cursor crossing *onto* a peer warps that peer's cursor to the entry
  point, and a cursor crossing back home tells every peer where it landed. The
  same two messages arriving the other way move this machine's input with the
  cursor — giving up whatever it was driving when a peer's cursor lands here,
  and following a peer that has taken the cursor back. Control belongs to
  whichever user moved last, and every keyboard types on the machine holding
  the cursor. While a remote peer is active, pointer
  motion travels as the cursor's **absolute position on the peer's screen**
  (mapped through the virtual desktop using both machines' sizes), not raw
  relative deltas — so the two cursors cannot drift apart and control stays
  correct across different resolutions. Input events ride unreliable QUIC
  datagrams to the active peer; signalling (connect/accept/disconnect/warp)
  rides the reliable control stream. Each peer connection runs in its own
  task; incoming requests from unknown peers wait (up to 120 s) for
  `omni accept`, trusted peers are auto-accepted. The Unix socket (mode 0600)
  serves the CLI.

### What `omni-cli` provides

The complete `omni` surface from the README: `start` (spawns the daemon
detached via a hidden `daemon` subcommand and waits for the socket), `stop`,
`status` (fingerprint, port, sessions with the input-here marker, pending
requests), `connect` / `disconnect <host>`, `accept` / `reject
<host|fingerprint>`, `peers` / `peers remove <host>`, `layout` (list or set
where each peer sits), `update` (self-update to the latest GitHub release —
stops the daemon, swaps the binary, restarts it), `doctor` (prints every permission/environment
check and the daemon's own capture state, non-zero exit when something is
unmet), and `uninstall` (stops the daemon, removes the config dir, deletes
the binary). Each command is one JSON line to the daemon and one line back;
errors land on stderr with a non-zero exit.

## Tooling & dependencies

- Rust workspace, edition 2024, resolver 3.
- Third-party deps pinned once in `[workspace.dependencies]`: `serde`, `postcard`,
  the network/crypto stack (`quinn`, `rustls` + `ring`, `rcgen`, `sha2`, `tokio`,
  `bytes`), the daemon/CLI layer (`clap`, `serde_json`, `dirs`, `rand`,
  `tracing` + `tracing-subscriber`), and the per-OS input/IPC backends
  (`core-graphics`/`core-foundation` on macOS, `evdev` on Linux, `windows-sys`
  on Windows).
- Quality gate per change: `cargo fmt --all`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test` — run locally and in CI
  (GitHub Actions) across Linux, macOS, and Windows.
- Building on Windows needs a linker: native MSVC (the standard runner toolchain
  CI uses), or the self-contained `x86_64-pc-windows-gnu` toolchain plus a
  MinGW-w64 C toolchain for the `ring` crypto backend on a box without MSVC.

## Workflow

Gitflow, local only (no remote yet):

- `master` — production.
- `develop` — integration. Protocol and the workspace scaffold are merged here.
- `feature/<name>` — one per unit of work (typically one crate), branched off
  `develop` and merged back with `--no-ff`.

Commits follow Conventional Commits. New behaviour is written test-first (TDD)
against ports using in-memory adapters.

## Not yet done

Everything below is known, deliberate, and ordered roughly by importance:

- **Control in both directions has not been run on two live machines yet.** The
  hook fix and both halves of the handover are in place and the arithmetic and
  the session rules are unit-tested, but the failures they address only appear on
  real hardware: what proves it is pushing a Mac's cursor onto a PC and being
  able to click and type there, then pushing the PC's cursor back and finding
  that the PC's keyboard now types on the Mac. The same goes for the Windows
  scrolling fixes — in particular that a precision touchpad scrolls at all, and
  that one wheel notch moves the same distance on both machines. Both machines
  must run the same version: an older peer cannot answer a message it does not
  know.
- **Only the machine holding the cursor should track it.** Every machine keeps
  its own copy of where the cursor is, and a machine that is not holding it hears
  the truth only at a handover. Move one machine's mouse around its own screen
  for a while and then reach for the other machine's mouse, and the pointer jumps
  once to where that machine last knew it before carrying on. The fix is to make
  the holder the single authority — the others send plain movement and let the
  holder decide where the cursor ends up and when it crosses — which also removes
  the second copy of the crossing arithmetic.
- **Display changes need a restart.** The desktop's geometry is read once at
  startup and the size is announced to a peer only when the session is
  established. Docking, unplugging a monitor, or changing the scaling therefore
  leaves the virtual desktop describing a screen that no longer exists until the
  daemon is restarted. Fixing it means re-reading the bounds when the OS says
  they changed (`WM_DISPLAYCHANGE`, `NSApplication.didChangeScreenParameters`)
  and telling live peers, which needs one additive control message.
- **Caps Lock cannot be injected into macOS.** Capture works in both directions,
  but `CGEventPost` cannot move the Caps Lock latch — only IOKit HID can — so a
  remote machine's Caps Lock has no effect on a Mac being controlled.
- **The GUIs do not expose `doctor` or the modifier swap.** `omni modifiers` and
  the permission checks are CLI-only. Both are worth surfacing, in both clients
  at once so the layout parity holds.
- **Automatic reconnection.** `omni connect` between two real machines
  (Windows ↔ macOS) is validated and works, including clipboard. What is missing
  is recovery from a *dropped* link: when the connection fails (network blip,
  idle timeout, peer restart) the session is torn down and the user must
  reconnect by hand. The intended behaviour is for the dialing side (Controller)
  to retry with exponential backoff until the peer returns or the user runs
  `omni disconnect`. This is the active next piece of work.
- **Linux live run.** macOS ↔ Linux ↔ Windows triangles have not been run; the
  Linux adapters build and unit-test but are not exercised on real hardware
  (see also "Linux clipboard sharing").
- **`omni start` is plain detach.** No launchd/systemd/Windows-service files
  yet, so the daemon does not survive a reboot or restart on crash.
- **Linux and Windows are not hardware-tested.** The evdev/uinput adapters and
  the Win32 hook/`SendInput` adapters build and unit-test, but have not run
  against real devices on a live desktop.
- **Linux clipboard sharing.** Text and image sync are validated between two
  real machines on Windows and macOS. Linux is not supported yet: the `arboard`
  adapter is built with `default-features = false`, so its X11/Wayland clipboard
  backends are not enabled or exercised — wiring and verifying Linux clipboard
  (text and image) is the remaining clipboard work.

## Planned: native GUI clients

A native **macOS** (Swift/SwiftUI) and **Windows** (C#/WinUI 3) GUI are planned,
each a thin client of the daemon's existing local IPC — no changes to the core
crates. The design, the binding constraints (GUI-only exception to Rust-only,
discovery/pairing never bypass accept+TOFU, daemon owns all state), the feature
inventory, and the phased plan live in
[`NATIVE_INTEGRATIONS.md`](NATIVE_INTEGRATIONS.md).

**Phase 1 is done:** the IPC has a live **event channel** (`Request::Subscribe`
streams `Event::Status` snapshots on any change, coalesced by a `watch` channel —
no polling) and a **version handshake** (`Request::Hello` →
`Response::Hello { protocol_version, daemon_version }`).

**The Windows app (Phase 4) has a first implementation** in
`clients/omni-windows/` (C#/WinUI 3), ahead of the planned order because the owner
is taking the macOS app. It is a thin IPC client (`Omni.Ipc`), MVVM view models
(`Omni.App.Core`), and a Fluent window (`Omni.App`), with 27 unit tests and a
Windows CI job. It already shows live status and drives accept/reject, connect,
peers, layout, and clipboard. Still to do on the client: a tray entry for the
accept prompt when the window is closed, and (after the backend lands) discovery
and pairing.

Next on the **daemon** side: the mDNS + pairing-code connection backend (Phase 2,
Rust), which the Windows app and the **macOS** app (done natively by the owner)
both consume. Linux stays CLI-only.

## Open decisions

- **Secure channel: decided — QUIC** (TLS 1.3 over UDP) via `quinn` + `rustls`,
  replacing the originally planned DTLS 1.3. Rationale: no production-ready
  *pure-Rust* DTLS 1.3 + mTLS exists (rustls has no DTLS; `rusty-dtls` is PSK-only;
  the webrtc `dtls` crate is 1.2 only; wolfSSL/OpenSSL mean a C dependency). QUIC
  keeps every required property (UDP-only, mutual cert auth, TOFU, anti-replay,
  modern crypto), carries input over unreliable datagrams (RFC 9221), and has the
  most mature pure-Rust implementation.
- **Local IPC: decided — Unix domain socket** in the config directory (mode
  0600) on macOS/Linux, and a **named pipe** (local clients only, first
  instance claimed) on Windows, behind one transport abstraction. JSON lines,
  one request, one response. Simple, debuggable, and access-controlled by the
  platform's own mechanism.
- Wire-format versioning: whether to prepend a protocol version byte in Transport
  framing (deliberately left out of the Protocol codec for now). The cost showed
  up when scrolling changed unit: two machines must run matching versions, and a
  mismatched pair scrolls wrongly with nothing to warn them. The IPC has a
  version handshake; the peer-to-peer wire does not.
