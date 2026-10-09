# Breaktime

A lightweight menu-bar-style app that reminds you to take screen breaks:
Shows an overlay eye that progressively closes until the break time happens, then 
plays a gentle sound to encourage breaking from the screen.

It also tracks physical stretch times so you can keep your body healthy inspite of 
computer time.

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
