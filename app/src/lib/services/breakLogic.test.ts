import { describe, expect, it } from 'vitest';
import {
	MAX_EVENTS,
	nextStats,
	normalizeSettings,
	normalizeStats,
	pruneStatsToRetention
} from '$lib/services/breakLogic';
import { DEFAULT_SETTINGS, DEFAULT_STATS, type BreakEvent, type BreakStats } from '$lib/types';

function event(overrides: Partial<BreakEvent> = {}): BreakEvent {
	return {
		id: 'test-1',
		timestamp: 1700000000000,
		type: 'eye',
		action: 'completed',
		durationSec: 20,
		...overrides
	};
}

describe('nextStats', () => {
	it('increments completed counters for an eye break', () => {
		const next = nextStats(DEFAULT_STATS, event({ action: 'completed', type: 'eye' }));
		expect(next.totalCompleted).toBe(1);
		expect(next.eyeCompleted).toBe(1);
		expect(next.eyeSkipped).toBe(0);
		expect(next.stretchCompleted).toBe(0);
	});

	it('counts skipped separately from completed', () => {
		let stats = nextStats(DEFAULT_STATS, event({ action: 'skipped' }));
		stats = nextStats(stats, event({ action: 'completed', id: 'test-2' }));
		expect(stats.totalSkipped).toBe(1);
		expect(stats.totalCompleted).toBe(1);
	});

	it('records one away-session event without touching break counters', () => {
		const stats = nextStats(
			DEFAULT_STATS,
			event({ type: 'away', action: 'completed', durationSec: 240 })
		);
		expect(stats.awayCount).toBe(1);
		expect(stats.totalAwaySec).toBe(240);
		expect(stats.totalCompleted).toBe(0); // away returns are not break completions
		expect(stats.events).toHaveLength(1);
	});

	it('prepends events and caps the log at MAX_EVENTS', () => {
		let stats: BreakStats = DEFAULT_STATS;
		for (let i = 0; i < MAX_EVENTS + 25; i++) {
			stats = nextStats(stats, event({ id: `e${i}` }));
		}
		expect(stats.events).toHaveLength(MAX_EVENTS);
		expect(stats.events[0]?.id).toBe(`e${MAX_EVENTS + 24}`);
	});

	it('treats missing counters as zero (legacy stats on disk)', () => {
		const legacy = { totalCompleted: 3, events: [{ id: 'x' }] } as unknown as BreakStats;
		const next = nextStats(legacy, event({ action: 'completed' }));
		expect(next.totalCompleted).toBe(4);
	});
});

describe('normalizeStats', () => {
	it('returns defaults for garbage input', () => {
		expect(normalizeStats(null).totalCompleted).toBe(0);
		expect(normalizeStats('nonsense').events).toEqual([]);
	});

	it('fills missing fields from older persisted stats', () => {
		const legacy = { totalCompleted: 2, totalSkipped: 1, events: [] };
		const stats = normalizeStats(legacy);
		expect(stats.totalCompleted).toBe(2);
		expect(stats.totalSkipped).toBe(1);
		expect(stats.events).toEqual([]);
	});

	it('keeps known counters', () => {
		const stats = normalizeStats({ totalCompleted: 4, eyeSkipped: 2, awayCount: 7 });
		expect(stats.totalCompleted).toBe(4);
		expect(stats.eyeSkipped).toBe(2);
		expect(stats.awayCount).toBe(7);
	});

	it('drops events recorded under the retired expired action', () => {
		const stats = normalizeStats({
			totalCompleted: 1,
			events: [
				{ id: 'a', timestamp: 1, type: 'eye', action: 'expired', durationSec: 20 },
				{ id: 'b', timestamp: 2, type: 'eye', action: 'completed', durationSec: 20 }
			]
		});
		expect(stats.events.map((e) => e.id)).toEqual(['b']);
	});
});

