import { Info } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { SettingsRow, SettingsSection } from "@/components/settings";
import { useAppVersion, useDaemonVersion, useEmbedded } from "@/stores/daemon-store";

/**
 * Updating replaces the whole app, daemon included, because the daemon ships
 * inside it. The action itself is not wired yet: it needs the Tauri updater
 * plugin, a signing key pair, and a release endpoint to check against.
 */
export function UpdateView() {
  const appVersion = useAppVersion();
  const daemonVersion = useDaemonVersion();
  const embedded = useEmbedded();

  // They are the same build when the daemon came from inside this app, and can
  // differ when it did not — an older `omni start` daemon, say. Showing one
  // number for both would hide exactly the case worth knowing about.
  const differs = Boolean(daemonVersion) && daemonVersion !== appVersion;

  return (
    <div className="space-y-4">
      <SettingsSection title="Version">
        <SettingsRow label="Omnipresent">
          <span className="text-sm text-muted-foreground">
            {appVersion ? `v${appVersion}` : "Unknown"}
          </span>
        </SettingsRow>
        <SettingsRow
          label="Daemon"
          description={embedded ? "Running inside this app" : "Was already running"}
        >
          <span className="text-sm text-muted-foreground">
            {daemonVersion ? `v${daemonVersion}` : "Not running"}
          </span>
        </SettingsRow>
      </SettingsSection>

      {differs ? (
        <Alert>
          <Info />
          <AlertDescription>
            This app and the daemon it is talking to are different builds. Both machines in a
            session must run matching versions — a peer on an older one cannot answer a
            message it does not know.
          </AlertDescription>
        </Alert>
      ) : null}

      <Alert>
        <Info />
        <AlertDescription>
          Automatic updates are not set up yet. Because the daemon ships inside this app,
          updating it means replacing the app — that needs the updater plugin, a signing key,
          and a release endpoint before this pane can do anything.
        </AlertDescription>
      </Alert>
    </div>
  );
}
