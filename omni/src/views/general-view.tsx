import { AlertTriangle, CheckCircle2, CircleSlash, MoreHorizontal, OctagonAlert } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Fingerprint, SettingsRow, SettingsSection } from "@/components/settings";
import {
  useConnection,
  useDaemonError,
  useDaemonStore,
  useDaemonVersion,
  useEmbedded,
  useStatus,
} from "@/stores/daemon-store";

const CONNECTION_LABEL = {
  connected: "Running",
  connecting: "Starting…",
  disconnected: "Not running",
  incompatible: "Incompatible version",
} as const;

const CONNECTION_ICON = {
  connected: CheckCircle2,
  connecting: MoreHorizontal,
  disconnected: CircleSlash,
  incompatible: OctagonAlert,
} as const;

export function GeneralView() {
  const connection = useConnection();
  const status = useStatus();
  const version = useDaemonVersion();
  const embedded = useEmbedded();
  const error = useDaemonError();

  const refresh = useDaemonStore((s) => s.refresh);
  const startDaemon = useDaemonStore((s) => s.startDaemon);
  const stopDaemon = useDaemonStore((s) => s.stopDaemon);

  const isConnected = connection === "connected";
  const isIncompatible = connection === "incompatible";
  const isBusy = connection === "connecting";
  const StatusIcon = CONNECTION_ICON[connection];

  if (isIncompatible) {
    return (
      <div className="space-y-4">
        <Alert variant="destructive">
          <OctagonAlert />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <SettingsSection title="Daemon">
        <SettingsRow label="Status">
          <span
            className={
              isConnected
                ? "flex items-center gap-1.5 text-sm"
                : "flex items-center gap-1.5 text-sm text-muted-foreground"
            }
          >
            <StatusIcon className="size-4" />
            {CONNECTION_LABEL[connection]}
          </span>
        </SettingsRow>
        <SettingsRow
          label="Controls"
          description={
            embedded
              ? "The daemon runs inside this app. Quitting stops input sharing."
              : "Using a daemon that was already running. Quitting this app leaves it running."
          }
        >
          {isConnected ? (
            <Button
              variant="destructive"
              size="sm"
              disabled={isBusy}
              onClick={() => void stopDaemon()}
            >
              Stop
            </Button>
          ) : (
            <Button size="sm" disabled={isBusy} onClick={() => void startDaemon()}>
              Start
            </Button>
          )}
          <Button variant="outline" size="sm" onClick={() => void refresh()}>
            Reconnect
          </Button>
        </SettingsRow>
      </SettingsSection>

      {isConnected && status ? (
        <SettingsSection title="Info">
          <SettingsRow
            label="Input capture"
            description={
              status.capturing
                ? "This machine can drive its peers."
                : "This machine can only be driven by a peer."
            }
          >
            <Badge variant={status.capturing ? "default" : "secondary"}>
              {status.capturing ? "Active" : "Target only"}
            </Badge>
          </SettingsRow>
          <SettingsRow label="Port">
            <span className="font-mono text-xs text-muted-foreground">{status.port}</span>
          </SettingsRow>
          <SettingsRow
            label="Fingerprint"
            description="What peers pin the first time they accept this machine."
            className="items-start"
          >
            <div className="max-w-[15rem]">
              <Fingerprint value={status.fingerprint} />
            </div>
          </SettingsRow>
          {version ? (
            <SettingsRow label="Version">
              <span className="text-sm text-muted-foreground">v{version}</span>
            </SettingsRow>
          ) : null}
        </SettingsSection>
      ) : null}

      {error && !isIncompatible ? (
        <Alert>
          <AlertTriangle />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}
    </div>
  );
}
