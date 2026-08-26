/**
 * TypeScript mirror of the daemon's local IPC protocol, plus the Tauri commands
 * that carry it.
 *
 * These types must stay in step with `crates/omni-runtime/src/ipc.rs`, which is
 * the single source of truth. Field names are snake_case because that is what
 * serde puts on the wire — renaming them here would only hide the mismatch.
 *
 * The window renders what the daemon reports and sends commands back. It never
 * re-implements a decision the daemon owns: trust, layout maths, fingerprinting.
 */

import { invoke } from "@tauri-apps/api/core";

/** Which side of a session this machine is on. */
export type Role = "controller" | "target";

/** The edge of the virtual desktop a peer sits past. */
export type Edge = "left" | "right" | "top" | "bottom";

export const EDGES: Edge[] = ["left", "right", "top", "bottom"];

/** How a peer's modifier keys are relabelled. */
export type ModifierSwap = "none" | "meta-control";

/** An active session with a peer. */
export interface SessionInfo {
  host: string;
  fingerprint: string;
  role: Role;
  /** Whether input is currently routed to this peer. */
  active: boolean;
}

/** An incoming request waiting on a human accept/reject decision. */
export interface PendingInfo {
  host: string;
  fingerprint: string;
}

/** A peer the daemon knows about. */
export interface PeerInfo {
  host: string | null;
  fingerprint: string;
  connected: boolean;
}

/** Where one peer sits in the virtual desktop. */
export interface LayoutInfo {
  host: string;
  edge: Edge;
  /** True from a live session, false when only saved for the next connection. */
  connected: boolean;
}

/** How one peer's modifier keys are relabelled. */
export interface ModifierInfo {
  host: string;
  swap: ModifierSwap;
  /** True from a live session, false when only saved for the next connection. */
  connected: boolean;
}

/** The full snapshot the daemon pushes on every state change. */
export interface StatusInfo {
  /** This machine's certificate fingerprint — what peers pin on first accept. */
  fingerprint: string;
  port: number;
  /** False means target-only: input capture is not running. */
  capturing: boolean;
  clipboard_sharing: boolean;
  sessions: SessionInfo[];
  pending: PendingInfo[];
  /** Known peers (the trusted list, plus which are currently connected).
   *  Included since protocol v1 as an additive field: a daemon built before it
   *  omits the key, and an old app sees an empty list. */
  peers: PeerInfo[];
  /** Where each peer sits in the virtual desktop. Additive, same as `peers`. */
  placements: LayoutInfo[];
  /** How each peer's modifier keys are relabelled. Additive, same as `peers`. */
  modifier_swaps: ModifierInfo[];
}

/** The daemon's version handshake. */
export interface DaemonVersion {
  protocol_version: number;
  daemon_version: string;
  /** False when the daemon is newer than this app understands. */
  compatible: boolean;
}

/** One environment requirement and whether it is met. */
export interface CheckInfo {
  name: string;
  ok: boolean;
  /** What was found — and, when not ok, how to fix it. */
  detail: string;
}

/** The daemon's own log, which is the only place a startup failure explains itself. */
export interface DaemonLog {
  path: string;
  /** Most recent lines, oldest first. Empty when the daemon has never run. */
  lines: string[];
}

/** Why the embedded daemon stopped. */
export interface DaemonExit {
  /** The failure that ended it, or null when it was asked to stop. */
  reason: string | null;
  /** True when another daemon owns the socket, so ours was never needed. */
  stood_down: boolean;
}

/** Events the Rust side pushes. Must match the constants in `src-tauri/src/`. */
export const STATUS_EVENT = "daemon://status";
export const DISCONNECTED_EVENT = "daemon://disconnected";
export const EXITED_EVENT = "daemon://exited";

/**
 * The daemon commands, one per Tauri command. Each rejects with the daemon's own
 * error message, which is what the UI shows the user.
 */
export const daemon = {
  status: () => invoke<StatusInfo>("daemon_status"),
  hello: () => invoke<DaemonVersion>("daemon_hello"),
  stop: () => invoke<void>("daemon_stop"),
  /** Starts the daemon inside this app. Only meaningful after a stop. */
  start: () => invoke<void>("daemon_start"),
  /** Whether the daemon being talked to is the one inside this app. */
  embedded: () => invoke<boolean>("daemon_embedded"),
  /** The permission and environment checks behind `omni doctor`. */
  doctor: () => invoke<CheckInfo[]>("daemon_doctor"),
  /** The tail of the daemon's log. */
  log: () => invoke<DaemonLog>("daemon_log"),
  /** Asks the OS for the input permission, showing its own prompt. */
  requestPermission: () => invoke<boolean>("request_input_permission"),

  connect: (host: string) => invoke<void>("peer_connect", { host }),
  disconnect: (host: string) => invoke<void>("peer_disconnect", { host }),
  accept: (selector: string) => invoke<void>("peer_accept", { selector }),
  reject: (selector: string) => invoke<void>("peer_reject", { selector }),

  peers: () => invoke<PeerInfo[]>("peer_list"),
  removePeer: (selector: string) => invoke<void>("peer_remove", { selector }),

  layout: () => invoke<LayoutInfo[]>("layout_list"),
  setLayout: (host: string, edge: Edge) => invoke<void>("layout_set", { host, edge }),

  modifiers: () => invoke<ModifierInfo[]>("modifiers_list"),
  setModifiers: (host: string, swap: ModifierSwap) =>
    invoke<void>("modifiers_set", { host, swap }),

  setClipboard: (enabled: boolean) => invoke<void>("clipboard_set", { enabled }),
};

/** This installation. Everything else lives inside the app itself. */
export const installation = {
  version: () => invoke<string>("app_version"),
  /** This machine's address on the local network, for a peer to dial. */
  localAddress: () => invoke<string | null>("local_address"),
};

/** Turns whatever `invoke` rejected with into something showable. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong talking to the daemon.";
}
