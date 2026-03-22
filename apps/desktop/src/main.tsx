import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ThemeProvider } from "./components/theme-provider";

const preventReload = (event: KeyboardEvent) => {
  const isReloadKey = event.key === "F5";
  const isCmdOrCtrlR = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "r";

  if (isReloadKey || isCmdOrCtrlR) {
    event.preventDefault();
  }
};

window.addEventListener("keydown", preventReload);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <ThemeProvider defaultTheme="dark" storageKey="vite-ui-theme">
    <React.StrictMode>
      <App />
    </React.StrictMode>
  </ThemeProvider>,
);
