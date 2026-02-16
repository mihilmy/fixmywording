# Fix My Wording

Select text, hit a hotkey, and get instant grammar and clarity improvements — all without leaving your current app.

Fix My Wording is a lightweight desktop app that lives in your system tray. It uses the Anthropic Claude API to rewrite selected text in-place: copy, improve, paste — in under a second.

## How It Works

1. Select any text in any app
2. Press `Cmd+Shift+K` (macOS) or `Ctrl+Shift+K` (Windows/Linux)
3. The selected text is replaced with an improved version

## Features

- **Global hotkey** — works in any app, any text field
- **In-place replacement** — improved text is pasted right where you selected it
- **System tray** — runs quietly in the background, zero UI clutter
- **Customizable prompt** — control how your text is rewritten (tone, formality, style)
- **Model selection** — choose between Claude Sonnet and Haiku
- **Cross-platform** — macOS, Windows, and Linux (built with Tauri 2)
- **Privacy-first** — your API key and config stay local on your machine

## Installation

### Prerequisites

- [Bun](https://bun.sh) (or Node.js)
- [Rust](https://rustup.rs)
- [Anthropic API key](https://console.claude.com/)

### Build from Source

```bash
git clone https://github.com/mihilmy/fixmywording.git
cd fixmywording
bun install
bun run build
```

The built app will be in `src-tauri/target/release/bundle/`.

### Development

```bash
bun run dev
```

## Setup

1. Launch the app — it appears in your system tray (no dock icon on macOS)
2. Click the tray icon → **Settings**
3. Enter your Anthropic API key
4. Optionally change the model or customize the system prompt
5. Close the settings window and start using `Cmd+Shift+K`

### macOS Permissions

On macOS, you'll need to grant **Accessibility** permission for Fix My Wording to simulate keyboard shortcuts (copy/paste). Go to **System Settings → Privacy & Security → Accessibility** and enable the app.

## Architecture

Tauri 2 app with a Rust backend and vanilla JS frontend.

```
src-tauri/src/
  lib.rs      — App setup: tray icon, global shortcut, IPC commands
  hotkey.rs   — Core flow: copy → AI → paste, with clipboard preservation
  ai.rs       — Anthropic API client
  config.rs   — Config read/write (~/.config/fixmywording/config.json)

src/
  index.html  — Settings UI (Tailwind CSS)
  main.js     — Settings logic with auto-save
```

## License

MIT
