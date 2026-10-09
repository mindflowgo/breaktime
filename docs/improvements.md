# Breaktime — Gap Analysis & Improvement Plan

> **Status (2026-10-08): Implemented.** All items below were applied on branch
> `fix/architecture-gaps` — see the git diff for this commit. The single-owner
> timer design (item 1) landed as a master/non-master window split in
> `timerManager.svelte.ts`; the long-term "timers in Rust" variant remains a
> future option. ARCHITECTURE.md has been updated to describe the fixed system.

Generated from a review of `ARCHITECTURE.md` against the actual codebase (Tauri v2 Rust backend + SvelteKit 5 frontend).

**How to build / verify while working:**

```bash
bun install                      # root deps
bun run tauri:dev                # full app (frontend build + cargo dev)
bun run check                    # svelte-check (app/)
cd tauri && cargo check          # Rust compile check
```

Fixes are grouped by priority. Each item lists exact locations, why it matters, and a concrete fix. Do not refactor beyond what an item describes unless noted.

---

## P0 — Correctness bugs (fix first)

### 1. Both webview windows run independent timer loops → double sound, double stats, races

**Location:** `app/src/lib/services/timerManager.svelte.ts` (`init()`, lines ~47–88, the `setInterval` at line ~56), `app/src/lib/components/SettingsView.svelte` (`onMount`, lines ~29–32), `tauri/tauri.conf.json` (the `main` window has no `url`, so it loads `/` at startup).

**Problem:** `tauri.conf.json` creates the `main` window with `"visible": false` but the webview still loads route `/` at startup. Route `/` renders `SettingsView`, whose `onMount` calls `timerManager.init()` — which starts a full 1-second `setInterval` countdown loop, idle-monitor startup, and event listeners. The `overlay` window does the same. Each Tauri webview window has its own JavaScript context, so `export const timerManager = new TimerManager()` produces **two unrelated singletons**, both counting down.

Consequences (all reachable today):

- **Double sound on every break.** When the overlay's countdown hits 0 it emits `breaktime:trigger-break-event`; the hidden main window's handler (`timerManager.svelte.ts` line ~79) calls `triggerBreak(type, false)` which calls `playSound(...)` in the main window's AudioContext. Two AudioContexts play the same chime overlapping (phasey/doubled audio). The same happens for tray "Take Eye Break Now": `app.emit("trigger-break")` from `native.rs` (line ~106) is delivered to **both** windows, and both call `triggerBreak`.
- **Double stats recording (race).** `tick()` calls `this.completeBreak()` when each window's *own* `breakTimeRemaining` reaches 0 (`timerManager.svelte.ts` lines ~250–252). The two loops were initialized at slightly different times; if the second window's counter hits 0 before the first window's `breaktime:break-action` broadcast arrives, `recordBreak()` runs twice → `totalCompleted` incremented twice and two history events logged.
- **Double `set_overlay_size` invokes** and duplicated `plugin:idlemonitor|start` calls (see item 3).

**Fix (minimal, frontend-only):** make the overlay window the single owner of timer state. The settings window becomes a read-only mirror.

1. In `timerManager.init()`, detect the window label and skip everything that mutates state when not the overlay:

```ts
import { getCurrentWindow } from '@tauri-apps/api/window';

private get isOverlay(): boolean {
    try { return getCurrentWindow().label === 'overlay'; } catch { return false; }
}

async init() {
    if (this.initialized) return;
    this.initialized = true;
    await settingsStore.init();
    if (this.isOverlay) {
        this.resetTimers();
        this.timerInterval = setInterval(() => this.tick(), 1000);
        this.initIdleMonitor();
    }
    // Event listeners stay registered in both windows (they sync state),
    // but handlers must only mutate/run side effects in the owner window.
    ...
}
```

