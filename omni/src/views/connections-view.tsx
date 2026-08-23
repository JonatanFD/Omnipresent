import { useState } from "react";
import { Radio, Unplug } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  EmptyState,
  Fingerprint,
  SettingsRow,
  SettingsSection,
  SettingsStack,
} from "@/components/settings";
import { EDGES, type Edge } from "@/lib/ipc";
import {
  useDaemonStore,
  useIsConnected,
  usePeers,
  usePending,
  usePlacements,
  useSessions,
} from "@/stores/daemon-store";

/**
 * Connect, Peers and Layout in one pane, as the macOS app groups them: they all
 * answer the same question — who is this machine talking to, and where do they sit.
 */
export function ConnectionsView() {
  const isConnected = useIsConnected();
  const sessions = useSessions();
  const pending = usePending();
  const peers = usePeers();
  const placements = usePlacements();

  const hasData =
    sessions.length > 0 || pending.length > 0 || peers.length > 0 || placements.length > 0;

  if (!isConnected && !hasData) {
    return (
      <EmptyState
        icon={<Unplug />}
        title="Daemon not running"
        description="Start the daemon from General to connect to peers."
      />
    );
  }

  return (
    <div className="space-y-4">
      {isConnected ? <ConnectSection /> : null}
      {pending.length > 0 ? <IncomingSection /> : null}
      {sessions.length > 0 ? <SessionsSection /> : null}
      {peers.length > 0 ? <PeersSection /> : null}
      {placements.length > 0 ? <LayoutSection /> : null}
    </div>
  );
}

function ConnectSection() {
  const [host, setHost] = useState("");
  const connect = useDaemonStore((s) => s.connect);

  const submit = () => {
    const trimmed = host.trim();
    if (!trimmed) return;
    void connect(trimmed);
    setHost("");
  };

  return (
    <SettingsSection title="Connect to host">
      <SettingsStack>
        <form
          className="flex gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            submit();
          }}
        >
          <Input
            value={host}
            onChange={(event) => setHost(event.target.value)}
            placeholder="Host or IP address"
            aria-label="Host or IP address"
          />
          <Button type="submit" disabled={!host.trim()}>
            Connect
          </Button>
        </form>
      </SettingsStack>
    </SettingsSection>
  );
}

/**
 * The accept prompt. It shows the peer's name and fingerprint together, because
 * this is the point where the user verifies a machine before it is pinned for
 * good — the decision cannot be made from the name alone.
 */
function IncomingSection() {
  const pending = usePending();
  const accept = useDaemonStore((s) => s.accept);
  const reject = useDaemonStore((s) => s.reject);

  return (
    <SettingsSection
      title="Incoming requests"
      footer="Accepting pins this machine's certificate. Check the fingerprint matches the one shown on the other machine."
    >
      {pending.map((request) => (
        <SettingsStack key={request.fingerprint}>
          <div className="space-y-1">
            <p className="text-sm font-medium">{request.host}</p>
            <Fingerprint value={request.fingerprint} />
          </div>
          <div className="flex gap-2">
            <Button size="sm" onClick={() => void accept(request.fingerprint)}>
              Accept
            </Button>
            <Button
              size="sm"
              variant="destructive"
              onClick={() => void reject(request.fingerprint)}
            >
              Reject
            </Button>
          </div>
        </SettingsStack>
      ))}
    </SettingsSection>
  );
}

function SessionsSection() {
  const sessions = useSessions();
  const disconnect = useDaemonStore((s) => s.disconnect);

  return (
    <SettingsSection title="Active sessions">
      {sessions.map((session) => (
        <SettingsRow
          key={session.fingerprint}
          label={
            <span className="flex items-center gap-1.5">
              {session.active ? (
                <Radio
                  className="size-3.5 text-primary"
                  aria-label="Input is routed here"
                />
              ) : null}
              {session.host}
            </span>
          }
          description={session.role === "controller" ? "Controller" : "Target"}
        >
          <Button
            size="sm"
            variant="destructive"
            onClick={() => void disconnect(session.host)}
          >
            Disconnect
          </Button>
        </SettingsRow>
      ))}
    </SettingsSection>
  );
}

function PeersSection() {
  const peers = usePeers();
  const removePeer = useDaemonStore((s) => s.removePeer);

  return (
    <SettingsSection title="Known peers">
      {peers.map((peer) => (
        <SettingsRow
          key={peer.fingerprint}
          className="items-start"
          label={peer.host ?? "(unnamed)"}
          description={<Fingerprint value={peer.fingerprint} />}
        >
          <Button
            size="sm"
            variant="ghost"
            onClick={() => void removePeer(peer.host ?? peer.fingerprint)}
          >
            Forget
          </Button>
        </SettingsRow>
      ))}
    </SettingsSection>
  );
}

function LayoutSection() {
  const placements = usePlacements();
  const setLayout = useDaemonStore((s) => s.setLayout);

  return (
    <SettingsSection
      title="Screen layout"
      footer="Where each peer sits relative to this machine. The cursor crosses at that edge."
    >
      {placements.map((placement) => (
        <SettingsRow
          key={placement.host}
          label={placement.host}
          description={placement.connected ? undefined : "Saved for the next connection"}
        >
          <Select
            value={placement.edge}
            onValueChange={(edge) => void setLayout(placement.host, edge as Edge)}
          >
            <SelectTrigger size="sm" className="w-28" aria-label={`Edge for ${placement.host}`}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {EDGES.map((edge) => (
                <SelectItem key={edge} value={edge}>
                  {edge.charAt(0).toUpperCase() + edge.slice(1)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SettingsRow>
      ))}
    </SettingsSection>
  );
}
