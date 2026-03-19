import "./App.css";
import { ThemeProvider } from "./components/theme-provider";
import { WelcomeView } from "./components/welcome/wrapper";

function App() {
  return (
    <ThemeProvider defaultTheme="dark" storageKey="vite-ui-theme">
      <main className="w-full min-h-screen">
        <WelcomeView />
      </main>
    </ThemeProvider>
  );
}

export default App;
