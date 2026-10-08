import { playSound } from '$lib/services/soundPlayer';
import { settingsStore } from '$lib/services/settingsStore.svelte';
import {
	emitTauriEvent,
	listenTauriEvent,
	isMasterWindow,
	isTauriEnvironment,
	setOverlayWindowSize,
	tauriInvoke,
	IDLE_HUD_WIDTH,
	IDLE_HUD_HEIGHT
} from '$lib/services/windowManager';
import { blinkCountFor, exerciseIndexFor } from '$lib/services/breakLogic';
import type { BreakType, HudSize, Settings, TimerCommand, TimerStateSync } from '$lib/types';

export const EYE_HUD_WIDTH = 104;
export const EYE_HUD_HEIGHT = 48;
export const STRETCH_HUD_WIDTH = 104;
export const STRETCH_HUD_HEIGHT = 48;

export interface StretchExercise {
	step: number;
	shortName: string;
	title: string;
	desc: string;
	tip: string;
}

export const STRETCH_EXERCISES: StretchExercise[] = [
	{
		step: 1,
		shortName: 'Walk',
		title: 'Stand up & walk',
		desc: 'Step away from your desk and take a few easy paces',
		tip: 'Breaks static sitting posture and reactivates legs'
	},
	{
		step: 2,
		shortName: 'Arms',
		title: 'Stretch your arms',
		desc: 'Reach high overhead and gently interlock your fingers',
		tip: 'Decompresses spine and opens ribcage'
	},
	{
		step: 3,
		shortName: 'Shoulders',
		title: 'Roll your shoulders',
		desc: 'Roll shoulders backward 5 times, then smoothly forward',
		tip: 'Releases tension across traps, upper back, and neck'
	},
	{
		step: 4,
		shortName: 'Squats',
		title: 'Gentle squats',
		desc: 'Perform 3–5 slow, relaxed squats at your own pace',
		tip: 'Restores healthy blood flow through hips and core'
	}
];

export const EYE_INSTRUCTIONS = [
	'Look 20 feet (6m) away into the distance',
	'Relax your facial, brow, and forehead muscles',
	'Blink gently to refresh your natural tear film',
	'Shift focus toward a window or far corner',
	'Release tension around your temples',
	'Blink naturally and soften your visual gaze',
	'Breathe deeply — disconnect from digital pixels',
	'Let your peripheral field open up smoothly',
	'One more slow, gentle blink into the distance',
	'Almost done — eyes refreshed, relaxed & clear'
];

class TimerManager {
	// Master identity
	isMaster = $state(false);

	// Status
	isPaused = $state(false);
	isIdle = $state(false);
	activeBreak = $state<BreakType | null>(null);

	// Time remaining in seconds
	eyeTimeRemaining = $state(20 * 60);
	stretchTimeRemaining = $state(45 * 60);
	breakTimeRemaining = $state(0);

	// Timestamp targets for drift-free calculations
	eyeTargetTimestamp = $state<number | null>(null);
	stretchTargetTimestamp = $state<number | null>(null);
	breakTargetTimestamp = $state<number | null>(null);

	// Guided animations
	blinkCount = $state(1); // 1 to 10
	currentExerciseIndex = $state(0); // 0 to 3

	// Idle & Away tracking
	awayStartTime = $state<number | null>(null);

	// Custom compact HUD size (persisted as settings.hudSize)
	idleHudSize = $state<HudSize>({ width: IDLE_HUD_WIDTH, height: IDLE_HUD_HEIGHT });

	private lastTickTime = Date.now();

	private timerInterval: ReturnType<typeof setInterval> | null = null;
	private initialized = false;
	private unlistenFns: (() => void)[] = [];

	async init() {
		if (this.initialized) return;
		this.initialized = true;

		await settingsStore.init();

		const master = isMasterWindow();
		this.isMaster = master;

		settingsStore.onSettingsChange((newSettings) => {
			if (this.isMaster) {
				this.handleSettingsChanged(newSettings);
			}
		});

		settingsStore.onStatsReset(() => {
			this.resetTimers();
		});

		if (this.isMaster) {
			this.initMaster();
		} else {
			this.initNonMaster();
		}
	}

