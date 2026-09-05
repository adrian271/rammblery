import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

type Suggestion = {
  original: string;
  suggestion: string;
  type: "grammar" | "clarity" | "tone" | "concision";
  explanation: string;
};

type ShowPayload = {
  session_id: number;
  suggestions: Suggestion[];
};

// The floating panel shown next to the focused text field in other apps.
// M2 is display-only: it renders suggestions pushed from the Rust checker.
// Accept/reject write-back arrives in M3.
export function SuggestionPanel() {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);

  useEffect(() => {
    const unlistenShow = listen<ShowPayload>("suggestions://show", (e) => {
      setSuggestions(e.payload.suggestions);
    });
    const unlistenHide = listen("suggestions://hide", () => {
      setSuggestions([]);
    });
    return () => {
      unlistenShow.then((f) => f());
      unlistenHide.then((f) => f());
    };
  }, []);

  return (
    <div className="panel">
      <div className="panel-header">
        <span className="logo-dot" />
        <span className="panel-title">Rammblery</span>
        <span className="panel-count">{suggestions.length}</span>
      </div>
      <div className="panel-body">
        {suggestions.map((s, i) => (
          <div key={i} className={`suggestion-card type-${s.type}`}>
            <div className="suggestion-type">{s.type}</div>
            <div className="suggestion-diff">
              <span className="original">{s.original}</span>
              <span className="arrow">→</span>
              <span className="revised">{s.suggestion}</span>
            </div>
            <p className="explanation">{s.explanation}</p>
          </div>
        ))}
      </div>
    </div>
  );
}
