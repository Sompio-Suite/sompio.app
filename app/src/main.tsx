import "@fontsource-variable/inter";
import "@fontsource-variable/space-grotesk";
import "./styles.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import Popup from "./views/Popup";

// Right-click menu off outside development: it only offers "Reload" and "Inspect".
if (import.meta.env.PROD) window.addEventListener("contextmenu", (e) => e.preventDefault());

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {window.location.hash === "#popup" ? <Popup /> : <App />}
  </StrictMode>,
);
