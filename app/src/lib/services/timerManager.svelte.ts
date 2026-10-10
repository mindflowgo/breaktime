import { playBreakEnd, playSound } from '$lib/services/soundPlayer';
import { settingsStore } from '$lib/services/settingsStore.svelte';
import {
	emitTauriEvent,
	listenTauriEvent,
	isMasterWindow,
	isTauriEnvironment,
	enterOverlayMode,
	setOverlayWindowSize,
	tauriInvoke,
	IDLE_HUD_WIDTH,
	IDLE_HUD_HEIGHT
} from '$lib/services/windowManager';
import {
	EV_COMMAND,
	EV_QUERY_STATE,
	EV_RESET_TIMER,
	EV_RESET_TIMERS,
	EV_STATE_SYNC,
	EV_SYSTEM_IDLE,
	EV_TRIGGER_BREAK
} from '$lib/services/events';
import {
	DEFAULT_SETTINGS,
	type BreakType,
	type Settings,
	type TimerCommand,
	type TimerStateSync
} from '$lib/types';

/** Overlay window size while an eye break is active. */
export const EYE_HUD_WIDTH = 136;
export const EYE_HUD_HEIGHT = 48;
/** Overlay window size while a stretch break is active (larger figure). */
export const STRETCH_HUD_WIDTH = 136;
export const STRETCH_HUD_HEIGHT = 48;
/**
 * Idle HUD window size in 2× mode. The eye/figures still render at 2×
 * scale; the window is hand-tuned tighter around them (not a straight
 * 2× of 104×48) so the pill doesn't sprawl.
 */
export const IDLE_HUD_2X_WIDTH = 149;
export const IDLE_HUD_2X_HEIGHT = 69;

class TimerManager {
	// Master identity: only the overlay window owns countdown state.
	isMaster = $state(false);

	// Status
	isPaused = $state(false);
	isIdle = $state(false);
	activeBreak = $state<BreakType | null>(null);

	// Time remaining in seconds
	eyeTimeRemaining = $state(DEFAULT_SETTINGS.eyeIntervalMin * 60);
	stretchTimeRemaining = $state(DEFAULT_SETTINGS.stretchIntervalMin * 60);
	breakTimeRemaining = $state(0);

	// Timestamp targets for drift-free calculations
	eyeTargetTimestamp = $state<number | null>(null);
	stretchTargetTimestamp = $state<number | null>(null);
	breakTargetTimestamp = $state<number | null>(null);

	// Idle & Away tracking
	awayStartTime = $state<number | null>(null);

	private lastTickTime = Date.now();
	/** Highest state-sync timestamp seen; older mirrors are dropped. */
	private lastSyncTimestamp = 0;

	private timerInterval: ReturnType<typeof setInterval> | null = null;
	private electionTimeout: ReturnType<typeof setTimeout> | null = null;
	private initialized = false;
	/** Bumped by destroy(); late-resolving unlistens from an older generation self-remove. */
	private generation = 0;
	private unlistenFns: (() => void)[] = [];

	async init() {
		if (this.initialized) return;
		this.initialized = true;

		await settingsStore.init();

		this.isMaster = isMasterWindow();

		// Single settings channel: the store notifies for local changes and
		// (in the master) for cross-window changes after echo suppression.
		settingsStore.onSettingsChange((newSettings) => {
			if (this.isMaster) {
				this.handleSettingsChanged(newSettings);
			}
		});

		// Stats reset: non-master forwards the intent as a command; the master
		// acts on the command (avoiding a second path via the broadcast event).
		if (!this.isMaster) {
			settingsStore.onStatsReset(() => {
				this.resetTimers();
			});
		}

		if (this.isMaster) {
			await this.initMaster();
		} else {
			await this.initNonMaster();
		}
	}

	private trackUnlisten(pending: Promise<() => void>) {
		const gen = this.generation;
		pending.then((fn) => {
			if (gen === this.generation) {
				this.unlistenFns.push(fn);
			} else {
				try {
					fn();
				} catch {}
			}
		});
	}

