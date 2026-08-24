import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import {
  DISCONNECTED_EVENT,
  EXITED_EVENT,
  STATUS_EVENT,
  daemon,
  errorMessage,
  installation,
  type CheckInfo,
  type DaemonExit,
  type DaemonLog,
  type Edge,
  type LayoutInfo,
  type ModifierInfo,
  type ModifierSwap,
  type PeerInfo,
  type PendingInfo,
  type SessionInfo,
  type StatusInfo,
} from "@/lib/ipc";

/**
 * The daemon's state as this window currently understands it.
 *
 * The daemon owns the real state. This store holds the latest snapshot it pushed,
 * so an update is a whole-snapshot replacement rather than a local edit — the UI
 * never runs ahead of the daemon. Every action sends a command and then re-reads
 * what the daemon says, instead of guessing at the result.
 */

export type ConnectionState =
  | "disconnected"
  | "connecting"
  | "connected"
  /** The daemon speaks a protocol newer than this build understands. */
  | "incompatible";

interface DaemonState {
  connection: ConnectionState;
  status: StatusInfo | null;
  peers: PeerInfo[];
  placements: LayoutInfo[];
  swaps: ModifierInfo[];
  /** The daemon's version, which is not this app's when it did not start it. */
  daemonVersion: string;
  appVersion: string;
  /** Whether the daemon being talked to is the one inside this app. */
  embedded: boolean;
  /** The last `doctor` run, or null if it has not been run yet. */
  checks: CheckInfo[] | null;
  /** The tail of the daemon's log, or null if it has not been read yet. */
  log: DaemonLog | null;
  /** The last failure, for showing the user. Never holds key material. */
  error: string | null;
}

interface DaemonActions {
  /** Subscribes to daemon pushes and loads the first snapshot. Returns a cleanup. */
  init: () => Promise<() => void>;
  refresh: () => Promise<void>;
  connect: (host: string) => Promise<void>;
  disconnect: (host: string) => Promise<void>;
  accept: (selector: string) => Promise<void>;
  reject: (selector: string) => Promise<void>;
  removePeer: (selector: string) => Promise<void>;
  setLayout: (host: string, edge: Edge) => Promise<void>;
  setModifiers: (host: string, swap: ModifierSwap) => Promise<void>;
  setClipboard: (enabled: boolean) => Promise<void>;
  startDaemon: () => Promise<void>;
  stopDaemon: () => Promise<void>;
  runDoctor: () => Promise<void>;
  /** Reads the tail of the daemon's log. */
  readLog: () => Promise<void>;
  clearError: () => void;
}

const initialState: DaemonState = {
  connection: "connecting",
  status: null,
  peers: [],
  placements: [],
  swaps: [],
  daemonVersion: "",
  appVersion: "",
  embedded: false,
  checks: null,
  log: null,
  error: null,
};

/**
 * What an embedded daemon's exit means for the user.
 *
 * Standing down is the normal outcome when a daemon is already running — from
 * `omni start`, or from another copy of this app. The window works exactly the
 * same against that one, so it is not something to report. A real failure is.
 *
 * Exported for its own sake: it is the one piece of judgement in this file, and
 * getting it wrong shows a scary error during an ordinary launch.
 */
export function exitMessage(exit: DaemonExit): string | null {
  if (exit.stood_down || !exit.reason) return null;
  return `The daemon stopped: ${exit.reason}`;
}

/** The one-line answer to "is everything ok". */
export type Health =
  /** The checks have not run yet. */
  | { state: "unknown"; failing: 0 }
  | { state: "ok"; failing: 0 }
  | { state: "problems"; failing: number };

/**
 * Reduces the checks to a verdict.
 *
 * "Not run yet" is deliberately not the same as "fine": showing a reassuring
 * green before anything has been looked at is the one wrong answer this can
 * give, because it is the answer nobody would question.
 */
export function health(checks: CheckInfo[] | null): Health {
  if (checks === null) return { state: "unknown", failing: 0 };

  const failing = checks.filter((check) => !check.ok).length;
  return failing === 0 ? { state: "ok", failing: 0 } : { state: "problems", failing };
}

