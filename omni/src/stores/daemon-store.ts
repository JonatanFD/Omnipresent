import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import {
  DISCONNECTED_EVENT,
  STATUS_EVENT,
  daemon,
  errorMessage,
  type Edge,
  type LayoutInfo,
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
  daemonVersion: string;
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
  setClipboard: (enabled: boolean) => Promise<void>;
  stopDaemon: () => Promise<void>;
  clearError: () => void;
}

const initialState: DaemonState = {
  connection: "connecting",
  status: null,
  peers: [],
  placements: [],
  daemonVersion: "",
  error: null,
};

export const useDaemonStore = create<DaemonState & DaemonActions>()((set, get) => {
  /** Peers and placements do not travel in the status snapshot, so they are read
   *  separately whenever the daemon says something changed. */
  const loadPeersAndLayout = async () => {
    try {
      const [peers, placements] = await Promise.all([daemon.peers(), daemon.layout()]);
      set({ peers, placements });
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
        void loadPeersAndLayout();
      });

      const unlistenDropped = await listen(DISCONNECTED_EVENT, () => {
        set({ connection: "disconnected" });
      });

      await get().refresh();

      return () => {
        unlistenStatus();
        unlistenDropped();
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

        const status = await daemon.status();
        set({
          status,
          connection: "connected",
          daemonVersion: version.daemon_version,
          error: null,
        });
        await loadPeersAndLayout();
      } catch (error) {
        set({ connection: "disconnected", error: errorMessage(error) });
      }
    },

    connect: (host) => run(() => daemon.connect(host)),
    disconnect: (host) => run(() => daemon.disconnect(host)),
    accept: (selector) => run(() => daemon.accept(selector)),
    reject: (selector) => run(() => daemon.reject(selector)),
    removePeer: (selector) => run(() => daemon.removePeer(selector)),
    setLayout: (host, edge) => run(() => daemon.setLayout(host, edge)),
    setClipboard: (enabled) => run(() => daemon.setClipboard(enabled)),

    stopDaemon: async () => {
      try {
        await daemon.stop();
        set({ connection: "disconnected", status: null, peers: [], placements: [] });
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
export const useDaemonError = (): string | null => useDaemonStore((s) => s.error);
export const useDaemonVersion = (): string => useDaemonStore((s) => s.daemonVersion);

export const useSessions = (): SessionInfo[] =>
  useDaemonStore((s) => s.status?.sessions ?? EMPTY_SESSIONS);

export const usePending = (): PendingInfo[] =>
  useDaemonStore((s) => s.status?.pending ?? EMPTY_PENDING);

export const useClipboardSharing = (): boolean =>
  useDaemonStore((s) => s.status?.clipboard_sharing ?? false);

export const useIsConnected = (): boolean => useDaemonStore((s) => s.connection === "connected");