	private async initMaster() {
		this.resetTimersInternal();

		// Restore custom compact HUD size if the user resized the grip before
		this.idleHudSize = settingsStore.settings.hudSize ?? {
			width: IDLE_HUD_WIDTH,
			height: IDLE_HUD_HEIGHT
		};

		// Start authoritative 1s tick loop
		this.lastTickTime = Date.now();
		if (this.timerInterval) clearInterval(this.timerInterval);
		this.timerInterval = setInterval(() => this.tick(), 1000);

		// Initialize native Tauri idle monitor
		await this.initIdleMonitor();

		// Listen to commands from non-master windows ('breaktime:command')
		const unlistenCmd = await listenTauriEvent<TimerCommand>('breaktime:command', async (cmd) => {
			if (this.isMaster && cmd) {
				await this.handleCommand(cmd);
			}
		});
		this.unlistenFns.push(unlistenCmd);

		// Listen to query state requests from non-master windows
		const unlistenQuery = await listenTauriEvent('breaktime:query-state', () => {
			if (this.isMaster) {
				this.emitStateSync();
			}
		});
		this.unlistenFns.push(unlistenQuery);

		// Listen to Tray menu events
		const unlistenTrigger = await listenTauriEvent<string>('trigger-break', async (type) => {
			if (this.isMaster && (type === 'eye' || type === 'stretch')) {
				await this.triggerBreak(type as BreakType);
			}
		});
		this.unlistenFns.push(unlistenTrigger);

		const unlistenToggle = await listenTauriEvent('toggle-timers', () => {
			if (this.isMaster) {
				this.togglePause();
			}
		});
		this.unlistenFns.push(unlistenToggle);

		// Listen to settings changes to adjust countdown intervals
		const unlistenSettings = await listenTauriEvent<Settings>('breaktime:settings-changed', (newSettings) => {
			if (this.isMaster && newSettings) {
				this.handleSettingsChanged(newSettings);
			}
		});
		this.unlistenFns.push(unlistenSettings);

		// Listen to stats reset / timer reset events
		const unlistenStatsReset = await listenTauriEvent('breaktime:stats-reset', () => {
			if (this.isMaster) {
				this.resetTimers();
			}
		});
		this.unlistenFns.push(unlistenStatsReset);

		// Broadcast initial authoritative state
		this.emitStateSync();
	}

	private async initNonMaster() {
		// Listen to state synchronization from Master window
		const unlistenSync = await listenTauriEvent<TimerStateSync>('breaktime:state-sync', (sync) => {
			if (!this.isMaster && sync) {
				this.handleStateSync(sync);
			}
		});
		this.unlistenFns.push(unlistenSync);

		// Query current state immediately
		await emitTauriEvent('breaktime:query-state');

		// Browser fallback: if not running in Tauri and no master responds after 1.5s,
		// elect this instance as master so standalone web development works smoothly.
		if (!isTauriEnvironment()) {
			setTimeout(() => {
				if (!this.isMaster && this.eyeTargetTimestamp === null && this.breakTimeRemaining === 0) {
					console.log('[TimerManager] Standalone browser mode: electing as master');
					this.isMaster = true;
					this.initMaster();
				}
			}, 1500);
		}
	}