	private async initMaster() {
		this.resetTimersInternal();

		// Start authoritative 1s tick loop
		this.lastTickTime = Date.now();
		if (this.timerInterval) clearInterval(this.timerInterval);
		this.timerInterval = setInterval(() => this.tick(), 1000);

		// Initialize native Tauri idle monitor
		await this.initIdleMonitor();

		// Commands from non-master windows
		this.trackUnlisten(
			listenTauriEvent<TimerCommand>(EV_COMMAND, async (cmd) => {
				if (this.isMaster && cmd) {
					await this.handleCommand(cmd);
				}
			})
		);

		// State sync requests from non-master windows
		this.trackUnlisten(
			listenTauriEvent(EV_QUERY_STATE, () => {
				if (this.isMaster) {
					this.emitStateSync();
				}
			})
		);

		// Tray menu events (emitted from Rust)
		this.trackUnlisten(
			listenTauriEvent<string>(EV_TRIGGER_BREAK, (type) => {
				if (this.isMaster && (type === 'eye' || type === 'stretch')) {
					void this.triggerBreak(type);
				}
			})
		);

		this.trackUnlisten(
			listenTauriEvent(EV_RESET_TIMERS, () => {
				if (this.isMaster) {
					this.resetTimers();
				}
			})
		);

		this.trackUnlisten(
			listenTauriEvent<string>(EV_RESET_TIMER, (type) => {
				if (this.isMaster && (type === 'eye' || type === 'stretch')) {
					this.resetTimer(type);
				}
			})
		);

		// Broadcast initial authoritative state
		this.emitStateSync();
	}

	private async initNonMaster() {
		this.trackUnlisten(
			listenTauriEvent<TimerStateSync>(EV_STATE_SYNC, (sync) => {
				if (!this.isMaster && sync) {
					this.handleStateSync(sync);
				}
			})
		);

		// Ask for state; retry a few times in case the master's listener
		// isn't registered yet (startup ordering).
		for (let attempt = 0; attempt < 3; attempt++) {
			await emitTauriEvent(EV_QUERY_STATE);
			await new Promise((r) => setTimeout(r, 500));
			if (this.lastSyncTimestamp > 0) break;
		}

		// Browser fallback: if not running in Tauri and no master responded,
		// elect this instance as master so standalone web development works.
		if (!isTauriEnvironment() && this.lastSyncTimestamp === 0) {
			this.electionTimeout = setTimeout(() => {
				if (!this.isMaster && this.eyeTargetTimestamp === null && this.breakTimeRemaining === 0) {
					console.log('[TimerManager] Standalone browser mode: electing as master');
					this.isMaster = true;
					void this.initMaster();
				}
			}, 1500);
		}
	}

	private async initIdleMonitor() {
		if (!this.isMaster) return;

		await this.applyIdleThreshold(settingsStore.settings.idleThresholdSec || 300);

		this.trackUnlisten(
			listenTauriEvent<{ idle: boolean; seconds?: number }>(EV_SYSTEM_IDLE, (payload) => {
				if (this.isMaster && payload) {
					this.handleIdleStateChange(payload.idle, payload.seconds);
				}
			})
		);
	}

	/**
	 * The idlemonitor plugin's `start` command fails with AlreadyRunning if the
	 * monitor is active, so a threshold change must stop first, then start.
	 */
	private async applyIdleThreshold(thresholdSec: number) {
		if (!this.isMaster) return;
		try {
			await tauriInvoke('plugin:idlemonitor|stop');
		} catch {
			// Not running yet — fine.
		}
		try {
			await tauriInvoke('plugin:idlemonitor|start', {
				options: { idleThresholdSecs: thresholdSec }
			});
		} catch (err) {
			console.warn('[TimerManager] Could not (re)start idlemonitor plugin:', err);
		}
	}

	private handleStateSync(sync: TimerStateSync) {
		if (this.isMaster) return;
		if (sync.timestamp && sync.timestamp <= this.lastSyncTimestamp) return; // stale
		this.lastSyncTimestamp = sync.timestamp || this.lastSyncTimestamp;

		this.isPaused = sync.isPaused;
		this.isIdle = sync.isIdle;
		this.activeBreak = sync.activeBreak;
		this.eyeTimeRemaining = sync.eyeTimeRemaining;
		this.stretchTimeRemaining = sync.stretchTimeRemaining;
		this.breakTimeRemaining = sync.breakTimeRemaining;
		this.awayStartTime = sync.awayStartTime;
		this.eyeTargetTimestamp = sync.eyeTargetTimestamp;
		this.stretchTargetTimestamp = sync.stretchTargetTimestamp;
		this.breakTargetTimestamp = sync.breakTargetTimestamp;
	}

