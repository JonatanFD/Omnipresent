import { Antenna, Download, Network, Settings, Stethoscope } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { useConnection, useFailingChecks, usePending } from "@/stores/daemon-store";

/**
 * The panes, in order. The first four are the macOS app's, unchanged; Doctor is
 * this client's own, because it links the runtime and can run the checks that
 * an IPC-only client has no message to ask for.
 */
export type Section = "general" | "connections" | "system" | "doctor" | "update";

const SECTIONS: { id: Section; label: string; icon: LucideIcon }[] = [
  { id: "general", label: "General", icon: Antenna },
  { id: "connections", label: "Connections", icon: Network },
  { id: "system", label: "System", icon: Settings },
  { id: "doctor", label: "Doctor", icon: Stethoscope },
  { id: "update", label: "Update", icon: Download },
];

export const SECTION_TITLES: Record<Section, string> = {
  general: "General",
  connections: "Connections",
  system: "System",
  doctor: "Doctor",
  update: "Update",
};

const STATUS_TEXT = {
  connected: "Daemon running",
  connecting: "Daemon starting…",
  disconnected: "Daemon not running",
  incompatible: "Daemon version incompatible",
} as const;

export function AppSidebar({
  section,
  onSelect,
}: {
  section: Section;
  onSelect: (section: Section) => void;
}) {
  const connection = useConnection();
  const pending = usePending();
  const failing = useFailingChecks();

  return (
    <Sidebar collapsible="none" className="border-r">
      <SidebarHeader className="px-4 py-3">
        <span className="text-sm font-semibold tracking-tight">Omnipresent</span>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {SECTIONS.map(({ id, label, icon: Icon }) => (
                <SidebarMenuItem key={id}>
                  <SidebarMenuButton isActive={section === id} onClick={() => onSelect(id)}>
                    <Icon />
                    <span>{label}</span>
                    {id === "general" ? <StatusDot connection={connection} /> : null}
                  </SidebarMenuButton>

                  {/* The pending count has to be visible without opening the pane:
                      an incoming request is waiting on a human decision. */}
                  {id === "connections" && pending.length > 0 ? (
                    <SidebarMenuBadge>{pending.length}</SidebarMenuBadge>
                  ) : null}

                  {/* Same reasoning for a failing check: nobody opens Doctor on
                      a machine they believe is working, which is exactly the
                      machine that is quietly target-only. */}
                  {id === "doctor" && failing > 0 ? (
                    <SidebarMenuBadge className="text-destructive">{failing}</SidebarMenuBadge>
                  ) : null}
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  );
}

/** The daemon's state at a glance, mirroring the dot in the macOS sidebar. */
function StatusDot({ connection }: { connection: keyof typeof STATUS_TEXT }) {
  return (
    <Tooltip>
      {/* Base UI composes through `render`, not Radix's `asChild`. */}
      <TooltipTrigger
        render={
          <span
            className={cn(
              "ml-auto size-2 shrink-0 rounded-full",
              connection === "connected" && "bg-emerald-500",
              connection === "connecting" && "bg-amber-500",
              connection === "incompatible" && "bg-amber-500",
              connection === "disconnected" && "bg-muted-foreground/40",
            )}
            role="img"
            aria-label={STATUS_TEXT[connection]}
          />
        }
      />
      <TooltipContent side="right">{STATUS_TEXT[connection]}</TooltipContent>
    </Tooltip>
  );
}
