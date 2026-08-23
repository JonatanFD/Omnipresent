import { Info, Terminal } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { SettingsRow, SettingsSection, SettingsStack } from "@/components/settings";
import { useCli, useClipboardSharing, useDaemonStore, useIsConnected } from "@/stores/daemon-store";

/** Machine-wide settings: what this machine shares, and what it installs. */
export function SystemView() {
  return (
    <div className="space-y-4">
      <ClipboardSection />
      <CommandLineSection />
    </div>
  );
}

function ClipboardSection() {
  const isConnected = useIsConnected();
  const clipboardSharing = useClipboardSharing();
  const setClipboard = useDaemonStore((s) => s.setClipboard);

  return (
    <SettingsSection
      title="Clipboard"
      footer="Off by default. While off, nothing reads this machine's clipboard and no remote clipboard is applied."
    >
      <SettingsRow
        label="Share clipboard with connected peers"
        description="Copies made here become available on the machines you are connected to."
      >
        <Switch
          checked={clipboardSharing}
          disabled={!isConnected}
          onCheckedChange={(enabled) => void setClipboard(enabled)}
          aria-label="Share clipboard with connected peers"
        />
      </SettingsRow>
    </SettingsSection>
  );
}

/**
 * The bundled `omni` command.
 *
 * The app ships it, but putting it on PATH writes outside the bundle, so it
 * stays the user's decision rather than something that happens at launch.
 */
function CommandLineSection() {
  const cli = useCli();
  const installCli = useDaemonStore((s) => s.installCli);

  if (!cli) return null;

  return (
    <SettingsSection
      title="Command line"
      footer="The same omni command the CLI install scripts provide, from inside this app."
    >
      <SettingsRow
        className="items-start"
        label={
          <span className="flex items-center gap-1.5">
            <Terminal className="size-4 shrink-0" />
            omni
          </span>
        }
        description={cli.installed ? `Installed at ${cli.target}` : `Would install to ${cli.target}`}
      >
        <Button
          variant="outline"
          size="sm"
          disabled={!cli.available}
          onClick={() => void installCli()}
        >
          {cli.installed ? "Reinstall" : "Install"}
        </Button>
      </SettingsRow>

      {!cli.available ? (
        <SettingsStack>
          <Alert>
            <Info />
            <AlertDescription>
              This is a development build, which has no bundled command. A packaged app
              ships one.
            </AlertDescription>
          </Alert>
        </SettingsStack>
      ) : null}

      {cli.installed && !cli.on_path ? (
        <SettingsStack>
          <Alert>
            <Info />
            <AlertDescription>
              That directory is not on your PATH, so <code>omni</code> will not be found by
              name yet. Add it to your shell profile.
            </AlertDescription>
          </Alert>
        </SettingsStack>
      ) : null}
    </SettingsSection>
  );
}
