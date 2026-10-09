# Overview

This is a small app to notify the user when to take screen-breaks, and exercise. The UI is a very lightweight floating HUD (matching the borderless style of ../llm_gauge and ../brAInstorm); opening settings expands it into a full window.

Every 20 minutes it triggers an 👁️ **Eye Reset** (default 20-second guided break) and every 45 minutes a 🧍 **Move Break** (default 60 seconds). Both interval and duration are customizable.

## The HUD (overlay window)

A compact always-on-top pill (macOS `NSPanel` via `tauri-nspanel`), borderless and draggable anywhere (click-drag anywhere except buttons; a tap without drag counts as a click).

**2× Size:** the ⋮ menu has a "2× Size" toggle (`settings.hud2x`). Normal mode is a 104×48 window; 2× mode doubles the window (208×96), the eye, the figures and the text together. Break-card windows scale the same way.

**Layout: `[eye (time under it) | person (stretch time under it) / skip]`**

- **Center:** the `EyeTimerSvg` eye, sized to fit the window (48×32 at 1×; 96×63 at 2×), with the **time directly below it, tucked just under the bottom curve of the eye** — next-eye-break countdown when idle, or the active break countdown. Colors: emerald (eye), amber (stretch), indigo (away), grey (paused/off).
- **Eyelid = progress:** on idle, the **upper eyelid droops as the eye interval elapses** — 25% closed at 75% time left, 50% at half, fully closed the moment the break fires; after the break the lid reopens (timer re-armed). During an eye break the **eye blinks twice per second** for the whole break duration, with a warm-orange countdown ring around the iris. When away, the eye is replaced by the zzz figure (`static/zzz-away.svg`).
- **Right:** a subtle grey standing person (`static/stand.svg`) with the **stretch countdown directly below it**. During any break it is replaced by the **skip button**. Clicking the person while idle starts a stretch break immediately.

**Stretch breaks:** the eye is switched out for a **larger stretching figure** (`StretchFigure.svelte`) with the countdown ring around it; the break countdown time stays under the figure.

**Transparency:** the HUD renders at `settings.overlayOpacity` (0.2–1.0, default 0.85, adjustable in Settings → Reminders → HUD Overlay). It switches to **100% opacity while the mouse is over it** and **during any break** — a prompt must never be missed because of translucency. (Native mouse-enter/exit tracking areas on macOS emit `overlay-entered`/`overlay-exited`.)

Clicking the eye: idle → start an eye break; during a break → complete it. `Esc` skips, `Enter` completes. The ⋮ menu holds Settings, Stats Tracker, break-now actions (with live durations), Pause/Resume and Quit.

## Break lifecycle & sounds

Each break: **start sound** → guided countdown (blinking eye / stretch figure) → user completes (✓/click/Enter), skips (✕/Esc), or lets it expire → **end sound**.

- The end-of-break sound is the **same preset phrase as the start chime followed by an ascending triumphant flourish** (C5→E5→G5→C6 bell sparkle) so "break done" is audibly distinct from "break start". It plays on *completed* and *expired*, not on *skip*.
- All sounds are synthesized (Web Audio, no assets). Volume, fade-in and repeat (1–5×) are configurable; six calm presets (soft bell, gentle chime, calm drop, calm piano, morning dew, still water).
- The overlay's `AudioContext` is unlocked on the first pointer gesture (WKWebView keeps it suspended otherwise).

Stats record `completed` / `skipped` / `expired` per type — expired prompts count against adherence. Away sessions record one event on return with the measured duration.

## Timer ownership (important)

Two webview windows exist: `main` (settings/stats, hidden in overlay mode) and `overlay` (the HUD). **The overlay window is the single owner ("master") of all timer state.** The hidden main window never runs its own countdown or plays sounds.

