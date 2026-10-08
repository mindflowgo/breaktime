# Breaktime

A lightweight macOS menu-bar-style app that reminds you to take screen breaks:
a 👁️ **Eye Reset** every 20 minutes (20-second guided routine with 10 counted
blinks) and a 🧍 **Move Break** every 45 minutes (60-second guided stretch
routine). The UI is a small floating HUD overlay; opening settings expands it
into a full window with reminders, sounds, inactivity and stats tabs.

## Tech stack

- **Frontend:** SvelteKit 5 + Tailwind (static adapter, no SSR) in `app/`
- **Backend:** Tauri v2 (Rust) in `tauri/`
- **Plugins:** `tauri-plugin-idlemonitor` (system idle detection),
  `tauri-nspanel` + `window-vibrancy` (macOS floating overlay panel)

## Development

Prerequisites: [bun](https://bun.sh), a Rust toolchain, and macOS.

```bash
bun install          # root deps
cd app && bun install && cd ..   # frontend deps

bun run tauri:dev    # build frontend + run the desktop app in dev mode
bun run check        # svelte-check (type/lint the frontend)
bun run test         # vitest unit tests (frontend)
bun run build        # build the frontend only

cd tauri
cargo check          # Rust compile check
cargo test           # Rust unit tests
cargo clippy -- -D warnings
```

Packaging: `bun run tauri:build` produces the app bundle from `tauri/target/`.

## Where data lives

- Settings & stats: `~/Library/Application Support/com.breaktime.app/.settings.json`
  (keys `settings` and `stats`)
- Window state: `~/Library/Application Support/com.breaktime.app/.breaktime_window.json`
  (positions are stored in **physical** pixels; sizes in logical points)

See `ARCHITECTURE.md` for the full design, event catalog and persistence
schemas.
