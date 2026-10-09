/**
 * Single source of truth for every cross-window / cross-process event name.
 * A typo in a scattered string literal is invisible to the compiler; here the
 * compiler enforces every reference.
 */

/** Master (overlay) broadcasts authoritative timer state each tick. */
export const EV_STATE_SYNC = 'breaktime:state-sync';
/** Non-master windows forward user intents to the master. */
export const EV_COMMAND = 'breaktime:command';
/** Non-master asks the master for an immediate state sync. */
export const EV_QUERY_STATE = 'breaktime:query-state';
/** Settings saved in one window, mirrored in the other. */
export const EV_SETTINGS_CHANGED = 'breaktime:settings-changed';
/** Stats updated in one window, mirrored in the other. */
export const EV_STATS_CHANGED = 'breaktime:stats-changed';
/** Stats cleared. */
export const EV_STATS_RESET = 'breaktime:stats-reset';
/** Open the settings window on a specific tab. */
export const EV_OPEN_TAB = 'breaktime:open-tab';
/** Rust: tray — start a break now ("eye" | "stretch"). */
export const EV_TRIGGER_BREAK = 'trigger-break';
/** Rust: tray — reset all timers. */
export const EV_RESET_TIMERS = 'reset-timers';
/** Rust: tray — reset one timer ("eye" | "stretch"). */
export const EV_RESET_TIMER = 'reset-timer';
/** Rust: idlemonitor state change ({ idle, seconds? }). */
export const EV_SYSTEM_IDLE = 'system:idle';
/** Rust: window mode switched ("minimalist" | "normal"). */
export const EV_OVERLAY_MODE_CHANGED = 'overlay-mode-changed';