- `app/src/lib/services/windowManager.ts` → `isMasterWindow()` decides ownership.
- `timerManager.svelte.ts` runs the authoritative deadline-based countdown (`Date.now()` targets, drift-free under timer throttling) only when master; non-master windows mirror state via `breaktime:state-sync` and forward intents via `breaktime:command`.
- Break claims are synchronous (`claimActiveBreak`) before any `await`, so double-clicks or a slow stats write cannot double-record.
- `triggerBreak` enters overlay mode first, so natural timer breaks are never invisible while the settings window is open.

## Idle / away behavior

`tauri-plugin-idlemonitor` emits `system:idle` (`{ idle, seconds? }`). On idle: timers freeze, HUD shows 💤. On return, the measured `seconds` drive the away logic: away ≥ `awayResetMin` (default 10 min) resets all timers; shorter absences resume where they left off. A system-sleep gap ≥ `awayResetMin` (detected via tick-gap) also resets. Changing the idle threshold restarts the monitor (stop → start; `start` errors with `AlreadyRunning` if already running).

## Event catalog

All names live in `app/src/lib/services/events.ts`. Cross-window payloads are wrapped in `{ sender, data }` envelopes — senders ignore their own echoes.

Rust → JS: `trigger-break` ("eye"|"stretch"), `reset-timers`, `reset-timer` ("eye"|"stretch"), `system:idle`, `overlay-entered` / `overlay-exited` (macOS mouse tracking).

JS ↔ JS: `breaktime:state-sync` (master → mirrors, timestamped; mirrors drop stale), `breaktime:command` (mirrors → master), `breaktime:query-state` (mirror asks for sync; retried 3× at startup), `breaktime:settings-changed`, `breaktime:stats-changed`, `breaktime:stats-reset`, `breaktime:open-tab`.

## Persistence

In the app data dir, all writes atomic (unique temp file + rename):

- **`.settings.json`** — `{ "settings": Settings, "stats": BreakStats }`. Settings are normalized on load (`normalizeSettings`: clamps numbers, validates the sound preset — corrupt disk values can't leak NaN into countdowns). Stats keep the last 100 events.
- **`.breaktime_window.json`** — `AppWindowState { mode, normal, minimalist }`. Positions are **physical pixels** (mixed-DPI safe; clamping via `clamp_rect` in physical space using the containing monitor's scale); sizes are logical points. State is cached in a managed `Mutex` (no guard held across window roundtrips — the macOS panel delegate fires synchronously inside `set_position`), persisted at most every 500 ms during drags and flushed on exit.

Window modes: `minimalist` (HUD visible) ⇄ `normal` (settings window). Mode switches are idempotent — re-entering minimalist mode only ensures the panel is shown; it never resets the HUD size or position mid-break.

## Tech stack

- Frontend: SvelteKit 5 (static adapter, prerendered `/` and `/overlay`), Tailwind, vitest.
- Backend: Tauri v2 (`tray-icon`, `image-png`, `devtools`), `tauri-plugin-idlemonitor = "0.2.3"`, `tauri-nspanel = "2.1.1"` (crates.io), no `window-vibrancy` (the HUD is fully transparent CSS over an NSPanel).
- macOS: accessory activation policy (menu-bar app, no dock icon), template tray icon.
- CI: `.github/workflows/ci.yml` — svelte-check, vitest, vite build, `cargo clippy -D warnings`, `cargo test`.

Reference code from earlier projects lives in `/tauri-code`.

## Directory structure

```
/                                root package.json (scripts: check/test/build/tauri:dev/tauri:build)
/tauri                           Tauri v2 Rust app (lib.rs, native.rs, settings.rs, window_state.rs)
/app                            SvelteKit app
  src/lib/services/             timerManager, settingsStore, soundPlayer, windowManager, breakLogic (+tests), events
  src/lib/components/           EyeTimerSvg, StretchFigure, SettingSlider, Toggle, SettingsView, settings/*Tab
  src/routes/overlay            the HUD page
  static/                       stand.svg, zzz-away.svg
```
