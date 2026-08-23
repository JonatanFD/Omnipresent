import { Info } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { SettingsRow, SettingsSection } from "@/components/settings";
import { useDaemonVersion } from "@/stores/daemon-store";

/**
 * Updating replaces the whole app, daemon included, because the daemon ships
 * inside it. The action itself is not wired yet: it needs the Tauri updater
 * plugin, a signing key pair, and a release endpoint to check against.
 */
export function UpdateView() {
  const version = useDaemonVersion();

  return (
    <div className="space-y-4">
      <SettingsSection title="Version">
        <SettingsRow label="Installed version">
          <span className="text-sm text-muted-foreground">
            {version ? `v${version}` : "Unknown"}
          </span>
        </SettingsRow>
      </SettingsSection>

      <Alert>
        <Info />
        <AlertDescription>
          Automatic updates are not set up yet. Because the daemon ships inside this
          app, updating it means replacing the app — that needs the updater plugin, a
          signing key, and a release endpoint before this pane can do anything.
        </AlertDescription>
      </Alert>
    </div>
  );
}
