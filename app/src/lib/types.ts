export type SoundPreset =
	| 'soft-bell'
	| 'gentle-chime'
	| 'calm-drop'
	| 'calm-piano'
	| 'morning-dew'
	| 'still-water';

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

export interface HudSize {
	width: number;
	height: number;
}

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
	idleThresholdSec: number; // default 300 (5 min)
	awayResetMin: number; // default 10 min
	hudSize?: HudSize; // custom compact HUD size (from resize grip)
}

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
	awayResetMin: 10
};

export type BreakType = 'eye' | 'stretch';

/**
 * 'completed': user clicked the checkmark.
 * 'skipped': user clicked the X.
 * 'expired': countdown reached zero without user action.
 */
export type BreakAction = 'completed' | 'skipped' | 'expired';

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
	totalExpired: number;
	eyeCompleted: number;
	eyeSkipped: number;
	eyeExpired: number;
	stretchCompleted: number;
	stretchSkipped: number;
	stretchExpired: number;
	events: BreakEvent[];
}

export const DEFAULT_STATS: BreakStats = {
	totalCompleted: 0,
	totalSkipped: 0,
	totalExpired: 0,
	eyeCompleted: 0,
	eyeSkipped: 0,
	eyeExpired: 0,
	stretchCompleted: 0,
	stretchSkipped: 0,
	stretchExpired: 0,
	events: []
};

/** Command sent from a non-master window to the master (overlay) window. */
export interface TimerCommand {
	action: 'trigger-break' | 'toggle-pause' | 'complete-break' | 'skip-break' | 'reset-timers';
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
	blinkCount: number;
	currentExerciseIndex: number;
	awayStartTime: number | null;
	eyeTargetTimestamp: number | null;
	stretchTargetTimestamp: number | null;
	breakTargetTimestamp: number | null;
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