	private async handleCommand(cmd: TimerCommand) {
		if (!this.isMaster || !cmd || !cmd.action) return;

		switch (cmd.action) {
			case 'trigger-break':
				if (cmd.type === 'eye' || cmd.type === 'stretch') {
					await this.triggerBreak(cmd.type);
				}
				break;
			case 'toggle-pause':
				this.togglePause();
				break;
			case 'complete-break':
				await this.completeBreak();
				break;
			case 'skip-break':
				await this.skipBreak();
				break;
			case 'reset-timers':
				this.resetTimers();
				break;
			case 'reset-timer':
				if (cmd.type === 'eye' || cmd.type === 'stretch') {
					this.resetTimer(cmd.type);
				}
				break;
		}
	}

	private handleSettingsChanged(newSettings: Settings) {
		if (!this.isMaster) return;

		// Reconfigure idle monitor if threshold changed
		if (newSettings.idleThresholdSec) {
			void this.applyIdleThreshold(newSettings.idleThresholdSec);
		}

		const now = Date.now();

		// Clamp / re-arm countdowns when intervals shrink (not during a break)
		if (!this.activeBreak) {
			if (newSettings.eyeEnabled) {
				const maxSec = this.intervalSecFor('eye');
				if (this.eyeTimeRemaining > maxSec) {
					this.eyeTimeRemaining = maxSec;
				}
				if (!this.isPaused && !this.isIdle) {
					this.eyeTargetTimestamp = now + this.eyeTimeRemaining * 1000;
				}
			} else {
				this.eyeTargetTimestamp = null;
			}

			if (newSettings.stretchEnabled) {
				const maxSec = this.intervalSecFor('stretch');
				if (this.stretchTimeRemaining > maxSec) {
					this.stretchTimeRemaining = maxSec;
				}
				if (!this.isPaused && !this.isIdle) {
					this.stretchTargetTimestamp = now + this.stretchTimeRemaining * 1000;
				}
			} else {
				this.stretchTargetTimestamp = null;
			}
		}

		this.emitStateSync();
	}

	emitStateSync() {
		if (!this.isMaster) return;

		const payload: TimerStateSync = {
			isPaused: this.isPaused,
			isIdle: this.isIdle,
			activeBreak: this.activeBreak,
			eyeTimeRemaining: this.eyeTimeRemaining,
			stretchTimeRemaining: this.stretchTimeRemaining,
			breakTimeRemaining: this.breakTimeRemaining,
			awayStartTime: this.awayStartTime,
			eyeTargetTimestamp: this.eyeTargetTimestamp,
			stretchTargetTimestamp: this.stretchTargetTimestamp,
			breakTargetTimestamp: this.breakTargetTimestamp,
			timestamp: Date.now()
		};

		emitTauriEvent(EV_STATE_SYNC, payload);
	}

	handleIdleStateChange(isIdle: boolean, idleSeconds?: number) {
		if (!this.isMaster) return;

		const now = Date.now();
		if (isIdle) {
			this.isIdle = true;
			this.awayStartTime = now;
			this.freezeTimers(now);
		} else {
			if (this.awayStartTime || idleSeconds !== undefined) {
				// Prefer the monitor's measured idle duration over wall-clock deltas
				const awaySecs = Math.max(
					1,
					Math.round(idleSeconds ?? (now - (this.awayStartTime ?? now)) / 1000)
				);
				const thresholdMinutes = settingsStore.settings.awayResetMin || 10;
				if (awaySecs / 60 >= thresholdMinutes) {
					this.resetTimersInternal(now);
				} else {
					this.resumeTimers(now);
				}

				// One away-session event per session, recorded on return with
				// the measured duration.
				void settingsStore.recordBreak({
					type: 'away',
					action: 'completed',
					durationSec: awaySecs,
					timestamp: now
				});
			} else {
				this.resumeTimers(now);
			}
			this.isIdle = false;
			this.awayStartTime = null;
		}

		this.emitStateSync();
	}

