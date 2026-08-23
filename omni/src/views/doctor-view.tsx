import { CheckCircle2, HelpCircle, TriangleAlert, XCircle } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { SettingsRow, SettingsSection } from "@/components/settings";
import { health, useChecks, useDaemonStore } from "@/stores/daemon-store";

/**
 * `omni doctor`, in the window.
 *
 * The reason it earns a pane of its own: everywhere else the app reports what
 * *is* — sessions, peers, versions. This is the only place that says whether
 * what is, is right. Without it a machine whose Accessibility permission was
 * never granted just reads "Target only" in General, with nothing anywhere
 * explaining why it cannot drive anything.
 */
export function DoctorView() {
  const checks = useChecks();
  const runDoctor = useDaemonStore((s) => s.runDoctor);
  const verdict = health(checks);

  return (
    <div className="space-y-4">
      <Verdict />

      {checks && checks.length > 0 ? (
        <SettingsSection
          title="Checks"
          footer="Granting a permission does not reach a daemon that is already running — stop and start it in General afterwards."
        >
          {/* Failures first: the whole point of the pane is what is wrong, and
              on a healthy machine the order makes no difference anyway. */}
          {[...checks]
            .sort((a, b) => Number(a.ok) - Number(b.ok))
            .map((check) => (
              <SettingsRow
                key={check.name}
                className="items-start"
                label={
                  <span className="flex items-center gap-1.5">
                    {check.ok ? (
                      <CheckCircle2
                        className="size-4 shrink-0 text-emerald-500"
                        aria-label="Passing"
                      />
                    ) : (
                      <XCircle className="size-4 shrink-0 text-destructive" aria-label="Failing" />
                    )}
                    {check.name}
                  </span>
                }
                description={check.detail}
              />
            ))}
        </SettingsSection>
      ) : null}

      <SettingsSection>
        <SettingsRow
          label="Re-run"
          description={
            verdict.state === "unknown"
              ? "Nothing has been checked yet."
              : "Every check is read fresh from the OS and the daemon."
          }
        >
          <Button variant="outline" size="sm" onClick={() => void runDoctor()}>
            Run checks
          </Button>
        </SettingsRow>
      </SettingsSection>
    </div>
  );
}

/** The answer to "is everything ok", before any of the detail. */
function Verdict() {
  const checks = useChecks();
  const verdict = health(checks);

  if (verdict.state === "unknown") {
    return (
      <Alert>
        <HelpCircle />
        <AlertTitle>Not checked yet</AlertTitle>
        <AlertDescription>Run the checks to see how this machine is set up.</AlertDescription>
      </Alert>
    );
  }

  if (verdict.state === "ok") {
    return (
      <Alert>
        <CheckCircle2 className="text-emerald-500" />
        <AlertTitle>Everything looks good</AlertTitle>
        <AlertDescription>
          This machine has the permissions and environment it needs to share a keyboard and
          mouse.
        </AlertDescription>
      </Alert>
    );
  }

  return (
    <Alert variant="destructive">
      <TriangleAlert />
      <AlertTitle>
        {verdict.failing === 1 ? "1 problem found" : `${verdict.failing} problems found`}
      </AlertTitle>
      <AlertDescription>
        Each one below says what was found and how to fix it. Until they are resolved this
        machine may only be able to be driven, not to drive.
      </AlertDescription>
    </Alert>
  );
}
