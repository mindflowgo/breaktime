import { playSound } from '$lib/services/soundPlayer';
import { settingsStore } from '$lib/services/settingsStore.svelte';
import {
	emitTauriEvent,
	listenTauriEvent,
	setOverlayWindowSize,
	tauriInvoke,
	IDLE_HUD_WIDTH,
	IDLE_HUD_HEIGHT
} from '$lib/services/windowManager';
import type { BreakType } from '$lib/types';

export const STRETCH_EXERCISES = [
	{ title: 'Stand up & walk', desc: 'Step away from your desk and take a few easy steps' },
	{ title: 'Stretch your arms', desc: 'Reach high above your head and interlock fingers' },
	{ title: 'Roll your shoulders', desc: 'Gently roll shoulders backwards 5 times, then forward' },
	{ title: 'Gentle squats', desc: 'Do 3-5 relaxed slow squats to restore leg blood flow' }
];

export const EYE_INSTRUCTIONS = [
	'Look 20 feet away into the distance',
	'Relax your eye muscles and blink naturally',
	'Disconnect from close-up screen focus'
];

class TimerManager {
	// Status
	isPaused = $state(false);
	isIdle = $state(false);
	activeBreak = $state<BreakType | null>(null);

	// Time remaining in seconds
	eyeTimeRemaining = $state(20 * 60);
	stretchTimeRemaining = $state(45 * 60);
	breakTimeRemaining = $state(0);

	// Guided animations
	blinkCount = $state(1); // 1 to 10
	currentExerciseIndex = $state(0); // 0 to 3

	// Idle & Away tracking
	awayStartTime = $state<number | null>(null);

	private timerInterval: ReturnType<typeof setInterval> | null = null;
	private initialized = false;

	async init() {
		if (this.initialized) return;
		this.initialized = true;

		await settingsStore.init();

		this.resetTimers();

		// Start interval loop (ticks every 1s)
		this.timerInterval = setInterval(() => this.tick(), 1000);

		// Initialize Tauri idle monitor
		this.initIdleMonitor();

		// Listen to Tray menu events
		listenTauriEvent<string>('trigger-break', (type) => {
			if (type === 'eye' || type === 'stretch') {
				this.triggerBreak(type as BreakType);
			}
		});

		listenTauriEvent('toggle-timers', () => {
			this.togglePause();
		});

		// Listen to cross-window timer events
		listenTauriEvent<{ type: BreakType; action: 'completed' | 'skipped' }>('breaktime:break-action', (evt) => {
			if (evt) {
				this.handleBreakActionRemote(evt.type, evt.action);
			}
		});

		listenTauriEvent<BreakType>('breaktime:trigger-break-event', (type) => {
			if (type && this.activeBreak !== type) {
				this.triggerBreak(type, false);
			}
		});

		listenTauriEvent<boolean>('breaktime:pause-event', (paused) => {
			this.isPaused = paused;
		});
	}

	private async initIdleMonitor() {
		const thresholdSec = settingsStore.settings.idleThresholdSec || 300;
		try {
			await tauriInvoke('plugin:idlemonitor|start', {
				options: { idleThresholdSecs: thresholdSec }
			});
		} catch (err) {
			console.warn('[TimerManager] Could not start idlemonitor plugin:', err);
		}

		// Listen to system:idle event from tauri-plugin-idlemonitor
		listenTauriEvent<{ idle: boolean; seconds?: number }>('system:idle', (payload) => {
			if (payload) {
				this.handleIdleStateChange(payload.idle);
			}
		});
	}

	handleIdleStateChange(isIdle: boolean) {
		if (isIdle) {
			this.isIdle = true;
			this.awayStartTime = Date.now();
		} else {
			if (this.awayStartTime) {
				const awayMinutes = (Date.now() - this.awayStartTime) / (1000 * 60);
				const thresholdMinutes = settingsStore.settings.awayResetMin || 10;
				if (awayMinutes >= thresholdMinutes) {
					// User was away for >= awayResetMin (default 10 min), reset timers on return
					this.resetTimers();
				}
			}
			this.isIdle = false;
			this.awayStartTime = null;
		}
	}

	resetTimers() {
		this.eyeTimeRemaining = (settingsStore.settings.eyeIntervalMin || 20) * 60;
		this.stretchTimeRemaining = (settingsStore.settings.stretchIntervalMin || 45) * 60;
	}

	togglePause() {
		this.isPaused = !this.isPaused;
		emitTauriEvent('breaktime:pause-event', this.isPaused);
	}

