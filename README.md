# Rammblery (Claude-powered Grammarly-style writing assistant)

Phase 1 scaffold: paste-in text box → debounced Claude suggestions → accept/reject cards.
No OS-level text capture yet (that's Phase 3/4 — see below).

## Setup

**Prerequisites:** Node.js 18+, Rust (via [rustup](https://rustup.rs)), and the Tauri CLI
prerequisites for your OS: https://tauri.app/start/prerequisites/

```bash
npm install
cp .env.example .env   # then put your ANTHROPIC_API_KEY in .env (gitignored)
npm run tauri dev
```

This opens a desktop window with a text editor on the left and a suggestions panel on
the right. Type or paste text, pause for ~1.5s, and suggestions stream in.

## How it works

- **Frontend** (`src/App.tsx`): plain text editor with a debounced `invoke("get_suggestions", ...)`
  call to the Rust backend. Never calls the Claude API directly — this keeps your API key
  out of the shipped app bundle, which anyone could otherwise extract from an Electron/Tauri build.
- **Backend** (`src-tauri/src/main.rs`): a single Tauri command that calls the Anthropic
  Messages API with a forced tool-use call (`report_suggestions`), so Claude returns
  structured JSON instead of prose you'd have to parse with regex.

## Where this goes next (Phases 2–4 from the plan)

1. **Live-typing mode inside this app** — swap the debounce trigger from "on pause" to
   "on sentence boundary" for a more responsive feel, and add a local spellchecker
   (e.g. `nspell`) for instant red-squiggle feedback without hitting the API per keystroke.
2. **Browser extension companion** — a content script that reads/writes `contentEditable`
   and `<textarea>` elements, talking to this Tauri app over a local WebSocket
   (Tauri can run a small local server in the Rust backend for this).
3. **macOS system-wide integration** — use the Accessibility API (`AXUIElement`,
   `AXFocusedUIElement`, `AXValue`) from Rust via the `accessibility-sys` or `cacao` crates,
   gated behind an `AXIsProcessTrusted()` permission prompt. This is the part with real
   platform-specific complexity — floating popup positioning, focus-change observers,
   and per-app quirks (Electron/Java apps often have incomplete AX trees).

## Notes

- `tauri.conf.json` bundle icons point to `icons/` — add your own icon set before
  building a distributable (`npm run tauri build`); the dev server doesn't need them.
- Model is set to `claude-sonnet-4-6` in `main.rs` — swap in whichever model string
  you want to test against for latency/cost tradeoffs.
