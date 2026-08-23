import { Antenna, Download, Network, Settings } from "lucide-react";
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
import { useConnection, usePending } from "@/stores/daemon-store";

/** The four panes, in the order the macOS app lists them. */
export type Section = "general" | "connections" | "system" | "update";

const SECTIONS: { id: Section; label: string; icon: LucideIcon }[] = [
  { id: "general", label: "General", icon: Antenna },
  { id: "connections", label: "Connections", icon: Network },
  { id: "system", label: "System", icon: Settings },
  { id: "update", label: "Update", icon: Download },
];

export const SECTION_TITLES: Record<Section, string> = {
  general: "General",
  connections: "Connections",
  system: "System",
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
