import {
	DEFAULT_SETTINGS,
	DEFAULT_STATS,
	EYE_DURATION_SEC_OPTIONS,
	EYE_INTERVAL_MIN_OPTIONS,
	SOUND_PRESETS,
	STRETCH_DURATION_SEC_OPTIONS,
	STRETCH_INTERVAL_MIN_OPTIONS,
	STATS_RETENTION_MS,
	type BreakEvent,
	type BreakStats,
	type Settings,
	type StatsRetention
} from '$lib/types';

/** Max number of break history events kept on disk. */
export const MAX_EVENTS = 100;

/**
 * Stats older than the selected retention are pruned and all counters are
 * recomputed from the surviving events, so the stats file only ever
 * describes the chosen window of time.
 */
export function pruneStatsToRetention(
	stats: BreakStats,
	retention: StatsRetention,
	now: number = Date.now()
): BreakStats {
	if (retention === 'forever') return stats;
	const cutoff = now - STATS_RETENTION_MS[retention];
	const kept = (stats.events || []).filter((e) => (e?.timestamp ?? 0) >= cutoff);

	// Events are stored newest-first; fold oldest → newest so the rebuilt
	// list keeps that order and counters derive purely from the window.
	let pruned: BreakStats = { ...DEFAULT_STATS, events: [] };
	for (let i = kept.length - 1; i >= 0; i--) {
		pruned = nextStats(pruned, kept[i]);
	}
	return pruned;
}

/**
 * Pure fold: stats after applying one new event. Away sessions are recorded
 * as a single event on return (type 'away', action 'completed', measured
 * duration), so awayCount/totalAwaySec stay one-per-session.
 */
export function nextStats(stats: BreakStats, event: BreakEvent): BreakStats {
	const isEye = event.type === 'eye';
	const isStretch = event.type === 'stretch';
	const isAway = event.type === 'away';
	const action = event.action;

	return {
		totalCompleted: (stats.totalCompleted || 0) + (!isAway && action === 'completed' ? 1 : 0),
		totalSkipped: (stats.totalSkipped || 0) + (action === 'skipped' ? 1 : 0),
		eyeCompleted: (stats.eyeCompleted || 0) + (isEye && action === 'completed' ? 1 : 0),
		eyeSkipped: (stats.eyeSkipped || 0) + (isEye && action === 'skipped' ? 1 : 0),
		stretchCompleted: (stats.stretchCompleted || 0) + (isStretch && action === 'completed' ? 1 : 0),
		stretchSkipped: (stats.stretchSkipped || 0) + (isStretch && action === 'skipped' ? 1 : 0),
		awayCount: (stats.awayCount || 0) + (isAway ? 1 : 0),
		totalAwaySec: (stats.totalAwaySec || 0) + (isAway ? event.durationSec || 0 : 0),
		events: [event, ...(stats.events || [])].slice(0, MAX_EVENTS)
	};
}

/** Normalizes a partially-loaded stats object (disk / cross-window payload).
 *  Events recorded under the retired 'expired' action are dropped. */
export function normalizeStats(raw: unknown): BreakStats {
	if (!raw || typeof raw !== 'object') {
		return { ...DEFAULT_STATS, events: [] };
	}
	const partial = raw as Partial<BreakStats>;
	return {
		totalCompleted: Number(partial.totalCompleted) || 0,
		totalSkipped: Number(partial.totalSkipped) || 0,
		eyeCompleted: Number(partial.eyeCompleted) || 0,
		eyeSkipped: Number(partial.eyeSkipped) || 0,
		stretchCompleted: Number(partial.stretchCompleted) || 0,
		stretchSkipped: Number(partial.stretchSkipped) || 0,
		awayCount: Number(partial.awayCount) || 0,
		totalAwaySec: Number(partial.totalAwaySec) || 0,
		events: Array.isArray(partial.events)
			? partial.events.filter((e) => e && (e.action as string) !== 'expired')
			: []
	};
}

/**
 * Normalizes a partially-loaded settings object: clamps numbers to sane
 * ranges, validates the sound preset, defaults unknown fields. Prevents a
 * corrupt value on disk (e.g. eyeIntervalMin: "abc" → NaN) from poisoning
 * countdown math.
 */
export function normalizeSettings(raw: unknown): Settings {
	const fallback = DEFAULT_SETTINGS;
	if (!raw || typeof raw !== 'object') {
		return { ...fallback };
	}
	const s = raw as Record<string, unknown>;

	const num = (v: unknown, def: number, min: number, max: number): number => {
		const n = Number(v);
		return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : def;
	};
	const bool = (v: unknown, def: boolean): boolean => (typeof v === 'boolean' ? v : def);

	/** Reminder schedules/durations are discrete dropdown choices; older or
	 *  hand-edited values snap to the nearest offered option. */
	const snap = (v: unknown, options: readonly number[], fallback: number): number => {
		const n = Number(v);
		if (!Number.isFinite(n)) return fallback;
		return options.reduce((best, o) => (Math.abs(o - n) < Math.abs(best - n) ? o : best));
	};

	const isKnownPreset =
		s.soundPreset === 'custom' || SOUND_PRESETS.some((p) => p.id === s.soundPreset);
	const preset = isKnownPreset
		? (s.soundPreset as Settings['soundPreset'])
		: fallback.soundPreset;

	// The custom sound file name is only honored alongside the custom preset.
	const customSoundFile =
		s.customSoundFile && typeof s.customSoundFile === 'string'
			? s.customSoundFile
			: fallback.customSoundFile;

	const retention: StatsRetention =
		s.statsRetention === '1w' || s.statsRetention === '1m' || s.statsRetention === '6m'
			? s.statsRetention
			: 'forever';

	return {
		eyeEnabled: bool(s.eyeEnabled, fallback.eyeEnabled),
		eyeIntervalMin: snap(s.eyeIntervalMin, EYE_INTERVAL_MIN_OPTIONS, fallback.eyeIntervalMin),
		eyeDurationSec: snap(s.eyeDurationSec, EYE_DURATION_SEC_OPTIONS, fallback.eyeDurationSec),
		stretchEnabled: bool(s.stretchEnabled, fallback.stretchEnabled),
		stretchIntervalMin: snap(
			s.stretchIntervalMin,
			STRETCH_INTERVAL_MIN_OPTIONS,
			fallback.stretchIntervalMin
		),
		stretchDurationSec: snap(
			s.stretchDurationSec,
			STRETCH_DURATION_SEC_OPTIONS,
			fallback.stretchDurationSec
		),
		soundPreset: preset,
		customSoundFile: preset === 'custom' ? customSoundFile : undefined,
		soundVolume: num(s.soundVolume, fallback.soundVolume, 0.05, 1),
		soundFadeIn: bool(s.soundFadeIn, fallback.soundFadeIn),
		soundRepeat: num(s.soundRepeat, fallback.soundRepeat, 1, 5),
		idleThresholdSec: num(s.idleThresholdSec, fallback.idleThresholdSec, 30, 3600),
		overlayOpacity: num(s.overlayOpacity, fallback.overlayOpacity, 0.2, 1),
		hud2x: bool(s.hud2x, fallback.hud2x),
		statsRetention: retention
	};
}
