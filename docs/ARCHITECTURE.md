# Overview

This is a small app to notify the user when to take screen-breaks, and exercise. The UI is a very lightweight floating HUD (matching the borderless style of ../llm_gauge and ../brAInstorm); opening settings expands it into a full window.

Every 20 minutes it triggers an 👁️ **Eye Reset** (default 20-second guided break) and every 45 minutes a 🧍 **Move Break** (default 60 seconds). Both interval and duration are customizable.

## The HUD (overlay window)

A compact always-on-top pill (macOS `NSPanel` via `tauri-nspanel`), borderless and draggable anywhere (click-drag anywhere except buttons; a tap without drag counts as a click).

**2× Size:** the ⋮ menu has a "2× Size" toggle (`settings.hud2x`). Normal mode is a 104×48 window; 2× mode doubles the eye, figures and text in a hand-tuned 149×69 window (the countdown time tucks up onto the eye's base). Break-card windows scale the same way. Rust reads `settings.hud2x` at startup and on mode switches (`idle_hud_logical_size`) so the window is already at the right scale before the frontend loads, and `overlay-mode-changed` events let the HUD re-apply its idle size as a safety net.

**Layout: `[eye (time under it) | person (stretch time under it) / skip]`**

- **Center:** the `EyeTimerSvg` eye, sized to fit the window (48×32 at 1×; 96×63 at 2×), with the **time directly below it, tucked just under the bottom curve of the eye** — next-eye-break countdown when idle, or the active break countdown. Colors: emerald (eye), amber (stretch), indigo (away), grey (paused/off).
- **Eyelid = progress:** on idle, the **upper eyelid droops as the eye interval elapses** — 25% closed at 75% time left, 50% at half, fully closed the moment the break fires; after the break the lid reopens (timer re-armed). During an eye break the **eye blinks twice per second** for the whole break duration, with a warm-orange countdown ring around the iris. When away, the eye is replaced by the zzz figure (`static/zzz-away.svg`).
- **Right:** a subtle grey standing person (`static/stand.svg`) with the **stretch countdown directly below it**. During any break it is replaced by the **skip button**. Clicking the person while idle starts a stretch break immediately.

**Stretch breaks:** the eye is switched out for a **larger stretching figure** (`StretchFigure.svelte`) with the countdown ring around it; the break countdown time stays under the figure.

**Transparency:** the HUD's default opacity is `settings.overlayOpacity` (0.2–1.0, default 0.85, adjustable in Settings → Reminders → HUD Overlay). It switches to **100% while the mouse is over it** (with a subtle llm_gauge-style outline of the overlay bounds), **during any break**, and **while the settings/stats window is open**. Opacity is applied natively via the panel's `setAlphaValue` (`native_overlay_set_opacity`, matching llm_gauge) — CSS opacity on a transparent NSPanel's webview proved unreliable on macOS — and hover state comes from the native tracking-area events `overlay-entered`/`overlay-exited`.

**The eye and the person are drag regions only** — pressing them never starts, completes, or skips a break (so moving the HUD can never misfire). Breaks start exclusively from the timer firing or the ⋮ menu (Eye/Stretch Now); `Enter` completes the active break, `Esc` or the ✕ button skips it. The ⋮ menu holds Settings, Stats Tracker, break-now actions (with live durations), Pause/Resume, 2× Size and Quit. The **tray (menu-bar) menu has the same "2× HUD Size" checkbox** — it flips `settings.hud2x` on disk and broadcasts the settings change, so both toggles stay in sync wherever you use them.

**No snap-to-size:** the overlay window keeps its idle size for the whole session — the break UI renders inside it. Resizing the macOS panel mid-session produced a white outline flash on every break; only the menu (and the 2× Size toggle) resize the window.

## Break lifecycle & sounds

Each break: **start sound** → guided countdown (blinking eye / stretch figure) → user completes (✓/click/Enter), skips (✕/Esc), or lets it expire → **end sound**.

- The end-of-break sound is a **fixed gentle two-tone completion cue** — a short low "tum" (A4) resolving to a longer, warm, higher "tuuumm" (E5) — independent of the start chime (and of custom sounds), so "time's up" is instantly distinguishable. It plays whenever a break ends naturally or is completed — only an explicit *skip* stays silent.
- All sounds are synthesized (Web Audio, no assets). Volume, fade-in and repeat (1–5×) are configurable; six calm presets (soft bell, gentle chime, calm drop, calm piano, morning dew, still water) plus a **custom sound**: pick an mp3/wav/ogg/m4a/aac/flac in Settings → Sounds ("Choose file…"). The file is copied into the app data dir (`custom-sound.<ext>`) via `import_custom_sound`, the chosen file name is persisted as `settings.customSoundFile`, and playback (volume, fade-in, repeat) goes through an `<audio>` element served from `get_custom_sound_data` as a data URL.
- The overlay's `AudioContext` is unlocked on the first pointer gesture (WKWebView keeps it suspended otherwise).

Stats record `completed` / `skipped` per type. A break that runs to the end of its countdown with no interaction **counts as completed** — the user is assumed to have done the exercise unless they explicitly clicked Skip. Away sessions record one event on return with the measured duration. Legacy `expired` events from older builds are dropped on load.

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

- **`.settings.json`** — `{ "settings": Settings, "windowState": AppWindowState }`. Settings are normalized on load (`normalizeSettings`: clamps numbers, validates the sound preset — corrupt disk values can't leak NaN into countdowns). Includes `hud2x` (the persisted 2× HUD mode, which Rust also reads when sizing the window at startup) and `statsRetention`.
- **`stats.json`** — `{ "stats": BreakStats }` under the `stats` key, through the same `get_setting`/`set_setting` handler (key `stats` routes here, all other keys route to `.settings.json`). Keep the last 100 events; the **stats retention window** (Stats tab dropdown: 1 week / 1 month / 6 months / forever, `settings.statsRetention`) prunes older events and recomputes all counters from the survivors, on load and whenever the window is shortened.

**Window state** lives in `.settings.json` under the `windowState` key: `AppWindowState { mode, normal, minimalist }`. Positions are **physical pixels** (mixed-DPI safe; clamping via `clamp_rect` in physical space using the containing monitor's scale); sizes are logical points. State is cached in a managed `Mutex` (no guard held across window roundtrips — the macOS panel delegate fires synchronously inside `set_position`), persisted at most every 500 ms during drags and flushed on exit — via the same `set_setting_value` handler as settings and stats.

Window modes: `minimalist` (HUD visible) ⇄ `normal` (settings window). Mode switches are idempotent — re-entering minimalist mode only ensures the panel is shown; it never resets the HUD size or position mid-break.

## Tech stack

- Frontend: SvelteKit 5 (static adapter, prerendered `/` and `/overlay`), Tailwind, vitest.
- Backend: Tauri v2 (`tray-icon`, `image-png`, `devtools`), `tauri-plugin-idlemonitor = "0.2.3"`, `tauri-nspanel = "2.1.1"` (crates.io), no `window-vibrancy` (the HUD is fully transparent CSS over an NSPanel).
- macOS: accessory activation policy (menu-bar app, no dock icon), template tray icon, NSPanel overlay (`tauri-nspanel`) with tracking-area hover events and `setAlphaValue` opacity.
- Windows (x64): the overlay is a plain transparent always-on-top window (`transparent` + `alwaysOnTop` + `skipTaskbar` in `tauri.conf.json`) — no panel needed. Panel alpha is emulated with a `WS_EX_LAYERED` layered window + `SetLayeredWindowAttributes` (`windows` crate, `native_overlay_set_opacity`); hover comes from DOM pointer events (macOS-only unreliability motivated the native path there); dragging uses `start_dragging`, show/hide and always-on-top use the cross-platform Tauri APIs.
- CI: `.github/workflows/ci.yml` — svelte-check, vitest, vite build, `cargo clippy -D warnings` + `cargo test` on macOS, and `cargo check` + `cargo test` on Windows (x64).

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
