import { useEffect, useState } from "react";
import { AppSidebar, SECTION_TITLES, type Section } from "@/components/app-sidebar";
import { ScrollArea } from "@/components/ui/scroll-area";
import { SidebarProvider } from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useSystemTheme } from "@/hooks/use-system-theme";
import { useDaemonStore } from "@/stores/daemon-store";
import { ConnectionsView } from "@/views/connections-view";
import { GeneralView } from "@/views/general-view";
import { SystemView } from "@/views/system-view";
import { UpdateView } from "@/views/update-view";

/**
 * The main window: a sidebar for navigation and a detail pane per section, the
 * same shape and the same four sections as the macOS app.
 */
export default function App() {
  useSystemTheme();

  const [section, setSection] = useState<Section>("general");
  const init = useDaemonStore((s) => s.init);

  useEffect(() => {
    // `init` resolves to its own cleanup, so the listeners are torn down even if
    // the effect is re-run before the subscription finished attaching.
    let cleanup: (() => void) | undefined;
    let cancelled = false;

    void init().then((unlisten) => {
      if (cancelled) {
        unlisten();
        return;
      }
      cleanup = unlisten;
    });

    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [init]);

  return (
    <TooltipProvider>
      <SidebarProvider className="h-screen min-h-0">
        <AppSidebar section={section} onSelect={setSection} />

        <main className="flex min-w-0 flex-1 flex-col">
          <header className="flex h-12 shrink-0 items-center border-b px-6">
            <h1 className="text-sm font-medium">{SECTION_TITLES[section]}</h1>
          </header>

          <ScrollArea className="flex-1">
            <div className="mx-auto flex w-full max-w-2xl flex-col p-6">
              {section === "general" && <GeneralView />}
              {section === "connections" && <ConnectionsView />}
              {section === "system" && <SystemView />}
              {section === "update" && <UpdateView />}
            </div>
          </ScrollArea>
        </main>
      </SidebarProvider>
    </TooltipProvider>
  );
}
