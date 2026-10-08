import { describe, expect, it } from 'vitest';
import {
	MAX_EVENTS,
	blinkCountFor,
	exerciseIndexFor,
	nextStats,
	normalizeStats
} from '$lib/services/breakLogic';
import { DEFAULT_STATS, type BreakEvent, type BreakStats } from '$lib/types';

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
		const stats = normalizeStats({ totalExpired: 4, eyeSkipped: 2 });
		expect(stats.totalExpired).toBe(4);
		expect(stats.eyeSkipped).toBe(2);
	});
});

describe('guided progress helpers', () => {
	it('spreads 10 blinks across the break', () => {
		expect(blinkCountFor(0, 20)).toBe(1);
		expect(blinkCountFor(10, 20)).toBe(6);
		expect(blinkCountFor(19, 20)).toBe(10);
		expect(blinkCountFor(100, 20)).toBe(10);
	});

	it('never divides by zero', () => {
		expect(blinkCountFor(5, 0)).toBe(1);
		expect(exerciseIndexFor(5, 0)).toBe(0);
	});

	it('rotates 4 stretch exercises per quarter', () => {
		expect(exerciseIndexFor(0, 60)).toBe(0);
		expect(exerciseIndexFor(15, 60)).toBe(1);
		expect(exerciseIndexFor(30, 60)).toBe(2);
		expect(exerciseIndexFor(45, 60)).toBe(3);
		expect(exerciseIndexFor(59, 60)).toBe(3);
	});
});
