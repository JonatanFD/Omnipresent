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
  /** The daemon's version, which is not this app's when it did not start it. */
  daemonVersion: string;
  appVersion: string;
  /** This machine's address on the local network, for a peer to dial. */
  localAddress: string | null;
  /** Hosts a connection is being requested from, so the UI can show progress. */
  connecting: string[];
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
  /** Asks the OS for the input permission, showing its own prompt. */
  requestPermission: () => Promise<void>;
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
  daemonVersion: "",
  appVersion: "",
  localAddress: null,
  connecting: [],
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
  /** Sends a command and waits for the daemon to push a fresh snapshot through
   *  the subscription. The snapshot is the source of truth: it now carries peers,
   *  placements, and modifier swaps, so there is no second round trip to fetch
   *  them. On failure the local state is left untouched, so the UI keeps showing
   *  what the daemon last confirmed. */
  const run = async (command: () => Promise<void>) => {
    try {
      await command();
      set({ error: null });
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  };

  return {
    ...initialState,

    init: async () => {
      // The snapshot the daemon pushes carries everything the UI shows: sessions,
      // pending, peers, placements, and modifier swaps. Replacing state from it
      // means the UI never runs ahead of the daemon, and no command needs a
      // follow-up fetch to stay consistent.
      const unlistenStatus = await listen<StatusInfo>(STATUS_EVENT, (event) => {
        set({ status: event.payload, connection: "connected", error: null });
      });

      const unlistenDropped = await listen(DISCONNECTED_EVENT, () => {
        // The subscription drops every second while the daemon is absent, which
        // is noise — not a state change the user needs to see. Only report a
        // drop when we actually had something to lose: a connected or
        // incompatible session. A `connecting` state (e.g. `startDaemon` in
        // flight) is left alone, so the user does not see a flicker to
        // "disconnected" while the daemon is still coming up.
        const current = get().connection;
        if (current === "connected" || current === "incompatible") {
          set({ connection: "disconnected" });
        }
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
      try {
        set({ localAddress: await installation.localAddress() });
      } catch {
        // A machine with no network still runs; the pane just shows nothing.
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
      } catch (error) {
        set({ connection: "disconnected", status: null, error: errorMessage(error) });
      }
    },

    /** Dialling a peer waits on a human at the other end, so it can take a
     *  while and has to look like it is doing something. The host is held in
     *  `connecting` until the daemon reports a session with it, or it fails. */
    connect: async (host) => {
      set((s) => ({ connecting: [...s.connecting, host], error: null }));
      try {
        await daemon.connect(host);
        // A refresh here keeps the snapshot honest: the connection request
        // may be answered on the other end within this call, and the pushed
        // snapshot arrives whenever it arrives. Re-reading once means the
        // window is not left guessing between the click and the push.
        await get().refresh();
      } catch (error) {
        set({ error: errorMessage(error) });
      } finally {
        set((s) => ({ connecting: s.connecting.filter((h) => h !== host) }));
      }
    },

    requestPermission: async () => {
      try {
        await daemon.requestPermission();
        // The answer lands in the OS, not here, so re-run the checks: they are
        // what will show it once the daemon has been restarted.
        await get().runDoctor();
      } catch (error) {
        set({ error: errorMessage(error) });
      }
    },
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
 *
 * Peers, placements, and modifier swaps now live inside the status snapshot, so the
 * hooks read them there with a stable empty-list fallback for the moment before the
 * first snapshot arrives.
 */

const EMPTY_SESSIONS: SessionInfo[] = [];
const EMPTY_PENDING: PendingInfo[] = [];
const EMPTY_PEERS: PeerInfo[] = [];
const EMPTY_PLACEMENTS: LayoutInfo[] = [];
const EMPTY_SWAPS: ModifierInfo[] = [];

export const useConnection = (): ConnectionState => useDaemonStore((s) => s.connection);
export const useStatus = (): StatusInfo | null => useDaemonStore((s) => s.status);
export const useDaemonError = (): string | null => useDaemonStore((s) => s.error);
export const useDaemonVersion = (): string => useDaemonStore((s) => s.daemonVersion);
export const useAppVersion = (): string => useDaemonStore((s) => s.appVersion);
export const useLocalAddress = (): string | null => useDaemonStore((s) => s.localAddress);
export const useConnecting = (): string[] => useDaemonStore((s) => s.connecting);
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

export const usePeers = (): PeerInfo[] =>
  useDaemonStore((s) => s.status?.peers ?? EMPTY_PEERS);

export const usePlacements = (): LayoutInfo[] =>
  useDaemonStore((s) => s.status?.placements ?? EMPTY_PLACEMENTS);

export const useSwaps = (): ModifierInfo[] =>
  useDaemonStore((s) => s.status?.modifier_swaps ?? EMPTY_SWAPS);

export const useClipboardSharing = (): boolean =>
  useDaemonStore((s) => s.status?.clipboard_sharing ?? false);

export const useIsConnected = (): boolean => useDaemonStore((s) => s.connection === "connected");
