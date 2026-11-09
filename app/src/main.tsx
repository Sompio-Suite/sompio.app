import "@fontsource-variable/inter";
import "@fontsource-variable/space-grotesk";
import "./styles.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import Popup from "./views/Popup";
import AutoTypeChooser from "./views/AutoTypeChooser";
import { api } from "./api";
import Recover from "./Recover";

// Right-click menu off outside development: it only offers "Reload" and "Inspect".
if (import.meta.env.PROD) window.addEventListener("contextmenu", (e) => e.preventDefault());

// Anything that goes wrong in the window lands in the local problems log (never sent anywhere).
const where = window.location.hash || "#main";
window.addEventListener("error", (e) => {
  api.logProblem(`${where} ${e.message} at ${e.filename?.split("/").pop()}:${e.lineno}`).catch(() => {});
});
window.addEventListener("unhandledrejection", (e) => {
  const r = e.reason;
  api.logProblem(`${where} unhandled: ${r instanceof Error ? r.message : String(r)}`).catch(() => {});
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Recover>
    {window.location.hash === "#popup" ? <Popup /> : window.location.hash === "#autotype" ? <AutoTypeChooser /> : <App />}
    </Recover>
  </StrictMode>,
);
