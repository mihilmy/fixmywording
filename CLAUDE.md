# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Fix My Wording is a native macOS menu bar app (Tauri 2) that improves selected text using the Anthropic Claude API via a global hotkey (Cmd+Shift+K). It copies selected text, sends it to Claude for grammar/clarity improvements, and pastes the result back — all without leaving the current app.

## Commands

```bash
bun run dev        # Launch dev build (frontend + Rust backend)
bun run build      # Production build (macOS .app bundle)
bun run tauri      # Run tauri CLI directly
```

No test suite exists yet. Rust code can be checked with `cargo check` from `src-tauri/`.

## Architecture

**Tauri 2 app**: Rust backend + vanilla JS/HTML frontend (Tailwind via CDN).

### Backend (`src-tauri/src/`)
- **`lib.rs`** — App initialization: registers Tauri plugins (clipboard, shell, global-shortcut, log), sets up tray icon (Settings/Quit), registers Cmd+Shift+K hotkey, exposes IPC commands (`get_config`, `save_config`)
- **`hotkey.rs`** — Core flow: saves clipboard → simulates Cmd+C → reads selected text → calls AI → writes result to clipboard → simulates Cmd+V → restores original clipboard. Uses `enigo` for key simulation.
- **`ai.rs`** — POST to `https://api.anthropic.com/v1/messages` with reqwest. Sends model, system prompt, and user text. Parses response JSON.
- **`config.rs`** — Reads/writes `~/.config/fixmywording/config.json` (API key, model, system prompt). Returns defaults if missing.

### Frontend (`src/`)
- **`index.html`** — Settings UI: API key input, model selector (Sonnet/Haiku), custom system prompt textarea
- **`main.js`** — Loads config on startup, auto-saves with 500ms debounce via Tauri `invoke()` IPC

### Key details
- App runs as `LSUIElement` (menu bar only, no dock icon)
- Window starts hidden; only shown when "Settings" is clicked from tray
- Config stored at `~/.config/fixmywording/config.json`
- Logs at `~/Library/Logs/com.fixmywording.dev/fixmywording.log`
- CSP is currently disabled (`null` in tauri.conf.json)
- Identifier: `com.fixmywording.dev`

## NEVER DO THIS

- **NEVER run `lsregister -kill`** or any variant of `/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -kill -r -domain local -domain system -domain user`. This destroys macOS Launch Services state and breaks the system.
- Never launch the app — the user handles launching themselves.
- Never run destructive macOS system commands to "fix" caching issues.
