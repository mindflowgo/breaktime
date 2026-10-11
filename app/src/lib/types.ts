export type SoundPreset =
	| 'soft-bell'
	| 'gentle-chime'
	| 'calm-drop'
	| 'calm-piano'
	| 'morning-dew'
	| 'still-water'
	| 'custom';

export interface SoundPresetInfo {
	id: SoundPreset;
	name: string;
	style: string;
	description: string;
}

export const SOUND_PRESETS: SoundPresetInfo[] = [
	{
		id: 'soft-bell',
		name: 'Soft bell',
		style: 'Short gentle alert',
		description: 'Gentle exponential chime with soft harmonic resonance'
	},
	{
		id: 'gentle-chime',
		name: 'Gentle chime',
		style: 'Light notification',
		description: 'Two-tone melodic harmonic chime'
	},
	{
		id: 'calm-drop',
		name: 'Calm drop',
		style: 'Soft minimal tone',
		description: 'Soft droplet marimba tone with organic decay'
	},
	{
		id: 'calm-piano',
		name: 'Calm piano',
		style: 'Four-note piano phrase',
		description: 'Peaceful four-note piano chord phrase'
	},
	{
		id: 'morning-dew',
		name: 'Morning dew',
		style: 'Gentle rising five-note melody',
		description: 'Ascending 5-note pentatonic melody'
	},
	{
		id: 'still-water',
		name: 'Still water',
		style: 'Slower, deeper calming melody',
		description: 'Slower, deeper calming ambient resonance'
	}
];

/**
 * Default compact HUD dimensions. Must stay in sync with IDLE_HUD_WIDTH /
 * IDLE_HUD_HEIGHT in tauri/src/window_state.rs (Rust applies them at startup
 * before the frontend runs).
 */
export const IDLE_HUD_WIDTH = 104;
export const IDLE_HUD_HEIGHT = 48;

export interface Settings {
	eyeEnabled: boolean;
	eyeIntervalMin: number;
	eyeDurationSec: number;
	stretchEnabled: boolean;
	stretchIntervalMin: number;
	stretchDurationSec: number;
	soundPreset: SoundPreset;
	soundVolume: number; // 0 to 1
	soundFadeIn: boolean;
	soundRepeat: number; // 1 to 5 phrase repetitions
	/** File name (in the app data dir) of the uploaded custom sound. */
	customSoundFile?: string;
	idleThresholdSec: number; // default 300 (5 min)
	awayResetMin: number; // default 10 min
	overlayOpacity: number; // 0.2 to 1.0 (default 0.85)
	hud2x: boolean; // 2× HUD mode (double-size eye & window)
	/** How far back to keep break statistics; older events are pruned. */
	statsRetention: StatsRetention;
}

export type StatsRetention = '1w' | '1m' | '6m' | 'forever';

export const STATS_RETENTION_MS: Record<Exclude<StatsRetention, 'forever'>, number> = {
	'1w': 7 * 24 * 60 * 60 * 1000,
	'1m': 30 * 24 * 60 * 60 * 1000,
	'6m': 180 * 24 * 60 * 60 * 1000
};

export const DEFAULT_SETTINGS: Settings = {
	eyeEnabled: true,
	eyeIntervalMin: 20,
	eyeDurationSec: 20,
	stretchEnabled: true,
	stretchIntervalMin: 45,
	stretchDurationSec: 60,
	soundPreset: 'gentle-chime',
	soundVolume: 0.6,
	soundFadeIn: true,
	soundRepeat: 1,
	idleThresholdSec: 300,
	awayResetMin: 10,
	overlayOpacity: 0.85,
	hud2x: false,
	statsRetention: 'forever'
};

export type BreakType = 'eye' | 'stretch' | 'away';

/**
 * 'completed': the break ran (checkmark, Enter, or natural expiry — the user
 * is assumed to have done the exercise unless they clicked Skip).
 * 'skipped': user clicked the X (or Esc).
 */
export type BreakAction = 'completed' | 'skipped';

export interface BreakEvent {
	id: string;
	timestamp: number;
	type: BreakType;
	action: BreakAction;
	durationSec: number;
}

export interface BreakStats {
	totalCompleted: number;
	totalSkipped: number;
	eyeCompleted: number;
	eyeSkipped: number;
	stretchCompleted: number;
	stretchSkipped: number;
	awayCount?: number;
	totalAwaySec?: number;
	events: BreakEvent[];
}

export const DEFAULT_STATS: BreakStats = {
	totalCompleted: 0,
	totalSkipped: 0,
	eyeCompleted: 0,
	eyeSkipped: 0,
	stretchCompleted: 0,
	stretchSkipped: 0,
	awayCount: 0,
	totalAwaySec: 0,
	events: []
};

/** Command sent from a non-master window to the master (overlay) window. */
export interface TimerCommand {
	action:
		| 'trigger-break'
		| 'toggle-pause'
		| 'complete-break'
		| 'skip-break'
		| 'reset-timers'
		| 'reset-timer';
	type?: BreakType;
}

/** Authoritative timer state broadcast from the master (overlay) window. */
export interface TimerStateSync {
	isPaused: boolean;
	isIdle: boolean;
	activeBreak: BreakType | null;
	eyeTimeRemaining: number;
	stretchTimeRemaining: number;
	breakTimeRemaining: number;
	awayStartTime: number | null;
	eyeTargetTimestamp: number | null;
	stretchTargetTimestamp: number | null;
	breakTargetTimestamp: number | null;
	/** Send time; mirrors drop payloads older than the last one seen. */
	timestamp: number;
}

export interface WindowBounds {
	width: number;
	height: number;
	x?: number;
	y?: number;
}

export interface AppWindowState {
	mode: 'normal' | 'minimalist';
	normal: WindowBounds;
	minimalist: WindowBounds;
}