2. Guard side effects, not just the loop: `triggerBreak`, `completeBreak`, `skipBreak`, `finishActiveBreak` should early-return (except for local state updates used for rendering) when `!this.isOverlay`. The settings window's UI only needs `activeBreak`, countdown display, and stats — all of which arrive via the existing `breaktime:*` events.
3. For state the settings window currently reads from the live singleton (`timerManager.isIdle`, countdown pill in SettingsView's Inactivity tab), have the overlay broadcast a lightweight `breaktime:state-sync` payload (`{ isPaused, isIdle, eyeTimeRemaining, stretchTimeRemaining, activeBreak, breakTimeRemaining }`) once per tick (or on change); the settings store subscribes and updates a plain `$state` object.

**Fix (longer-term, recommended follow-up):** move countdown ownership into Rust (a tokio task holding `Instant` deadlines, emitting `break-started` / `break-ticked` / `break-expired` events). This survives webview timer throttling (see item 2) and keeps working if a webview is recreated. Implement after the minimal fix stabilizes.

**Verification:** start the app, wait for (or trigger via tray) an eye break; only one chime should be audible (check by ear or by adding a `console.log` in `playSound` per window via devtools). Complete a break; `~/Library/Application Support/com.breaktime.app/.settings.json` → `stats.totalCompleted` must increase by exactly 1 and `events` gains exactly one entry.

---

### 2. Timers drift and stall under webview throttling (counter decrement instead of deadlines)

**Location:** `timerManager.svelte.ts` `tick()` (lines ~229–283) and `breakTimeRemaining--` logic.

**Problem:** The countdown decrements a counter once per `setInterval(1000)` tick. `setInterval` in WKWebView is throttled for occluded/backgrounded windows and drifts (each tick fires late; accumulated drift can reach minutes per hour). The hidden `main` window is throttled hardest (this amplified item 1). Even the overlay can be throttled when its Space is not visible. Pausing/idle freeze also relies on the tick firing.

**Fix:** store absolute deadlines and derive remaining time from `Date.now()`:

```ts
eyeDeadline = $state<number | null>(null);      // epoch ms
eyeTimeRemaining = $state(20 * 60);             // kept for display

private armEye(fromRemaining?: number) {
    const remaining = fromRemaining ?? this.eyeTimeRemaining;
    this.eyeDeadline = Date.now() + remaining * 1000;
}

// in tick():
if (this.eyeDeadline !== null) {
    this.eyeTimeRemaining = Math.max(0, Math.round((this.eyeDeadline - Date.now()) / 1000));
    if (this.eyeTimeRemaining <= 0) { this.triggerBreak('eye'); return; }
}
```

On pause/idle: capture `eyeTimeRemaining` and set `eyeDeadline = null`. On resume: re-arm from the captured remaining. Apply the same pattern to `stretchTimeRemaining` and `breakTimeRemaining`. A throttled window now catches up correctly on the next tick instead of silently losing time.

**Verification:** in devtools console run something that blocks the thread ~10s inside the overlay window (or move the overlay to another Space for a minute with a 5s interval configured); on return, remaining time must jump to the true wall-clock value, not continue from where it froze.

---

### 3. Idle threshold changes never take effect; second `start` invocation always errors

**Location:** `timerManager.svelte.ts` `initIdleMonitor()` (lines ~90–106); plugin source: `tauri-plugin-idlemonitor-0.2.3/src/commands.rs`.

**Problem:**
- The monitor is started once at init with the then-current `idleThresholdSec`. The Inactivity settings slider mutates `idleThresholdSec`, but nothing restarts the monitor — the new threshold applies only after an app restart.
- The plugin's `start` command returns `Error::AlreadyRunning` if the monitor is already running (verified in plugin source). Because both windows call `start` (item 1), the second call already fails today (silently — `console.warn` only).
- Changing the threshold requires `stop` → `start`; there is no live reconfigure.

**Fix:**
1. After item 1, only the overlay calls `plugin:idlemonitor|start`.
2. Add a reactive watcher in the overlay (e.g., in `overlay/+page.svelte` `$effect` or inside the store) on `settingsStore.settings.idleThresholdSec` that applies the new threshold:

```ts
async function applyIdleThreshold(secs: number) {
    try {
        await tauriInvoke('plugin:idlemonitor|stop');
    } catch { /* not running yet — fine */ }
    try {
        await tauriInvoke('plugin:idlemonitor|start', {
            options: { idleThresholdSecs: secs }
        });
    } catch (err) {
        console.warn('[TimerManager] idlemonitor restart failed:', err);
    }
}
```

3. While touching this: `handleIdleStateChange` (lines ~108–124) computes away-duration from wall clock between events. The plugin's `system:idle` payload already carries `seconds` (verified: `IdlePayload { idle: bool, seconds: Option<u64> }`). On the `idle: false` transition, prefer `payload.seconds` when present for the away-duration comparison against `awayResetMin`.

**Verification:** set Idle Threshold to 1 min in settings, wait ~70s without input → HUD must show 💥/💤 Away without restarting the app.

---

### 4. Non-atomic, racy, high-frequency persistence (settings + window state)

**Location:**
- `tauri/src/settings.rs` `set_setting_value` (lines ~30–49) — read-file → merge → `std::fs::write`.
- `tauri/src/window_state.rs` `save_window_state` (lines ~132–141), called from the `Moved`/`Resized` handlers in `tauri/src/lib.rs` (lines ~85–119) on **every** event, i.e. many times per second during a drag; each save is also preceded by a full file read via `get_window_state`.
- `app/src/lib/services/settingsStore.svelte.ts` `updateSettings` (lines ~90–105) — invoked from slider `oninput`, i.e., a Tauri invoke + disk write + broadcast per pixel of slider movement.

**Problem:**
- A crash mid-write truncates/corrupts `.settings.json` / `.breaktime_window.json` (next launch silently falls back to defaults — user loses settings and stats).
- Read-modify-write without locking: two concurrent `set_setting` calls (settings key vs stats key, from two windows) can interleave and lose one update. (Mostly mitigated by item 1, but the file-level race remains.)
- Disk churn during drags is unnecessary.

**Fix (Rust, atomicity):** write to a temp file then rename (atomic on same volume, both macOS APFS and others):

```rust
fn atomic_write(path: &std::path::Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}
```

Use it in both `set_setting_value` and `save_window_state`.

**Fix (Rust, debounce window state):** in `lib.rs`'s `on_window_event` handler, don't save per event. Manage a `Mutex<Option<AppWindowState>>` of pending state, and a flag/timestamp; save at most every ~500 ms, plus a final flush on `RunEvent::Exit`/`WindowEvent::Destroyed`. Simplest workable version: store pending state in managed state on every `Moved`/`Resized`, and spawn a debounced task (`tauri::async_runtime::spawn` + `sleep(500ms)`, replacing any earlier pending flush via an epoch counter in the managed state).

**Fix (frontend, slider writes):** in `SettingsView.svelte`, sliders currently commit via `oninput` → `settingsStore.updateSettings`. Change to: update a local `$state` value on `oninput` (UI stays live), commit with `updateSettings` on `onchange` (fires on release). Apply to all sliders (eye interval/duration, stretch interval/duration, volume, idle threshold, away reset).

**Verification:** drag the overlay around continuously for 5 s; `fs_usage`/Activity Monitor should show a handful of writes, not hundreds. Kill -9 the app during a settings save; the JSON files must remain parseable on next launch.

---

## P1 — Robustness & UX

### 5. Break sound may not play at all (WKWebView autoplay policy)

**Location:** `app/src/lib/services/soundPlayer.ts` `getAudioContext()` (lines ~6–15).

**Problem:** `new AudioContext()` in a webview that has never received a user gesture starts in `"suspended"` state; `resume()` without a gesture can be rejected by WKWebView. The overlay window (`NonactivatingPanel`, `can_become_key_window: false`) may legitimately never receive a gesture before the first break → silent breaks, intermittently.

**Fix (frontend):** opportunistically unlock audio on the first pointer interaction in the overlay window:

```ts
// overlay/+page.svelte onMount (or app-level in the overlay layout)
const unlock = () => {
    void getAudioContext();   // exports a warmed-up ctx; resume() inside a
                              // user-gesture handler is allowed
    window.removeEventListener('pointerdown', unlock);
};
window.addEventListener('pointerdown', unlock);
```

Export `getAudioContext` from `soundPlayer.ts` for this. Additionally, make `playSound` detect a still-suspended context and retry `resume()` once before giving up, and surface failures (see item 11) instead of only logging.

**Fix (alternative/stronger):** synthesize the chimes in Rust (e.g., `rodio` with pre-rendered WAV assets) — sidesteps webview audio policy entirely and keeps working if the overlay webview is unresponsive. Only do this if the frontend unlock proves insufficient in practice.

**Verification:** launch the app and trigger a break via tray (no click inside the overlay first) — chime must play. This is somewhat environment-dependent; test both cold start and after interaction.

### 6. Auto-expired breaks are recorded as user "completed"

**Location:** `timerManager.svelte.ts` `tick()` → `this.completeBreak()` at lines ~250–252.

**Problem:** When the break countdown reaches 0 without any user action, `completeBreak()` records `action: 'completed'` with the full duration. ARCHITECTURE.md says stats track "the user action" — an ignored prompt is not a completion, and adherence stats are inflated for users who never look at the HUD.

**Fix:** introduce a third action:

- `app/src/lib/types.ts`: extend `export type BreakAction = 'completed' | 'skipped' | 'expired';` and add counters (`totalExpired`, `eyeExpired`, `stretchExpired`) to `BreakStats`/`DEFAULT_STATS` (default 0 — old persisted stats without the fields merge fine because of the `{...DEFAULT_STATS, ...loaded}` spread in `settingsStore.load`).
- `timerManager.tick()`: when `breakTimeRemaining` hits 0, call a new `expireBreak()` that records `action: 'expired'` and runs the same finish/reset path as `finishActiveBreak`.
- `SettingsView.svelte` stats tab: decide display policy — recommended: count `expired` separately from adherence numerator, or fold into skipped for the adherence percentage but show it distinctly in the history list ("Expired"). Whichever is chosen, document it in ARCHITECTURE.md.

**Verification:** trigger a break and don't touch it; after it elapses, stats show one `expired` event, and `totalCompleted` is unchanged.

### 7. Mixed-DPI multi-monitor clamping math is wrong

**Location:** `tauri/src/window_state.rs` `clamp_overlay_position` (lines ~50–77) and `clamp_main_position` (lines ~79–106).

**Problem:** Both functions divide each monitor's **physical** position and size by **that monitor's own** scale factor, then compare against a logical point that was computed with the **window's** scale factor. With mixed scale factors (e.g., built-in 2x next to an external 1x), a logical coordinate from one monitor does not live in the other monitor's "logical" space computed this way — the containment test gives false negatives (position rejected → HUD snaps to default) or accepts off-screen positions.

**Fix:** do the containment test entirely in physical pixels. Track physical coordinates in `AppWindowState` (or convert saved logical → physical using the scale factor of the monitor that contains the point, which requires the same test — simpler to just persist physical):

```rust
pub fn clamp_overlay_position_physical<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>, x: i32, y: i32, w: i32, h: i32,
) -> (i32, i32) {
    if let Ok(monitors) = app.available_monitors() {
        for mon in &monitors {
            let mp = mon.position(); let ms = mon.size();
            if x >= mp.x && x + w <= mp.x + ms.width as i32
                && y >= mp.y && y + h <= mp.y + ms.height as i32 {
                return (x, y);
            }
        }
        if let Some(first) = monitors.first() {
            let p = first.position();
            return (p.x + 60, p.y + 100);
        }
    }
    (60, 100)
}
```

Then call `set_position(tauri::Position::Physical(...))` and update the `Moved`/`Resized` handlers in `lib.rs` to store raw physical values (drop the per-event scale division there too — it has the same per-monitor-scale ambiguity). Update `enter_overlay_mode`/`exit_overlay_mode`/`sync_overlay_bounds` accordingly. This is a deliberate on-disk format change for `.breaktime_window.json` — either bump the file name (`.breaktime_window_v2.json`) or accept one-time reset to defaults.

**Verification:** on a 2x-Retina + 1x-external setup, drag the HUD to the external display, quit, relaunch — it reappears on the external at the same spot, not snapped to the default position.

### 8. Overlay resize grip is ephemeral; custom size silently lost on restart

**Location:** `overlay/+page.svelte` `handleResizePointerDown` (lines ~103–155); `tauri/src/window_state.rs` `set_overlay_size` (lines ~260–269) and the setup path in `tauri/src/lib.rs` (lines ~69–71); `Resized` handler in `lib.rs` (lines ~104–117) only persists size for `label == "main"`.

**Problem:** The UI offers a drag-to-resize grip, but `set_overlay_size` never persists the size, `Resized` events for the overlay are ignored, and startup force-resets the overlay to `IDLE_HUD_WIDTH × IDLE_HUD_HEIGHT`. The feature resets every launch — either persist it or remove it.

**Fix (persist):** in the `Resized` handler, also store overlay size when `window.label() == "overlay"` (guard against transient menu/break expansions: only persist when `logical_w == IDLE_HUD_WIDTH && logical_h == IDLE_HUD_HEIGHT` is false but the window is in idle state — simplest: persist only sizes that survive 2 s, or persist on `WindowEvent::Destroyed`). In `lib.rs` startup and `enter_overlay_mode`, use `state.minimalist.width/height` instead of the constants. Remove the `safe_w/safe_h` minimum clamp in `set_overlay_size` if custom smaller sizes are intended to persist (keep a sane floor like 200×40).

**Fix (remove):** alternatively delete the grip from `overlay/+page.svelte` — but persisting is the better match for existing UX.

**Verification:** resize HUD, quit, relaunch — size persists.

### 9. Tray "Toggle Visible" bypasses window-mode state

**Location:** `tauri/src/native.rs` tray handler `"toggle-overlay"` (lines ~92–97).

**Problem:** The tray item shows the overlay but, when the app is in "normal" (settings) mode, leaves `main` visible and `state.mode == "normal"` in `.breaktime_window.json`. The app is then in a hybrid state the frontend state machine doesn't model (both windows visible, mode says normal). `enter_overlay_mode` exists and does the right thing but is only invoked from the frontend.

**Fix:** extract the body of `enter_overlay_mode` (`window_state.rs` lines ~149–193) into a plain helper `pub fn do_enter_overlay_mode<R: Runtime>(app: &AppHandle<R>) -> Result<AppWindowState, String>` (same for `exit_overlay_mode`), have the `#[tauri::command]` wrappers call it, and call `do_enter_overlay_mode` from the tray handler instead of `overlay.show()/set_focus()`. Rename the menu item id/label to `show-overlay`/"Show Overlay" or make it a true toggle using `state.mode`.

**Verification:** open Settings (normal mode), click tray "Show Overlay" → main hides, overlay appears, `.breaktime_window.json` `mode` becomes `"minimalist"`.

### 10. Overlay dropdown menu doesn't close when the user clicks away to another app

**Location:** `overlay/+page.svelte` `handleClickOutside` (lines ~35–47).

**Problem:** Outside-click detection listens for `click` inside the webview only. The overlay is a non-activating panel; clicking any other app never delivers a click/blur DOM event the handler can see, so the expanded menu (and its expanded 210 px window height) stays open indefinitely.

**Fix:** additionally subscribe to native focus loss:

```ts
import { getCurrentWindow } from '@tauri-apps/api/window';
// in onMount:
const unlistenFocus = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (!focused) void closeMenu();
});
// return () => { unlistenFocus(); ... }
```

Test whether WKWebView delivers `onFocusChanged` for a non-activating panel; if unreliable, fall back to closing the menu on a timer (e.g., 10 s) and on any break/mode transition.

**Verification:** open the ⋮ menu, click into another app → menu closes and the HUD shrinks back to 56 px.

### 11. All failures are silent (`console.warn` only)

**Location:** `windowManager.ts` `tauriInvoke` (lines ~14–22) swallows every error and returns `null`; `settingsStore.load()`'s `try/catch` around `tauriInvoke` is consequently dead code (it never rejects); no user-visible error surface exists anywhere.

**Problem:** If a permission is missing or a command fails (e.g., stats save failing), the app degrades silently — settings appear to save but don't persist, idle detection dies at startup, etc.

**Fix:**
- Keep `tauriInvoke` non-throwing for optional calls, but add a lightweight toast/status mechanism for failures that matter: settings save (`set_setting`), idle monitor start, overlay resize. Minimum viable: a small `$state` error string in `settingsStore` ("⚠️ Settings could not be saved") rendered in SettingsView's header; in the overlay, fall back to `statusText` for a few seconds.
- Remove the dead `try/catch` in `load()` or make it meaningful (e.g., distinguish "no settings file" from "invoke failed").

**Verification:** temporarily rename the `get_setting` command in Rust → the settings window shows the warning instead of silently using defaults.

---

## P2 — Dead code & cleanup

### 12. Unused Tauri commands and events

**Location:** `tauri/src/lib.rs` invoke handler (lines ~121–132); `tauri/src/native.rs` `setup_event_listeners` (lines ~125–149).

Verified by grep — the frontend never references these:
- `window_state::sync_overlay_bounds` — registered command, never invoked.
- `window_state::set_window_always_on_top` — registered command, never invoked.
- `"break-triggered"` event — emitted by the Rust `trigger-break` listener, never listened to anywhere. The listener's only useful side effect is showing/focusing the overlay; the re-emit is dead.

**Fix:** delete `sync_overlay_bounds` and `set_window_always_on_top` (functions + `generate_handler!` entries), and drop the `"break-triggered"` re-emit (keep the overlay show/focus). If item 1's long-term Rust-owns-timers refactor happens, revisit.

### 13. Duplicate/unreachable route: `/settings`

**Location:** `app/src/routes/settings/+page.svelte` is byte-identical to `app/src/routes/+page.svelte` (both render `<SettingsView />`); `tauri.conf.json`'s main window loads `/` by default and nothing ever navigates to `/settings`.

**Fix:** delete `app/src/routes/settings/`. Keep `/` (main window) and `/overlay`.

### 14. Unused plugins: `dialog` and `opener`

**Location:** `tauri/Cargo.toml` (lines ~18–19), `tauri/src/lib.rs` (lines ~17–18), `tauri/capabilities/default.json` (`dialog:default`, `opener:default`), `app/package.json` (`@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-opener`).

**Problem:** No code uses dialog or opener. `opener:default` grants URL/path opening from any window — unused capability surface.

**Fix:** remove the two Rust plugin registrations + Cargo deps, the two JS deps, and the two capability permissions.

### 15. Event listener leak on re-init/navigation

**Location:** `windowManager.ts` `listenTauriEvent` (lines ~34–62) correctly returns an unlisten function, but every caller in `timerManager.init()` / `settingsStore.init()` discards it. SvelteKit client-side navigation or HMR re-runs `onMount` → duplicate listeners.

**Fix:** store the unlisten functions (e.g., `private unlisteners: (() => void)[] = []` in both managers) and, in `overlay/+page.svelte` and `SettingsView.svelte`, return cleanup from `onMount` that tears them down (add a `destroy()` method to the managers). The `initialized` guard in `init()` should be reset by `destroy()`.

### 16. Duplicate `IDLE_HUD_*` constants in two languages

**Location:** `tauri/src/window_state.rs` (lines ~5–6: `300.0` / `56.0`) and `app/src/lib/services/windowManager.ts` (lines ~5–6: `300` / `56`).

**Problem:** They must stay in sync manually; a drift breaks size restore (the frontend would shrink to a size Rust immediately re-expands, or vice versa).

**Fix:** expose them from Rust via a `#[tauri::command] fn get_hud_metrics() -> { width: f64, height: f64 }` and have the frontend use the returned values after init (keep the TS constants only as pre-hydration fallback), or add a comment in both files pointing at each other and a unit assert. Prefer the command if item 1's Rust-ownership refactor lands.

### 17. Minor code quality

- `timerManager.statusText` getter (lines ~293–313) is unused — the overlay builds its own status markup. Delete or use it.
- `settingsStore.load()`'s `try/catch` around `tauriInvoke` is unreachable (see item 11).
- `overlay/+page.svelte` `openMenu` uses `MENU_EXPANDED_HEIGHT = 210` and break-expansion sizes `340×96` / `360×130` are hardcoded in `timerManager.triggerBreak` — collect these in one exported constants object (e.g., in `types.ts`) and document them in ARCHITECTURE.md.
- `.gitignore` lists `node_modules` twice (lines 3 and 9). `tauri/gen` is ignored but `gen/schemas` files exist in the repo — decide whether to keep the schemas (they're regenerable; if kept, remove the ignore entry, otherwise `git rm -r tauri/gen`).
- Note: `git status` shows the previously committed `app/build/*` artifacts staged for deletion — finish that cleanup with a commit so builds aren't tracked.

---

## P3 — Documentation & hygiene

### 18. ARCHITECTURE.md inaccuracies to fix in the doc itself

1. **Stretch interval contradiction:** the overview says "Every 40 minutes" (line 9) while the settings table (line 37) and the code default (`types.ts` `stretchIntervalMin: 45`) say 45 minutes. Standardize on **45**.
2. **tauri-nspanel dependency:** the doc specifies the git dependency `tauri-nspanel = { git = "https://github.com/ahkohd/tauri-nspanel", branch = "v2.1" }`, but `tauri/Cargo.toml` (line 25) uses `path = "../../tauri-nspanel"` — a machine-local clone outside the repo. The project does not build on any machine without that checkout. **Pick one:** switch Cargo.toml to the git dep (reproducible, matches doc), or vendor the crate into the repo and update the doc. Recommend the git dep; verify the overlay panel API (`tauri_panel!`, `WebviewWindowExt`) is unchanged on branch `v2.1`.
3. **"Volume and repeat behavior are fully configurable"** (line 51): repeat behavior does not exist — no setting, sounds play once. Either remove the claim or implement `soundRepeat: number` (1–5) in `Settings` + `SOUND_PRESETS` plumbing + a repeat loop in `playSound` (schedule the phrase again with a `setTimeout`/WebAudio start-time offset of phrase length + 1.5 s gap).
4. **Missing sections the code now depends on:** add to ARCHITECTURE.md —
   - The cross-window event catalog: `breaktime:settings-changed`, `breaktime:stats-changed`, `breaktime:break-action`, `breaktime:trigger-break-event`, `breaktime:pause-event`, plus Rust→JS `trigger-break`, `toggle-timers`, `open-settings`, `system:idle` — who emits, who consumes.
   - Timer ownership rule (after item 1: overlay window owns countdown; settings window is a mirror).
   - Persistence layout: `~/Library/Application Support/com.breaktime.app/.settings.json` (keys `settings`, `stats`) and `.breaktime_window.json` (schema of `AppWindowState`).
   - Stats schema (`BreakStats`/`BreakEvent`, 100-event cap) and the `expired` action (after item 6).
   - HUD size constants and break/menu expansion sizes (see item 17).

### 19. Project hygiene

- **README.md** contains only `# breaktime`. Add: what the app is, prerequisites (bun, Rust toolchain, the local `tauri-nspanel` note until item 18.2 is resolved), dev/build/package commands (`bun run tauri:dev`, `bun run tauri:build`), where settings live.
- **No tests / no CI.** Minimum bar: a GitHub Actions workflow (or local script) running `bun run check` (svelte-check), `cd tauri && cargo check`/`cargo clippy -- -D warnings`, and `cargo test`. Add pure-logic unit tests where they pay off immediately: timer deadline math (item 2), stats aggregation in `settingsStore.recordBreak`, and Rust `clamp_*_position` (item 7) — all are pure enough to test after modest extraction.
- **CSP is disabled** (`tauri.conf.json` `"csp": null`). The app loads only local content and synthesizes audio, so a strict policy is cheap:

  ```json
  "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src ipc: http://ipc.localhost"
  ```

  (`style-src 'unsafe-inline'` is needed for Svelte inline `style:` attributes; verify the overlay and settings pages both still render and `invoke` still works — `connect-src ipc:` is required for Tauri IPC on some platforms.)

- **App icons** are the unmodified Tauri defaults (`tauri/icons/*`). Replace with a Breaktime icon eventually; low priority.
- **Trigger-over-active-break edge case:** tray "Take Eye Break Now" while a stretch break is active calls `triggerBreak('eye')`, which overwrites `activeBreak` without recording the interrupted stretch or resetting its interval (the stretch timer is left at 0 → it will re-fire immediately after the eye break finishes). Fix alongside item 1: in `triggerBreak`, if another break is active, finish it first via `expireBreak()` (item 6) or queue the new one.
- **Dock & Taskbar Unobtrusiveness (Accessory mode / skipTaskbar):**
  - **macOS Dock:** Configured `app.set_activation_policy(tauri::ActivationPolicy::Accessory)` in `tauri/src/lib.rs` (matching `llm_gauge`), ensuring Breaktime operates strictly as a background menu bar helper without appearing in the macOS Dock or App Switcher (`Cmd+Tab`).
  - **Windows / Linux Taskbar:** Verified `skipTaskbar: true` is configured on the floating overlay in `tauri/tauri.conf.json`. For the `main` window (settings & stats), verify whether `skipTaskbar: true` should also be set if settings is meant to remain taskbar-free, or keep taskbar presence only when the settings window is actively opened.


---

## Suggested order of work

1. Items 1–2 (single-owner timers + deadline-based countdown) — together, they remove the loudest bugs.
2. Item 4 (atomic + debounced persistence) and item 3 (idle threshold restart) — small, independent.
3. Items 5–11 individually, in any order.
4. Items 12–17 as one cleanup commit (`cargo check` + `bun run check` must stay green).
5. Items 18–19 (docs, CI, CSP) last, updating ARCHITECTURE.md to describe the system as fixed — not as it was.
