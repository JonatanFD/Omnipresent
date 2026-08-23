import type { ReactNode } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";

/**
 * The grouped-form building blocks the detail panes are made of.
 *
 * These stand in for SwiftUI's `Form(.grouped)` and `LabeledContent` so the
 * Windows and Linux windows lay out the same information in the same order as the
 * macOS app, which is where the layout comes from.
 */

export function SettingsSection({
  title,
  footer,
  children,
}: {
  title?: string;
  footer?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Card className="gap-0 py-0">
      {title ? (
        <CardHeader className="px-4 pt-4 pb-0">
          <CardTitle className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
            {title}
          </CardTitle>
        </CardHeader>
      ) : null}
      <CardContent className="divide-y divide-border px-0 py-0">{children}</CardContent>
      {footer ? (
        <div className="border-t px-4 py-3 text-xs text-muted-foreground">{footer}</div>
      ) : null}
    </Card>
  );
}

/** One row: a label on the left, a value or control on the right. */
export function SettingsRow({
  label,
  description,
  children,
  className,
}: {
  label: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("flex items-center justify-between gap-4 px-4 py-3", className)}>
      <div className="min-w-0 space-y-0.5">
        <div className="text-sm">{label}</div>
        {description ? (
          <div className="text-xs text-muted-foreground">{description}</div>
        ) : null}
      </div>
      {children ? <div className="flex shrink-0 items-center gap-2">{children}</div> : null}
    </div>
  );
}

/** A row whose content stacks instead of sitting beside the label. */
export function SettingsStack({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return <div className={cn("space-y-2 px-4 py-3", className)}>{children}</div>;
}

/**
 * A fingerprint. Shown in a monospaced face and selectable, because it is what
 * the user compares by eye to verify a peer — it is not secret, but it must be
 * readable character by character.
 */
export function Fingerprint({ value }: { value: string }) {
  return (
    <span className="truncate font-mono text-xs text-muted-foreground select-all" title={value}>
      {value}
    </span>
  );
}

/** The centred "nothing here yet" pane, standing in for ContentUnavailableView. */
export function EmptyState({
  icon,
  title,
  description,
}: {
  icon: ReactNode;
  title: string;
  description: string;
}) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 px-8 py-16 text-center">
      <div className="text-muted-foreground [&_svg]:size-10">{icon}</div>
      <div className="space-y-1">
        <p className="text-sm font-medium">{title}</p>
        <p className="max-w-xs text-xs text-muted-foreground">{description}</p>
      </div>
    </div>
  );
}
