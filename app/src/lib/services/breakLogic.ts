import { DEFAULT_STATS, type BreakEvent, type BreakStats } from '$lib/types';

/** Max number of break history events kept on disk. */
export const MAX_EVENTS = 100;

/** Guided blink counter: 10 counted blinks spread evenly across the break. */
export function blinkCountFor(elapsedSec: number, totalSec: number): number {
	if (totalSec <= 0) return 1;
	return Math.min(10, Math.floor((elapsedSec / totalSec) * 10) + 1);
}

/** Guided stretch step: one of 4 exercises per quarter of the break. */
export function exerciseIndexFor(elapsedSec: number, totalSec: number): number {
	if (totalSec <= 0) return 0;
	return Math.min(3, Math.floor((elapsedSec / totalSec) * 4));
}

/** Pure fold: stats after applying one new break event. */
export function nextStats(stats: BreakStats, event: BreakEvent): BreakStats {
	const isEye = event.type === 'eye';
	const action = event.action;

	return {
		totalCompleted: (stats.totalCompleted || 0) + (action === 'completed' ? 1 : 0),
		totalSkipped: (stats.totalSkipped || 0) + (action === 'skipped' ? 1 : 0),
		totalExpired: (stats.totalExpired || 0) + (action === 'expired' ? 1 : 0),
		eyeCompleted: (stats.eyeCompleted || 0) + (isEye && action === 'completed' ? 1 : 0),
		eyeSkipped: (stats.eyeSkipped || 0) + (isEye && action === 'skipped' ? 1 : 0),
		eyeExpired: (stats.eyeExpired || 0) + (isEye && action === 'expired' ? 1 : 0),
		stretchCompleted: (stats.stretchCompleted || 0) + (!isEye && action === 'completed' ? 1 : 0),
		stretchSkipped: (stats.stretchSkipped || 0) + (!isEye && action === 'skipped' ? 1 : 0),
		stretchExpired: (stats.stretchExpired || 0) + (!isEye && action === 'expired' ? 1 : 0),
		events: [event, ...(stats.events || [])].slice(0, MAX_EVENTS)
	};
}

/** Normalizes a partially-loaded stats object (disk / cross-window payload). */
export function normalizeStats(raw: unknown): BreakStats {
	if (!raw || typeof raw !== 'object') {
		return { ...DEFAULT_STATS, events: [] };
	}
	const partial = raw as Partial<BreakStats>;
	return {
		totalCompleted: Number(partial.totalCompleted) || 0,
		totalSkipped: Number(partial.totalSkipped) || 0,
		totalExpired: Number(partial.totalExpired) || 0,
		eyeCompleted: Number(partial.eyeCompleted) || 0,
		eyeSkipped: Number(partial.eyeSkipped) || 0,
		eyeExpired: Number(partial.eyeExpired) || 0,
		stretchCompleted: Number(partial.stretchCompleted) || 0,
		stretchSkipped: Number(partial.stretchSkipped) || 0,
		stretchExpired: Number(partial.stretchExpired) || 0,
		events: Array.isArray(partial.events) ? partial.events : []
	};
}
