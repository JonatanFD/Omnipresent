import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * The store's dealings with the daemon.
 *
 * The rule these tests hold to is the one the store is built on: the daemon owns
 * the state, so a command that fails must leave the window showing what the
 * daemon last confirmed rather than what the user hoped for.
 */

/** Answers keyed by Tauri command name, replaced per test. */
let answers: Record<string, unknown> = {};
/** Commands that should reject, and with what. */
let failures: Record<string, string> = {};
/** Every command invoked, in order. */
let invoked: string[] = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string) => {
    invoked.push(command);
    if (command in failures) return Promise.reject(failures[command]);
    return Promise.resolve(answers[command]);
  },
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

const { exitMessage, health, useDaemonStore } = await import("./daemon-store");

const STATUS = {
  fingerprint: "ab12",
  port: 4733,
  capturing: true,
  clipboard_sharing: false,
  sessions: [],
  pending: [],
};

/** The answers a healthy daemon gives. */
function healthy() {
  return {
    daemon_hello: { protocol_version: 1, daemon_version: "0.7.3", compatible: true },
    daemon_status: STATUS,
    daemon_embedded: true,
    peer_list: [],
    layout_list: [],
    modifiers_list: [],
  };
}

beforeEach(() => {
  answers = healthy();
  failures = {};
  invoked = [];
  useDaemonStore.setState(useDaemonStore.getInitialState(), true);
});

describe("refresh", () => {
  it("reports a healthy daemon as connected", async () => {
    await useDaemonStore.getState().refresh();

    const state = useDaemonStore.getState();
    expect(state.connection).toBe("connected");
    expect(state.daemonVersion).toBe("0.7.3");
    expect(state.error).toBeNull();
  });

  it("reports a daemon newer than this app as incompatible", async () => {
    // Guessing at a protocol we do not know would misbehave silently; the point
    // of the handshake is to say so instead.
    answers.daemon_hello = { protocol_version: 99, daemon_version: "9.0.0", compatible: false };

    await useDaemonStore.getState().refresh();

    expect(useDaemonStore.getState().connection).toBe("incompatible");
    expect(useDaemonStore.getState().error).toContain("Update Omnipresent");
  });

  it("does not read status from a daemon it cannot understand", async () => {
    answers.daemon_hello = { protocol_version: 99, daemon_version: "9.0.0", compatible: false };

    await useDaemonStore.getState().refresh();

    expect(invoked).not.toContain("daemon_status");
  });

  it("clears the stale snapshot when the daemon goes away", async () => {
    await useDaemonStore.getState().refresh();
    failures.daemon_hello = "the daemon is not running";

    await useDaemonStore.getState().refresh();

    const state = useDaemonStore.getState();
    expect(state.connection).toBe("disconnected");
    expect(state.status).toBeNull();
  });

  it("records whether the daemon is the one inside this app", async () => {
    answers.daemon_embedded = false;

    await useDaemonStore.getState().refresh();

    expect(useDaemonStore.getState().embedded).toBe(false);
  });
});

describe("commands", () => {
  it("re-reads the daemon after a command instead of guessing", async () => {
    await useDaemonStore.getState().connect("studio");

    expect(invoked).toContain("peer_connect");
    expect(invoked).toContain("daemon_status");
  });

  it("keeps the last confirmed state when a command fails", async () => {
    await useDaemonStore.getState().refresh();
    failures.peer_connect = "no route to host";

    await useDaemonStore.getState().connect("studio");

    const state = useDaemonStore.getState();
    expect(state.error).toBe("no route to host");
    expect(state.connection).toBe("connected");
    expect(state.status).toEqual(STATUS);
  });

  it("sends a modifier swap for one host only", async () => {
    await useDaemonStore.getState().setModifiers("pc", "meta-control");

    expect(invoked).toContain("modifiers_set");
  });
});

