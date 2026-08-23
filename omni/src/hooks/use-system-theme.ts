import { useEffect } from "react";

/**
 * Keeps shadcn's `.dark` class in step with the operating system's appearance.
 *
 * A desktop app is expected to follow the system's light/dark setting rather than
 * carry its own toggle, so this only mirrors what the OS reports. The webview
 * surfaces that setting through `prefers-color-scheme`, and Tauri passes through
 * changes while the app is running.
 */
export function useSystemTheme(): void {
  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");

    const apply = (dark: boolean) => {
      document.documentElement.classList.toggle("dark", dark);
    };

    apply(query.matches);
    const onChange = (event: MediaQueryListEvent) => apply(event.matches);

    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);
}
