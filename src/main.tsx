import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { SuggestionPanel } from "./windows/SuggestionPanel";
import "./App.css";

// Both windows load the same bundle; branch on the Tauri window label so the
// floating panel renders its compact UI instead of the full editor.
function currentLabel(): string {
  if (!("__TAURI_INTERNALS__" in window)) return "main";
  try {
    return getCurrentWindow().label;
  } catch {
    return "main";
  }
}

const isPanel = currentLabel() === "suggestions";
if (isPanel) document.body.classList.add("panel-window");
const Root = isPanel ? SuggestionPanel : App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>
);
