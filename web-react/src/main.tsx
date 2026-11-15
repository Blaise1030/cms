import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@lenso/tokens/styles.css";
import "./app.css";
import { GlassProvider, ThemeScope } from "@lenso/ui";
import { App } from "./App";

const root = document.getElementById("root");
if (!root) throw new Error("#root missing");

createRoot(root).render(
  <StrictMode>
    <GlassProvider>
      <ThemeScope theme="light" style={{ minHeight: "100%" }}>
        <App />
      </ThemeScope>
    </GlassProvider>
  </StrictMode>,
);