	private freezeTimers(now: number = Date.now()) {
		if (this.eyeTargetTimestamp) {
			this.eyeTimeRemaining = Math.max(0, Math.ceil((this.eyeTargetTimestamp - now) / 1000));
			this.eyeTargetTimestamp = null;
		}
		if (this.stretchTargetTimestamp) {
			this.stretchTimeRemaining = Math.max(0, Math.ceil((this.stretchTargetTimestamp - now) / 1000));
			this.stretchTargetTimestamp = null;
		}
		if (this.breakTargetTimestamp) {
			this.breakTimeRemaining = Math.max(0, Math.ceil((this.breakTargetTimestamp - now) / 1000));
			this.breakTargetTimestamp = null;
		}
	}

	private resumeTimers(now: number = Date.now()) {
		if (this.isPaused || this.isIdle) return;

		if (this.activeBreak) {
			if (!this.breakTargetTimestamp && this.breakTimeRemaining > 0) {
				this.breakTargetTimestamp = now + this.breakTimeRemaining * 1000;
			}
			return;
		}

		this.eyeTargetTimestamp = settingsStore.settings.eyeEnabled
			? now + this.eyeTimeRemaining * 1000
			: null;
		this.stretchTargetTimestamp = settingsStore.settings.stretchEnabled
			? now + this.stretchTimeRemaining * 1000
			: null;
	}

	/** Guided break duration in seconds for a break type. */
	durationFor(type: BreakType): number {
		return type === 'eye'
			? settingsStore.settings.eyeDurationSec || DEFAULT_SETTINGS.eyeDurationSec
			: settingsStore.settings.stretchDurationSec || DEFAULT_SETTINGS.stretchDurationSec;
	}

	/** Reminder interval in seconds for a break type. */
	intervalSecFor(type: BreakType): number {
		return (
			(type === 'eye'
				? settingsStore.settings.eyeIntervalMin || DEFAULT_SETTINGS.eyeIntervalMin
				: settingsStore.settings.stretchIntervalMin || DEFAULT_SETTINGS.stretchIntervalMin) * 60
		);
	}

	/** HUD scale multiplier: 1 = normal, 2 = "2× Size" menu mode. */
	get hudScale(): 1 | 2 {
		return settingsStore.settings.hud2x ? 2 : 1;
	}

	/** Compact idle HUD size at the current scale. */
	get idleHudSize(): { width: number; height: number } {
		return this.hudScale === 2
			? { width: IDLE_HUD_2X_WIDTH, height: IDLE_HUD_2X_HEIGHT }
			: { width: IDLE_HUD_WIDTH, height: IDLE_HUD_HEIGHT };
	}

	private resetTimersInternal(now: number = Date.now()) {
		this.eyeTimeRemaining = this.intervalSecFor('eye');
		this.stretchTimeRemaining = this.intervalSecFor('stretch');
		this.activeBreak = null;
		this.breakTimeRemaining = 0;
		this.breakTargetTimestamp = null;

		if (!this.isPaused && !this.isIdle) {
			this.eyeTargetTimestamp = settingsStore.settings.eyeEnabled
				? now + this.eyeTimeRemaining * 1000
				: null;
			this.stretchTargetTimestamp = settingsStore.settings.stretchEnabled
				? now + this.stretchTimeRemaining * 1000
				: null;
		} else {
			this.eyeTargetTimestamp = null;
			this.stretchTargetTimestamp = null;
		}
	}

	resetTimers() {
		if (!this.isMaster) {
			emitTauriEvent(EV_COMMAND, { action: 'reset-timers' });
			return;
		}

		this.resetTimersInternal();
		this.emitStateSync();
	}

	/** Restart a single reminder countdown to its full interval. */
	resetTimer(type: BreakType) {
		if (type !== 'eye' && type !== 'stretch') return;
		if (!this.isMaster) {
			emitTauriEvent(EV_COMMAND, { action: 'reset-timer', type });
			return;
		}

		this.rearmTimerFor(type, Date.now());
		this.emitStateSync();
	}

