import "@fontsource-variable/atkinson-hyperlegible-next";
import "./styles.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";

// Right-click menu off outside development: it only offers "Reload" and "Inspect".
if (import.meta.env.PROD) window.addEventListener("contextmenu", (e) => e.preventDefault());

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
