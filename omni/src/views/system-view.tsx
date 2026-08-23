import { SettingsRow, SettingsSection } from "@/components/settings";
import { Switch } from "@/components/ui/switch";
import { useClipboardSharing, useDaemonStore, useIsConnected } from "@/stores/daemon-store";

export function SystemView() {
  const isConnected = useIsConnected();
  const clipboardSharing = useClipboardSharing();
  const setClipboard = useDaemonStore((s) => s.setClipboard);

  return (
    <div className="space-y-4">
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
    </div>
  );
}
