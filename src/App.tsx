import { useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

type Suggestion = {
  original: string;
  suggestion: string;
  type: "grammar" | "clarity" | "tone" | "concision";
  explanation: string;
};

const DEBOUNCE_MS = 1500;

export default function App() {
  const [text, setText] = useState("");
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const fetchSuggestions = useCallback(async (value: string) => {
    if (!value.trim()) {
      setSuggestions([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      // The actual Claude API call happens in Rust (src-tauri/src/main.rs)
      // so the API key never touches the frontend bundle.
      const result = await invoke<Suggestion[]>("get_suggestions", { text: value });
      setSuggestions(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  const handleChange = (value: string) => {
    setText(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => fetchSuggestions(value), DEBOUNCE_MS);
  };

  const acceptSuggestion = (s: Suggestion) => {
    setText((prev) => prev.replace(s.original, s.suggestion));
    setSuggestions((prev) => prev.filter((x) => x !== s));
  };

  const dismissSuggestion = (s: Suggestion) => {
    setSuggestions((prev) => prev.filter((x) => x !== s));
  };

  return (
    <div className="app">
      <header className="app-header">
        <span className="logo-dot" />
        <h1>WriteWise</h1>
        {loading && <span className="status">checking…</span>}
      </header>

      <main className="main">
        <textarea
          className="editor"
          placeholder="Paste or type text here. Suggestions appear ~1.5s after you pause typing."
          value={text}
          onChange={(e) => handleChange(e.target.value)}
        />

        <aside className="suggestions-panel">
          <h2>Suggestions {suggestions.length > 0 && `(${suggestions.length})`}</h2>
          {error && <p className="error">{error}</p>}
          {!error && suggestions.length === 0 && !loading && (
            <p className="empty">No suggestions yet. Start typing.</p>
          )}
          {suggestions.map((s, i) => (
            <div key={i} className={`suggestion-card type-${s.type}`}>
              <div className="suggestion-type">{s.type}</div>
              <div className="suggestion-diff">
                <span className="original">{s.original}</span>
                <span className="arrow">→</span>
                <span className="revised">{s.suggestion}</span>
              </div>
              <p className="explanation">{s.explanation}</p>
              <div className="suggestion-actions">
                <button className="accept" onClick={() => acceptSuggestion(s)}>
                  Accept
                </button>
                <button className="dismiss" onClick={() => dismissSuggestion(s)}>
                  Dismiss
                </button>
              </div>
            </div>
          ))}
        </aside>
      </main>
    </div>
  );
}