export const useDaemonStore = create<DaemonState & DaemonActions>()((set, get) => {
  /** Peers, placements and modifier swaps do not travel in the status snapshot,
   *  so they are read separately whenever the daemon says something changed. */
  const loadPeerSettings = async () => {
    try {
      const [peers, placements, swaps] = await Promise.all([
        daemon.peers(),
        daemon.layout(),
        daemon.modifiers(),
      ]);
      set({ peers, placements, swaps });
    } catch {
      // A daemon that went away is already reported by the status path; there is
      // nothing extra to tell the user here.
    }
  };

  /** Sends a command, then re-reads the daemon. On failure the local state is
   *  left untouched, so the UI keeps showing what the daemon last confirmed. */
  const run = async (command: () => Promise<void>) => {
    try {
      await command();
      set({ error: null });
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  };

  return {
    ...initialState,

    init: async () => {
      const unlistenStatus = await listen<StatusInfo>(STATUS_EVENT, (event) => {
        set({ status: event.payload, connection: "connected", error: null });
        void loadPeerSettings();
      });

      const unlistenDropped = await listen(DISCONNECTED_EVENT, () => {
        set({ connection: "disconnected" });
      });

      // The embedded daemon ending is not the same as losing touch with one: it
      // may have stood down for a daemon that was already there, in which case
      // the window carries on against that one and says nothing.
      const unlistenExited = await listen<DaemonExit>(EXITED_EVENT, (event) => {
        const message = exitMessage(event.payload);
        set({ embedded: false, ...(message ? { error: message } : {}) });
        void get().refresh();
      });

      // The app's own version never changes while it runs, so it is read once.
      try {
        set({ appVersion: await installation.version() });
      } catch {
        // Leaves the version blank rather than blocking the whole window.
      }
      // Run once at startup so the sidebar can flag a problem without the user
      // having to go looking for one. They are local permission queries and a
      // single IPC round trip, so this costs nothing on an idle app.
      void get().runDoctor();
      await get().refresh();

      return () => {
        unlistenStatus();
        unlistenDropped();
        unlistenExited();
      };
    },

    refresh: async () => {
      try {
        const version = await daemon.hello();
        if (!version.compatible) {
          set({
            connection: "incompatible",
            daemonVersion: version.daemon_version,
            error: `The daemon speaks protocol v${version.protocol_version}, which is newer than this app understands. Update Omnipresent.`,
          });
          return;
        }

        const [status, embedded] = await Promise.all([daemon.status(), daemon.embedded()]);
        set({
          status,
          embedded,
          connection: "connected",
          daemonVersion: version.daemon_version,
          error: null,
        });
        await loadPeerSettings();
      } catch (error) {
        set({ connection: "disconnected", status: null, error: errorMessage(error) });
      }
    },

    connect: (host) => run(() => daemon.connect(host)),
    disconnect: (host) => run(() => daemon.disconnect(host)),
    accept: (selector) => run(() => daemon.accept(selector)),
    reject: (selector) => run(() => daemon.reject(selector)),
    removePeer: (selector) => run(() => daemon.removePeer(selector)),
    setLayout: (host, edge) => run(() => daemon.setLayout(host, edge)),
    setModifiers: (host, swap) => run(() => daemon.setModifiers(host, swap)),
    setClipboard: (enabled) => run(() => daemon.setClipboard(enabled)),

    startDaemon: async () => {
      set({ connection: "connecting", error: null });

      let startFailure: string | null = null;
      try {
        await daemon.start();
      } catch (error) {
        startFailure = errorMessage(error);
      }

      // The daemon binds its socket on a thread of its own, so it is not
      // necessarily serving by the time `start` returns. The subscription's
      // reconnect loop picks it up; this is just the first look.
      await get().refresh();

      // `refresh` reports not being able to reach a daemon, which is only the
      // symptom. When starting one is what failed, that reason is the cause and
      // is the more useful thing to leave on screen.
      if (startFailure && get().connection !== "connected") {
        set({ error: startFailure });
      }

      // One of the checks is the daemon's own capture state, which has just
      // changed. A stale verdict here is worse than none.
      void get().runDoctor();
    },

    stopDaemon: async () => {
      try {
        await daemon.stop();
        set({
          connection: "disconnected",
          status: null,
          peers: [],
          placements: [],
          swaps: [],
          error: null,
        });
        void get().runDoctor();
      } catch (error) {
        set({ error: errorMessage(error) });
      }
    },

    runDoctor: async () => {
      try {
        // Deliberately does not clear `error` on success. This runs in the
        // background — at startup, and after the daemon starts or stops — and
        // there is one error slot: clearing it here would wipe the reason the
        // daemon failed to start moments after the user was shown it.
        set({ checks: await daemon.doctor() });
      } catch (error) {
        set({ error: errorMessage(error) });
      }
    },

    readLog: async () => {
      try {
        // Deliberately does not clear `error`: the log is usually read *because*
        // something failed, and wiping the reason would defeat the point.
        set({ log: await daemon.log() });
      } catch (error) {
        set({ error: errorMessage(error) });
      }
    },

    clearError: () => set({ error: null }),
  };
});

/*
 * Selector hooks. Subscribing to one slice keeps a component from re-rendering on
 * every unrelated snapshot — which matters, because the daemon pushes a fresh full
 * snapshot on every change.
 */

const EMPTY_SESSIONS: SessionInfo[] = [];
const EMPTY_PENDING: PendingInfo[] = [];

export const useConnection = (): ConnectionState => useDaemonStore((s) => s.connection);
export const useStatus = (): StatusInfo | null => useDaemonStore((s) => s.status);
export const usePeers = (): PeerInfo[] => useDaemonStore((s) => s.peers);
export const usePlacements = (): LayoutInfo[] => useDaemonStore((s) => s.placements);
export const useSwaps = (): ModifierInfo[] => useDaemonStore((s) => s.swaps);
export const useDaemonError = (): string | null => useDaemonStore((s) => s.error);
export const useDaemonVersion = (): string => useDaemonStore((s) => s.daemonVersion);
export const useAppVersion = (): string => useDaemonStore((s) => s.appVersion);
export const useEmbedded = (): boolean => useDaemonStore((s) => s.embedded);
export const useChecks = (): CheckInfo[] | null => useDaemonStore((s) => s.checks);

/** How many checks are failing — what the sidebar badges. */
export const useFailingChecks = (): number =>
  useDaemonStore((s) => health(s.checks).failing);
export const useLog = (): DaemonLog | null => useDaemonStore((s) => s.log);

export const useSessions = (): SessionInfo[] =>
  useDaemonStore((s) => s.status?.sessions ?? EMPTY_SESSIONS);

export const usePending = (): PendingInfo[] =>
  useDaemonStore((s) => s.status?.pending ?? EMPTY_PENDING);

export const useClipboardSharing = (): boolean =>
  useDaemonStore((s) => s.status?.clipboard_sharing ?? false);

export const useIsConnected = (): boolean => useDaemonStore((s) => s.connection === "connected");
