import "./App.css";
import { WelcomeView } from "./components/welcome/wrapper";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { getFromLocalStorage } from "./lib/utils";
import { Home } from "./components/dashboard/home";

function App() {
  const hasAcceptedTerms = getFromLocalStorage("acceptedTerms") === "true";

  const defaultRoute = [hasAcceptedTerms ? "/" : "/welcome"];

  return (
    <MemoryRouter initialEntries={defaultRoute}>
      <Routes>
        <Route path="/welcome" element={<WelcomeView />} />
        <Route path="/" element={<Home />} />
      </Routes>
    </MemoryRouter>
  );
}

export default App;