	async triggerBreak(type: BreakType, broadcast: boolean = true) {
		this.activeBreak = type;

		const duration =
			type === 'eye'
				? settingsStore.settings.eyeDurationSec || 20
				: settingsStore.settings.stretchDurationSec || 60;

		this.breakTimeRemaining = duration;
		this.blinkCount = 1;
		this.currentExerciseIndex = 0;

		// Expand overlay window smoothly
		if (type === 'eye') {
			await setOverlayWindowSize(340, 96);
		} else {
			await setOverlayWindowSize(360, 130);
		}

		// Play alert sound
		playSound(
			settingsStore.settings.soundPreset,
			settingsStore.settings.soundVolume,
			settingsStore.settings.soundFadeIn
		);

		if (broadcast) {
			emitTauriEvent('breaktime:trigger-break-event', type);
		}
	}

	async completeBreak() {
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

	private async finishActiveBreak(type: BreakType, action: 'completed' | 'skipped') {
		this.activeBreak = null;
		this.breakTimeRemaining = 0;

		// Reset countdown for this break type
		if (type === 'eye') {
			this.eyeTimeRemaining = (settingsStore.settings.eyeIntervalMin || 20) * 60;
		} else {
			this.stretchTimeRemaining = (settingsStore.settings.stretchIntervalMin || 45) * 60;
		}

		// Shrink HUD back to compact idle size
		await setOverlayWindowSize(IDLE_HUD_WIDTH, IDLE_HUD_HEIGHT);

		emitTauriEvent('breaktime:break-action', { type, action });
	}

	private async handleBreakActionRemote(type: BreakType, _action: 'completed' | 'skipped') {
		this.activeBreak = null;
		this.breakTimeRemaining = 0;
		if (type === 'eye') {
			this.eyeTimeRemaining = (settingsStore.settings.eyeIntervalMin || 20) * 60;
		} else {
			this.stretchTimeRemaining = (settingsStore.settings.stretchIntervalMin || 45) * 60;
		}
		await setOverlayWindowSize(IDLE_HUD_WIDTH, IDLE_HUD_HEIGHT);
	}

	private tick() {
		// Active break in progress
		if (this.activeBreak) {
			if (this.breakTimeRemaining > 0) {
				this.breakTimeRemaining--;

				const totalDuration =
					this.activeBreak === 'eye'
						? settingsStore.settings.eyeDurationSec || 20
						: settingsStore.settings.stretchDurationSec || 60;

				const elapsed = totalDuration - this.breakTimeRemaining;

				if (this.activeBreak === 'eye') {
					// 10 counted blinks over duration (default 20s = 1 blink every 2s)
					this.blinkCount = Math.min(10, Math.floor((elapsed / totalDuration) * 10) + 1);
				} else {
					// 4 rotating stretch exercises (every quarter of duration)
					this.currentExerciseIndex = Math.min(3, Math.floor((elapsed / totalDuration) * 4));
				}

				if (this.breakTimeRemaining <= 0) {
					this.completeBreak();
				}
			}
			return;
		}

		// If user is idle or paused, freeze countdowns
		if (this.isIdle || this.isPaused) {
			return;
		}

		// Decrement Eye Timer
		if (settingsStore.settings.eyeEnabled) {
			if (this.eyeTimeRemaining > 0) {
				this.eyeTimeRemaining--;
				if (this.eyeTimeRemaining <= 0) {
					this.triggerBreak('eye');
					return;
				}
			}
		}

		// Decrement Stretch Timer
		if (settingsStore.settings.stretchEnabled) {
			if (this.stretchTimeRemaining > 0) {
				this.stretchTimeRemaining--;
				if (this.stretchTimeRemaining <= 0) {
					this.triggerBreak('stretch');
					return;
				}
			}
		}
	}

	formatTimeShort(seconds: number): string {
		if (seconds >= 60) {
			const mins = Math.ceil(seconds / 60);
			return `${mins}m`;
		}
		return `${seconds}s`;
	}

	get statusText(): string {
		if (this.isIdle) return '💤 Away';
		if (this.isPaused) return '⏸ Paused';

		if (this.activeBreak === 'eye') {
			return `👁️ Eye Break ${this.breakTimeRemaining}s`;
		}
		if (this.activeBreak === 'stretch') {
			return `🧍 Stretch ${this.breakTimeRemaining}s`;
		}

		const parts: string[] = [];
		if (settingsStore.settings.eyeEnabled) {
			parts.push(`👁️ ${this.formatTimeShort(this.eyeTimeRemaining)}`);
		}
		if (settingsStore.settings.stretchEnabled) {
			parts.push(`🧍 ${this.formatTimeShort(this.stretchTimeRemaining)}`);
		}

		return parts.length > 0 ? parts.join(' | ') : 'Off';
	}
}

export const timerManager = new TimerManager();