	private async initIdleMonitor() {
		if (!this.isMaster) return;

		await this.applyIdleThreshold(settingsStore.settings.idleThresholdSec || 300);

		// Listen to system:idle event from tauri-plugin-idlemonitor
		const unlistenIdle = await listenTauriEvent<{ idle: boolean; seconds?: number }>('system:idle', (payload) => {
			if (this.isMaster && payload) {
				this.handleIdleStateChange(payload.idle, payload.seconds);
			}
		});
		this.unlistenFns.push(unlistenIdle);
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

		this.isPaused = sync.isPaused;
		this.isIdle = sync.isIdle;
		this.activeBreak = sync.activeBreak;
		this.eyeTimeRemaining = sync.eyeTimeRemaining;
		this.stretchTimeRemaining = sync.stretchTimeRemaining;
		this.breakTimeRemaining = sync.breakTimeRemaining;
		this.blinkCount = sync.blinkCount;
		this.currentExerciseIndex = sync.currentExerciseIndex;
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
		}
	}

	private handleSettingsChanged(newSettings: Settings) {
		if (!this.isMaster) return;

		// Reconfigure idle monitor if threshold changed
		if (newSettings.idleThresholdSec) {
			void this.applyIdleThreshold(newSettings.idleThresholdSec);
		}

		// Track custom HUD size without touching countdowns
		if (newSettings.hudSize) {
			this.idleHudSize = newSettings.hudSize;
		}

		const now = Date.now();

		// If eye setting interval updated and not in active break
		if (!this.activeBreak) {
			if (newSettings.eyeEnabled) {
				const maxSec = (newSettings.eyeIntervalMin || 20) * 60;
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
				const maxSec = (newSettings.stretchIntervalMin || 45) * 60;
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
			blinkCount: this.blinkCount,
			currentExerciseIndex: this.currentExerciseIndex,
			awayStartTime: this.awayStartTime,
			eyeTargetTimestamp: this.eyeTargetTimestamp,
			stretchTargetTimestamp: this.stretchTargetTimestamp,
			breakTargetTimestamp: this.breakTargetTimestamp,
			timestamp: Date.now()
		};

		emitTauriEvent('breaktime:state-sync', payload);
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
				const awaySecs = idleSeconds ?? (now - (this.awayStartTime ?? now)) / 1000;
				const thresholdMinutes = settingsStore.settings.awayResetMin || 10;
				if (awaySecs / 60 >= thresholdMinutes) {
					// User was away for >= awayResetMin (default 10 min), reset timers on return
					this.resetTimersInternal(now);
				} else {
					this.resumeTimers(now);
				}
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

		if (settingsStore.settings.eyeEnabled) {
			this.eyeTargetTimestamp = now + this.eyeTimeRemaining * 1000;
		} else {
			this.eyeTargetTimestamp = null;
		}

		if (settingsStore.settings.stretchEnabled) {
			this.stretchTargetTimestamp = now + this.stretchTimeRemaining * 1000;
		} else {
			this.stretchTargetTimestamp = null;
		}
	}

	private resetTimersInternal(now: number = Date.now()) {
		this.eyeTimeRemaining = (settingsStore.settings.eyeIntervalMin || 20) * 60;
		this.stretchTimeRemaining = (settingsStore.settings.stretchIntervalMin || 45) * 60;
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
			emitTauriEvent('breaktime:command', { action: 'reset-timers' });
			return;
		}

		this.resetTimersInternal();
		this.emitStateSync();
	}

	togglePause() {
		if (!this.isMaster) {
			emitTauriEvent('breaktime:command', { action: 'toggle-pause' });
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

	async triggerBreak(type: BreakType) {
		if (!this.isMaster) {
			await emitTauriEvent('breaktime:command', { action: 'trigger-break', type });
			return;
		}

		// If another break is active, record it as expired before replacing it
		// (e.g. tray "Take Eye Break Now" during an active stretch break).
		if (this.activeBreak && this.activeBreak !== type) {
			const interrupted = this.activeBreak;
			const interruptedDuration =
				interrupted === 'eye'
					? settingsStore.settings.eyeDurationSec || 20
					: settingsStore.settings.stretchDurationSec || 60;
			await settingsStore.recordBreak({
				type: interrupted,
				action: 'expired',
				durationSec: interruptedDuration
			});
			this.rearmTimerFor(interrupted, Date.now());
		}

		this.activeBreak = type;

		const duration =
			type === 'eye'
				? settingsStore.settings.eyeDurationSec || 20
				: settingsStore.settings.stretchDurationSec || 60;

		const now = Date.now();
		this.breakTimeRemaining = duration;
		this.breakTargetTimestamp = now + duration * 1000;
		this.blinkCount = 1;
		this.currentExerciseIndex = 0;

		// Freeze non-active countdowns while break is underway
		if (this.eyeTargetTimestamp) {
			this.eyeTimeRemaining = Math.max(0, Math.ceil((this.eyeTargetTimestamp - now) / 1000));
			this.eyeTargetTimestamp = null;
		}
		if (this.stretchTargetTimestamp) {
			this.stretchTimeRemaining = Math.max(0, Math.ceil((this.stretchTargetTimestamp - now) / 1000));
			this.stretchTargetTimestamp = null;
		}

		// Expand overlay window smoothly
		if (type === 'eye') {
			await setOverlayWindowSize(EYE_HUD_WIDTH, EYE_HUD_HEIGHT);
		} else {
			await setOverlayWindowSize(STRETCH_HUD_WIDTH, STRETCH_HUD_HEIGHT);
		}

		// Play alert sound once from authoritative master
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
			await emitTauriEvent('breaktime:command', { action: 'complete-break' });
			return;
		}

		if (!this.activeBreak) return;
		const type = this.activeBreak;
		const duration =
			type === 'eye'
				? settingsStore.settings.eyeDurationSec || 20
				: settingsStore.settings.stretchDurationSec || 60;

		await settingsStore.recordBreak({
			type,
			action: 'completed',
			durationSec: duration
		});

		await this.finishActiveBreak(type, 'completed');
	}

	async skipBreak() {
		if (!this.isMaster) {
			await emitTauriEvent('breaktime:command', { action: 'skip-break' });
			return;
		}

		if (!this.activeBreak) return;
		const type = this.activeBreak;
		const duration =
			type === 'eye'
				? settingsStore.settings.eyeDurationSec || 20
				: settingsStore.settings.stretchDurationSec || 60;

		await settingsStore.recordBreak({
			type,
			action: 'skipped',
			durationSec: duration
		});

		await this.finishActiveBreak(type, 'skipped');
	}

	/**
	 * Countdown reached zero without user action: record as 'expired' so
	 * adherence stats are not inflated by ignored prompts.
	 */
	async expireBreak() {
		if (!this.isMaster) return;

		if (!this.activeBreak) return;
		const type = this.activeBreak;
		const duration =
			type === 'eye'
				? settingsStore.settings.eyeDurationSec || 20
				: settingsStore.settings.stretchDurationSec || 60;

		await settingsStore.recordBreak({
			type,
			action: 'expired',
			durationSec: duration
		});

		await this.finishActiveBreak(type, 'expired');
	}

	/** Reset one reminder countdown to its full interval (shared by finish/expire paths). */
	private rearmTimerFor(type: BreakType, now: number) {
		if (type === 'eye') {
			this.eyeTimeRemaining = (settingsStore.settings.eyeIntervalMin || 20) * 60;
			if (!this.isPaused && !this.isIdle && settingsStore.settings.eyeEnabled) {
				this.eyeTargetTimestamp = now + this.eyeTimeRemaining * 1000;
			}
		} else {
			this.stretchTimeRemaining = (settingsStore.settings.stretchIntervalMin || 45) * 60;
			if (!this.isPaused && !this.isIdle && settingsStore.settings.stretchEnabled) {
				this.stretchTargetTimestamp = now + this.stretchTimeRemaining * 1000;
			}
		}
	}

	private async finishActiveBreak(type: BreakType, _action: 'completed' | 'skipped' | 'expired') {
		this.activeBreak = null;
		this.breakTimeRemaining = 0;
		this.breakTargetTimestamp = null;

		const now = Date.now();

		// Reset countdown for this completed/skipped/expired break type
		this.rearmTimerFor(type, now);

		// Resume the other timer if not paused or idle
		if (!this.isPaused && !this.isIdle) {
			if (type === 'eye' && settingsStore.settings.stretchEnabled && !this.stretchTargetTimestamp) {
				this.stretchTargetTimestamp = now + this.stretchTimeRemaining * 1000;
			} else if (type === 'stretch' && settingsStore.settings.eyeEnabled && !this.eyeTargetTimestamp) {
				this.eyeTargetTimestamp = now + this.eyeTimeRemaining * 1000;
			}
		}

		// Shrink HUD back to compact idle size (user's custom size if set)
		await setOverlayWindowSize(this.idleHudSize.width, this.idleHudSize.height);

		this.emitStateSync();
	}

	/** Persist a custom compact HUD size captured from the resize grip. */
	async setIdleHudSize(width: number, height: number) {
		this.idleHudSize = { width, height };
		await settingsStore.updateSettings({ hudSize: { width, height } });
	}

	private tick() {
		if (!this.isMaster) return;

		const now = Date.now();

		// Check for sleep/suspension gap
		const elapsedSinceLastTick = this.lastTickTime ? now - this.lastTickTime : 0;
		this.lastTickTime = now;

		const awayResetMs = (settingsStore.settings.awayResetMin || 10) * 60 * 1000;
		if (elapsedSinceLastTick >= awayResetMs) {
			// Computer was asleep or suspended for >= awayResetMin, reset upon return
			this.resetTimersInternal(now);
			this.emitStateSync();
			return;
		}

		// 1. Active break in progress
		if (this.activeBreak) {
			if (!this.breakTargetTimestamp) {
				this.breakTargetTimestamp = now + this.breakTimeRemaining * 1000;
			}

			const diffMs = this.breakTargetTimestamp - now;
			this.breakTimeRemaining = Math.max(0, Math.ceil(diffMs / 1000));

			const totalDuration =
				this.activeBreak === 'eye'
					? settingsStore.settings.eyeDurationSec || 20
					: settingsStore.settings.stretchDurationSec || 60;

			const elapsed = Math.max(0, totalDuration - this.breakTimeRemaining);

			if (this.activeBreak === 'eye') {
				// 10 counted blinks over duration (default 20s = 1 blink every 2s)
				this.blinkCount = blinkCountFor(elapsed, totalDuration);
			} else {
				// 4 rotating stretch exercises (every quarter of duration)
				this.currentExerciseIndex = exerciseIndexFor(elapsed, totalDuration);
			}

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
			const diffMs = this.eyeTargetTimestamp - now;
			this.eyeTimeRemaining = Math.max(0, Math.ceil(diffMs / 1000));

			if (this.eyeTimeRemaining <= 0) {
				this.triggerBreak('eye');
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
			const diffMs = this.stretchTargetTimestamp - now;
			this.stretchTimeRemaining = Math.max(0, Math.ceil(diffMs / 1000));

			if (this.stretchTimeRemaining <= 0) {
				this.triggerBreak('stretch');
				return;
			}
		} else {
			this.stretchTargetTimestamp = null;
		}

		this.emitStateSync();
	}

	formatTimeShort(seconds: number): string {
		if (seconds >= 60) {
			const mins = Math.ceil(seconds / 60);
			return `${mins}m`;
		}
		return `${seconds}s`;
	}

	destroy() {
		if (this.timerInterval) {
			clearInterval(this.timerInterval);
			this.timerInterval = null;
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