describe('normalizeSettings', () => {
	it('returns defaults for garbage input', () => {
		expect(normalizeSettings(null)).toEqual(DEFAULT_SETTINGS);
		expect(normalizeSettings('x')).toEqual(DEFAULT_SETTINGS);
	});

	it('coerces corrupt numbers instead of leaking NaN into countdown math', () => {
		const s = normalizeSettings({ ...DEFAULT_SETTINGS, eyeIntervalMin: 'abc' });
		expect(s.eyeIntervalMin).toBe(DEFAULT_SETTINGS.eyeIntervalMin);
		expect(Number.isFinite(s.eyeIntervalMin)).toBe(true);
	});

	it('clamps out-of-range values into sane ranges', () => {
		const s = normalizeSettings({ ...DEFAULT_SETTINGS, soundRepeat: 99, overlayOpacity: 5 });
		expect(s.soundRepeat).toBe(5);
		expect(s.overlayOpacity).toBe(1);
	});

	it('snaps reminder intervals and durations to the offered dropdown options', () => {
		const s = normalizeSettings({
			...DEFAULT_SETTINGS,
			eyeDurationSec: 50,
			stretchIntervalMin: 100,
			stretchDurationSec: 210
		});
		expect(s.eyeDurationSec).toBe(60);
		expect(s.stretchIntervalMin).toBe(90);
		expect(s.stretchDurationSec).toBe(180);
	});

	it('falls back to the default preset for unknown ids', () => {
		const s = normalizeSettings({ ...DEFAULT_SETTINGS, soundPreset: 'foghorn' });
		expect(s.soundPreset).toBe(DEFAULT_SETTINGS.soundPreset);
	});

	it('fills missing newer fields (overlayOpacity) from older persisted settings', () => {
		const legacy = { eyeEnabled: false, eyeIntervalMin: 10 } as Record<string, unknown>;
		const s = normalizeSettings(legacy);
		expect(s.eyeEnabled).toBe(false);
		expect(s.eyeIntervalMin).toBe(10);
		expect(s.overlayOpacity).toBe(DEFAULT_SETTINGS.overlayOpacity);
	});

	it('keeps statsRetention only when it is a known value', () => {
		expect(normalizeSettings({ statsRetention: '1m' }).statsRetention).toBe('1m');
		expect(normalizeSettings({ statsRetention: 'bogus' }).statsRetention).toBe('forever');
		expect(normalizeSettings({}).statsRetention).toBe('forever');
	});

	it('normalizes effectWords properly', () => {
		expect(normalizeSettings({ effectWords: ['rest', ' eyes '] }).effectWords).toEqual(['REST', 'EYES']);
		expect(normalizeSettings({ effectWords: 'pause, breathe' }).effectWords).toEqual(['PAUSE', 'BREATHE']);
		expect(normalizeSettings({ effectWords: [] }).effectWords).toEqual(['BREAK', 'TIME']);
		expect(normalizeSettings({ effectWords: '   ' }).effectWords).toEqual(['BREAK', 'TIME']);
		expect(normalizeSettings({}).effectWords).toEqual(['BREAK', 'TIME']);
	});

	it('normalizes effectWordDurationSec by snapping to 1, 2, or 3', () => {
		expect(normalizeSettings({ effectWordDurationSec: 1 }).effectWordDurationSec).toBe(1);
		expect(normalizeSettings({ effectWordDurationSec: 2 }).effectWordDurationSec).toBe(2);
		expect(normalizeSettings({ effectWordDurationSec: 3 }).effectWordDurationSec).toBe(3);
		expect(normalizeSettings({ effectWordDurationSec: 0 }).effectWordDurationSec).toBe(1);
		expect(normalizeSettings({ effectWordDurationSec: 5 }).effectWordDurationSec).toBe(3);
		expect(normalizeSettings({ effectWordDurationSec: 'invalid' }).effectWordDurationSec).toBe(1);
	});
});

describe('pruneStatsToRetention', () => {
	const NOW = 1_800_000_000_000;
	const day = 24 * 60 * 60 * 1000;

	function statsWith(events: BreakEvent[]): BreakStats {
		let stats = DEFAULT_STATS;
		for (const e of events) stats = nextStats(stats, e);
		return stats;
	}

	it('keeps everything for the forever window', () => {
		const stats = statsWith([event({ timestamp: NOW - 400 * day })]);
		expect(pruneStatsToRetention(stats, 'forever', NOW)).toBe(stats);
	});

	it('drops events older than the window and recomputes counters from survivors', () => {
		const stats = statsWith([
			event({ id: 'old', type: 'eye', action: 'completed', timestamp: NOW - 40 * day }),
			event({ id: 'new', type: 'stretch', action: 'completed', timestamp: NOW - 2 * day })
		]);
		const pruned = pruneStatsToRetention(stats, '1m', NOW);
		expect(pruned.events.map((e) => e.id)).toEqual(['new']);
		expect(pruned.totalCompleted).toBe(1);
		expect(pruned.stretchCompleted).toBe(1);
		expect(pruned.eyeCompleted).toBe(0);
	});

	it('honors the 1-week window boundary', () => {
		const stats = statsWith([
			event({ id: 'edge', timestamp: NOW - 7 * day }),
			event({ id: 'just-inside', timestamp: NOW - 7 * day + 1000 }),
			event({ id: 'outside', timestamp: NOW - 8 * day })
		]);
		const pruned = pruneStatsToRetention(stats, '1w', NOW);
		expect(pruned.events.map((e) => e.id)).toEqual(['just-inside', 'edge']);
	});

	it('recomputes away counters within the window', () => {
		const stats = statsWith([
			event({ id: 'a1', type: 'away', action: 'completed', durationSec: 300, timestamp: NOW - 200 * day }),
			event({ id: 'a2', type: 'away', action: 'completed', durationSec: 60, timestamp: NOW - day })
		]);
		const pruned = pruneStatsToRetention(stats, '6m', NOW);
		expect(pruned.awayCount).toBe(1);
		expect(pruned.totalAwaySec).toBe(60);
	});
});