	togglePause() {
		if (!this.isMaster) {
			emitTauriEvent(EV_COMMAND, { action: 'toggle-pause' });
			return;
		}

		this.isPaused = !this.isPaused;
		const now = Date.now();

		if (this.isPaused) {
			this.freezeTimers(now);
		} else {
			this.resumeTimers(now);
		}

		this.emitStateSync();
	}

	/**
	 * Claims the active break synchronously (clearing it before any await) so
	 * a double-click or a tick racing a slow stats write cannot double-record.
	 */
	private claimActiveBreak(): BreakType | null {
		const type = this.activeBreak;
		if (!type || type === 'away') return null;
		this.activeBreak = null;
		this.breakTargetTimestamp = null;
		this.breakTimeRemaining = 0;
		return type;
	}

	async triggerBreak(type: BreakType) {
		if (!this.isMaster) {
			await emitTauriEvent(EV_COMMAND, { action: 'trigger-break', type });
			return;
		}
		if (type === 'away') return;

		// If another break is active, record it as expired before replacing it.
		const interrupted = this.claimActiveBreak();
		if (interrupted && interrupted !== type) {
			await settingsStore.recordBreak({
				type: interrupted,
				action: 'expired',
				durationSec: this.durationFor(interrupted)
			});
			this.rearmTimerFor(interrupted, Date.now());
		}

		// Make sure the overlay is visible even if we're in settings mode —
		// natural timer breaks must not fire in a hidden window.
		await enterOverlayMode();

		const now = Date.now();
		this.activeBreak = type;
		this.breakTimeRemaining = this.durationFor(type);
		this.breakTargetTimestamp = now + this.breakTimeRemaining * 1000;

		// Freeze reminder countdowns while the break is underway
		this.freezeTimers(now);

		// No window resize here: break UI renders inside the existing
		// idle-sized window. Resizing the macOS panel mid-session produces
		// a visible white outline flash on every break.

		// Play the start-of-break chime (once, from the authoritative master)
		playSound(
			settingsStore.settings.soundPreset,
			settingsStore.settings.soundVolume,
			settingsStore.settings.soundFadeIn,
			Math.max(1, Math.min(5, settingsStore.settings.soundRepeat || 1))
		);

		this.emitStateSync();
	}

	async completeBreak() {
		if (!this.isMaster) {
			await emitTauriEvent(EV_COMMAND, { action: 'complete-break' });
			return;
		}

		const type = this.claimActiveBreak();
		if (!type) return;

		await settingsStore.recordBreak({
			type,
			action: 'completed',
			durationSec: this.durationFor(type)
		});

		await this.finishActiveBreak(type, 'completed');
	}

	async skipBreak() {
		if (!this.isMaster) {
			await emitTauriEvent(EV_COMMAND, { action: 'skip-break' });
			return;
		}

		const type = this.claimActiveBreak();
		if (!type) return;

		await settingsStore.recordBreak({
			type,
			action: 'skipped',
			durationSec: this.durationFor(type)
		});

		await this.finishActiveBreak(type, 'skipped');
	}

	/**
	 * Countdown reached zero without user action: record as 'expired' so
	 * adherence stats are not inflated by ignored prompts.
	 */
	async expireBreak() {
		if (!this.isMaster) return;

		const type = this.claimActiveBreak();
		if (!type) return;

		await settingsStore.recordBreak({
			type,
			action: 'expired',
			durationSec: this.durationFor(type)
		});

		await this.finishActiveBreak(type, 'expired');
	}

	/** Reset one reminder countdown to its full interval. */
	private rearmTimerFor(type: BreakType, now: number) {
		if (type === 'eye') {
			this.eyeTimeRemaining = this.intervalSecFor('eye');
			if (!this.isPaused && !this.isIdle && settingsStore.settings.eyeEnabled) {
				this.eyeTargetTimestamp = now + this.eyeTimeRemaining * 1000;
			}
		} else {
			this.stretchTimeRemaining = this.intervalSecFor('stretch');
			if (!this.isPaused && !this.isIdle && settingsStore.settings.stretchEnabled) {
				this.stretchTargetTimestamp = now + this.stretchTimeRemaining * 1000;
			}
		}
	}