describe("start and stop", () => {
  it("can start the daemon again after stopping it", async () => {
    // The whole point of the supervisor: Stop used to be a one-way door, and the
    // only way back was quitting the app.
    await useDaemonStore.getState().refresh();
    await useDaemonStore.getState().stopDaemon();
    expect(useDaemonStore.getState().connection).toBe("disconnected");

    await useDaemonStore.getState().startDaemon();

    expect(invoked).toContain("daemon_start");
    expect(useDaemonStore.getState().connection).toBe("connected");
  });

  it("forgets peers and placements when the daemon stops", async () => {
    answers.peer_list = [{ host: "studio", fingerprint: "aa", connected: true }];
    answers.layout_list = [{ host: "studio", edge: "right", connected: true }];
    answers.modifiers_list = [{ host: "studio", swap: "none", connected: true }];
    await useDaemonStore.getState().refresh();

    await useDaemonStore.getState().stopDaemon();

    const state = useDaemonStore.getState();
    expect(state.peers).toEqual([]);
    expect(state.placements).toEqual([]);
    expect(state.swaps).toEqual([]);
  });

  it("surfaces a daemon that refuses to start", async () => {
    failures.daemon_start = "the daemon is already running in this app";
    failures.daemon_hello = "the daemon is not running";

    await useDaemonStore.getState().startDaemon();

    expect(useDaemonStore.getState().error).toContain("already running");
  });

  it("does not let the follow-up checks wipe why the start failed", async () => {
    // `startDaemon` re-runs doctor, because the daemon's capture state has just
    // changed. That background success must not clear the foreground error —
    // there is only one error slot, and the user has just been shown it.
    failures.daemon_start = "the daemon is already running in this app";
    failures.daemon_hello = "the daemon is not running";
    answers.daemon_doctor = [{ name: "daemon", ok: false, detail: "not running" }];

    await useDaemonStore.getState().startDaemon();
    await Promise.resolve();

    expect(useDaemonStore.getState().error).toContain("already running");
    expect(useDaemonStore.getState().checks).toHaveLength(1);
  });
});

describe("exitMessage", () => {
  it("says nothing when the daemon was asked to stop", () => {
    expect(exitMessage({ reason: null, stood_down: false })).toBeNull();
  });

  it("says nothing when ours stood down for one already running", () => {
    // The ordinary case when `omni start` got there first. The window works
    // exactly the same against that daemon, so an error here would be a lie.
    expect(
      exitMessage({ reason: "QUIC endpoint: address already in use", stood_down: true }),
    ).toBeNull();
  });

  it("reports a daemon that really did fail", () => {
    const message = exitMessage({ reason: "identity: permission denied", stood_down: false });

    expect(message).toContain("identity: permission denied");
  });
});

describe("health", () => {
  const passing = { name: "accessibility permission", ok: true, detail: "granted" };
  const failing = { name: "daemon", ok: false, detail: "capture is off" };

  it("does not call an unchecked machine healthy", () => {
    // The one wrong answer this can give: a reassuring verdict before anything
    // has been looked at is the one nobody would think to question.
    expect(health(null)).toEqual({ state: "unknown", failing: 0 });
  });

  it("reports a machine whose checks all pass", () => {
    expect(health([passing, passing])).toEqual({ state: "ok", failing: 0 });
  });

  it("counts only the failing checks", () => {
    expect(health([passing, failing, passing, failing])).toEqual({
      state: "problems",
      failing: 2,
    });
  });

  it("treats no checks at all as nothing wrong", () => {
    // An empty list is a run that found nothing to complain about, unlike null,
    // which is a run that never happened.
    expect(health([])).toEqual({ state: "ok", failing: 0 });
  });
});

describe("doctor and the daemon log", () => {
  it("stores the checks it was given", async () => {
    answers.daemon_doctor = [
      { name: "accessibility permission", ok: false, detail: "not granted" },
    ];

    await useDaemonStore.getState().runDoctor();

    expect(useDaemonStore.getState().checks).toHaveLength(1);
    expect(useDaemonStore.getState().checks?.[0].ok).toBe(false);
  });

  it("reports a doctor run that could not happen", async () => {
    failures.daemon_doctor = "cannot resolve the state directory";

    await useDaemonStore.getState().runDoctor();

    expect(useDaemonStore.getState().checks).toBeNull();
    expect(useDaemonStore.getState().error).toContain("state directory");
  });

  it("keeps the daemon log it was given", async () => {
    answers.daemon_log = {
      path: "/home/someone/.config/omni/daemon.log",
      lines: ["daemon starting", "QUIC endpoint: address already in use"],
    };

    await useDaemonStore.getState().readLog();

    expect(useDaemonStore.getState().log?.lines).toHaveLength(2);
  });

  it("does not wipe the error that made the user open the log", async () => {
    // The log is read *because* something failed. Clearing the reason on a
    // successful read would remove the very thing being investigated.
    answers.daemon_log = { path: "/tmp/daemon.log", lines: ["boom"] };
    useDaemonStore.setState({ error: "The daemon stopped: address in use" });

    await useDaemonStore.getState().readLog();

    expect(useDaemonStore.getState().error).toContain("address in use");
    expect(useDaemonStore.getState().log?.lines).toEqual(["boom"]);
  });
});
