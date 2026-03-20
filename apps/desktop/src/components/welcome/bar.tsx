import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button } from "../ui/button";

const appWindow = getCurrentWindow();

export function WelcomeBar() {
  return (
    <div className="titlebar fixed top-0 left-0 right-0 z-10 flex justify-between items-center px-2 py-1">
      {/* Zona draggable */}
      <div data-tauri-drag-region className="flex-1 select-none">
        <h1 className="pointer-events-none">Omnipresent</h1>
      </div>

      {/* Controles */}
      <div className="flex gap-2">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => appWindow.minimize()}
        >
          _
        </Button>

        <Button
          variant="ghost"
          size="icon"
          onClick={() => appWindow.toggleMaximize()}
        >
          ☐
        </Button>

        <Button variant="ghost" size="icon" onClick={() => appWindow.close()}>
          ✕
        </Button>
      </div>
    </div>
  );
}