	private async finishActiveBreak(type: BreakType, action: 'completed' | 'skipped' | 'expired') {
		this.activeBreak = null;
		this.breakTimeRemaining = 0;
		this.breakTargetTimestamp = null;

		const now = Date.now();

		// Reset countdown for this break type
		this.rearmTimerFor(type, now);

		// Resume the other reminder if not paused or idle
		if (!this.isPaused && !this.isIdle) {
			if (type === 'eye' && settingsStore.settings.stretchEnabled && !this.stretchTargetTimestamp) {
				this.stretchTargetTimestamp = now + this.stretchTimeRemaining * 1000;
			} else if (type === 'stretch' && settingsStore.settings.eyeEnabled && !this.eyeTargetTimestamp) {
				this.eyeTargetTimestamp = now + this.eyeTimeRemaining * 1000;
			}
		}

		// End-of-break cue: fixed gentle two-tone "tum-tuumm" (not a repeat of
		// the start chime) — played when the routine actually ran (not skip).
		if (action !== 'skipped') {
			void playBreakEnd();
		}

		// No window resize here (see triggerBreak): the window stays at its
		// idle size for the whole session.

		this.emitStateSync();
	}

	private tick() {
		if (!this.isMaster) return;

		const now = Date.now();

		// Check for sleep/suspension gap
		const elapsedSinceLastTick = this.lastTickTime ? now - this.lastTickTime : 0;
		this.lastTickTime = now;

		const awayResetMs = (settingsStore.settings.awayResetMin || 10) * 60 * 1000;
		if (elapsedSinceLastTick >= awayResetMs) {
			// Computer was asleep or suspended for >= awayResetMin, reset on return
			this.resetTimersInternal(now);
			this.emitStateSync();
			return;
		}

		// 1. Active break in progress
		if (this.activeBreak) {
			if (!this.breakTargetTimestamp) {
				this.breakTargetTimestamp = now + this.breakTimeRemaining * 1000;
			}

			this.breakTimeRemaining = Math.max(
				0,
				Math.ceil((this.breakTargetTimestamp - now) / 1000)
			);

			if (this.breakTimeRemaining <= 0) {
				void this.expireBreak();
				return;
			}

			this.emitStateSync();
			return;
		}

		// 2. If user is idle or paused, freeze countdowns
		if (this.isIdle || this.isPaused) {
			this.emitStateSync();
			return;
		}

		// 3. Eye timer countdown
		if (settingsStore.settings.eyeEnabled) {
			if (!this.eyeTargetTimestamp) {
				this.eyeTargetTimestamp = now + this.eyeTimeRemaining * 1000;
			}
			this.eyeTimeRemaining = Math.max(0, Math.ceil((this.eyeTargetTimestamp - now) / 1000));

			if (this.eyeTimeRemaining <= 0) {
				void this.triggerBreak('eye');
				return;
			}
		} else {
			this.eyeTargetTimestamp = null;
		}

		// 4. Stretch timer countdown
		if (settingsStore.settings.stretchEnabled) {
			if (!this.stretchTargetTimestamp) {
				this.stretchTargetTimestamp = now + this.stretchTimeRemaining * 1000;
			}
			this.stretchTimeRemaining = Math.max(
				0,
				Math.ceil((this.stretchTargetTimestamp - now) / 1000)
			);

			if (this.stretchTimeRemaining <= 0) {
				void this.triggerBreak('stretch');
				return;
			}
		} else {
			this.stretchTargetTimestamp = null;
		}

		this.emitStateSync();
	}

	destroy() {
		this.generation++;
		if (this.timerInterval) {
			clearInterval(this.timerInterval);
			this.timerInterval = null;
		}
		if (this.electionTimeout) {
			clearTimeout(this.electionTimeout);
			this.electionTimeout = null;
		}
		for (const unlisten of this.unlistenFns) {
			try {
				unlisten();
			} catch {}
		}
		this.unlistenFns = [];
		this.initialized = false;
	}
}

export const timerManager = new TimerManager();
