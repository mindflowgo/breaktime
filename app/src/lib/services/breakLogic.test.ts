import { describe, expect, it } from 'vitest';
import { MAX_EVENTS, nextStats, normalizeSettings, normalizeStats } from '$lib/services/breakLogic';
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

	it('counts skipped and expired separately', () => {
		let stats = nextStats(DEFAULT_STATS, event({ action: 'skipped' }));
		stats = nextStats(stats, event({ action: 'expired', id: 'test-2' }));
		expect(stats.totalSkipped).toBe(1);
		expect(stats.totalExpired).toBe(1);
		expect(stats.totalCompleted).toBe(0);
	});

	it('attributes expired breaks to the right type', () => {
		const stats = nextStats(DEFAULT_STATS, event({ type: 'stretch', action: 'expired' }));
		expect(stats.stretchExpired).toBe(1);
		expect(stats.eyeExpired).toBe(0);
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
		const next = nextStats(legacy, event({ action: 'expired' }));
		expect(next.totalCompleted).toBe(3);
		expect(next.totalExpired).toBe(1);
	});
});

describe('normalizeStats', () => {
	it('returns defaults for garbage input', () => {
		expect(normalizeStats(null).totalCompleted).toBe(0);
		expect(normalizeStats('nonsense').events).toEqual([]);
	});

	it('fills missing expired counters from older persisted stats', () => {
		const legacy = { totalCompleted: 2, totalSkipped: 1, events: [] };
		const stats = normalizeStats(legacy);
		expect(stats.totalExpired).toBe(0);
		expect(stats.totalCompleted).toBe(2);
		expect(stats.events).toEqual([]);
	});

	it('keeps known counters', () => {
		const stats = normalizeStats({ totalExpired: 4, eyeSkipped: 2, awayCount: 7 });
		expect(stats.totalExpired).toBe(4);
		expect(stats.eyeSkipped).toBe(2);
		expect(stats.awayCount).toBe(7);
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
});
