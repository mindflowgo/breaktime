# Overview

This is a small app to notify the user when to take screen-breaks, and exercise. The UI is very lightweight and takes on the simple clear overlay style done in ../brAInstorm.

It has this clean simple overlay UI, and when the user selects settings it expands to a larger window for editing settings. The settings UI is compact and cleanly/organized - not a AI-verbose spaced out 6-12px with lots of cards.

Every 20 minutes it shows a little (👁️) in the overlay and says Eye-Break (the user can click checkmark for when done or skip). This goes on for a 20 second countdown; at that point it sets the timer for another 20 minutes for the next reminder.

Every 45 minutes it reminds the user to Stretch (🧍). Again with checkmark when done or skip.

It tracks the times the prompts came up and the user action (completed / skipped / expired-ignored) in a stats tracker accessible from the same drop-menu that allows settings.

During the break, a guided animation encourages you to:
- look away from the screen
- shift your focus into the distance
- relax your eyes
- blink naturally
- briefly disconnect from close-up screen focus
- The default guided exercise includes 10 counted blinks over 20 seconds.

The guided routine for stretching encourages you to:

- stand up and walk for a moment
- stretch your arms
- roll your shoulders
- stretch your upper body
- perform simple squats
- briefly get away from your desk

The goal is not exercise performance. Four guided steps rotate across the break duration.

## Settings

| Reminder | Default interval | Default duration |
| --- | --- | --- |
| 👀 Eye Reset | 20 minutes | 20 seconds |
| 🧍 Move Break | 45 minutes | 60 seconds |

Every interval and duration can be customized. Sliders persist on release (not per-pixel).

### Sounds

🔔 Calm sounds, not alarms. Volume, fade-in and repeat behavior (1–5 repetitions) are fully configurable.

| Sound | Style |
| --- | --- |
| Soft bell | Short gentle alert |
| Gentle chime | Light notification |
| Calm drop | Soft minimal tone |
| Calm piano | Four-note piano phrase |
| Morning dew | Gentle rising five-note melody |
| Still water | Slower, deeper calming melody |

The softer melodies fade in gradually instead of starting suddenly at full volume. All sounds are synthesized with the Web Audio API (no audio assets).

## Timer ownership (important)

Two webview windows exist: `main` (settings/stats, hidden in overlay mode) and `overlay` (the HUD). **The overlay window is the single owner ("master") of all timer state.** The hidden main window must never run its own countdown or play sounds — otherwise every break chime plays twice and stats get double-recorded.

- `app/src/lib/services/windowManager.ts` → `isMasterWindow()` decides ownership.
- `app/src/lib/services/timerManager.svelte.ts` runs the authoritative countdown only when master; non-master windows mirror state from `breaktime:state-sync` and forward user intents as `breaktime:command`.
- Countdowns use absolute deadlines (`Date.now()`-based target timestamps), not per-tick decrements, so webview timer throttling cannot lose time.

## Idle / away behavior

`tauri-plugin-idlemonitor` emits `system:idle` (`{ idle: boolean, seconds?: number }`). When idle begins, timers freeze and the HUD shows 💤 Away. When activity resumes, the away duration is taken from the payload's `seconds` when present; if the user was away ≥ `awayResetMin` (default 10 min), all timers reset on return. Changing the idle threshold in settings restarts the monitor (stop → start; the plugin's `start` errors with `AlreadyRunning` if already running).

## Event catalog

Rust → JS (emitted with `app.emit`, received in all windows):

| Event | Payload | Purpose |
| --- | --- | --- |
| `trigger-break` | `"eye" \| "stretch"` | tray menu: start a break now (Rust also enters overlay mode first) |
| `toggle-timers` | `()` | tray menu: pause/resume |
| `open-settings` | `()` | tray menu: exit overlay mode, show main window |
| `system:idle` | `{ idle, seconds? }` | idle monitor state change |

JS → JS (master overlay ⇄ settings window):

| Event | Payload | Purpose |
| --- | --- | --- |
| `breaktime:state-sync` | `TimerStateSync` | master broadcasts full timer state each tick / on change |
| `breaktime:command` | `TimerCommand` | non-master windows forward trigger/complete/skip/pause/reset intents |
| `breaktime:query-state` | `()` | non-master asks master for an immediate state sync |
| `breaktime:settings-changed` | `Settings` | settings saved in one window, mirrored in the other |
| `breaktime:stats-changed` | `BreakStats` | stats updated in one window, mirrored in the other |
| `breaktime:stats-reset` | `BreakStats` | stats cleared |
| `breaktime:open-tab` | `TabId` | open the settings window on a specific tab |

(All types live in `app/src/lib/types.ts`.)

## Persistence

Both files live in the app data dir (`~/Library/Application Support/com.breaktime.app/`) and are written atomically (temp file + rename):

- **`.settings.json`** — `{ "settings": Settings, "stats": BreakStats }`. Settings include `soundRepeat` and an optional `hudSize` (custom compact HUD size from the resize grip, persisted across restarts).
- **`.breaktime_window.json`** — `AppWindowState { mode: "normal" | "minimalist", normal: WindowBounds, minimalist: WindowBounds }`. Positions are stored in **physical pixels** (mixed-DPI safe; clamping is done in physical space via `clamp_rect` in `tauri/src/window_state.rs`), sizes in logical points. Window-state writes during drags are throttled (max one write per 500 ms) and flushed on exit.

Stats schema (`BreakStats`): totals + per-type counters for `completed`, `skipped`, `expired`, plus the last 100 `BreakEvent`s (`{ id, timestamp, type, action, durationSec }`). Adherence = completed / (completed + skipped + expired).

## Window sizes

- Compact HUD: 300×56 logical (`IDLE_HUD_WIDTH/HEIGHT`, kept in sync between `tauri/src/window_state.rs` and `app/src/lib/types.ts`); user-resizable via the bottom-right grip, persisted as `settings.hudSize`.
- Eye break card: 360×116. Stretch break card: 380×150. Menu-expanded: 300×224. (Defined in `timerManager.svelte.ts` / the overlay page.)

## Tech Stack

- Frontend: SvelteKit 5 (static adapter, `ssr = false`, prerendered pages at `/` and `/overlay`)
- Backend: Tauri v2

Cargo plugins:

- `tauri-plugin-idlemonitor = "0.2.3"`
- `tauri-nspanel = { git = "https://github.com/ahkohd/tauri-nspanel", branch = "v2.1" }`
- `window-vibrancy = "0.6"`

Reference code from earlier projects lives in `/tauri-code` (HUD overlay, native bridge, settings and window-state patterns).

## Directory Structure

```
/                     root package.json (scripts)
/tauri                Tauri v2 Rust app (src: lib.rs, native.rs, settings.rs, window_state.rs)
/app                 SvelteKit 5 app (routes: / overlay HUD, / settings & stats)
/tauri-code          reference samples from earlier projects
```
