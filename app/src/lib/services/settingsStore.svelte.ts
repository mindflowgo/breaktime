import {
	DEFAULT_STATS,
	type BreakEvent,
	type BreakStats,
	type Settings
} from '$lib/types';
import { nextStats, normalizeSettings, normalizeStats, pruneStatsToRetention } from '$lib/services/breakLogic';
import { EV_SETTINGS_CHANGED, EV_STATS_CHANGED, EV_STATS_RESET } from '$lib/services/events';
import {
	emitTauriEvent,
	listenTauriEvent,
	tauriInvoke,
	tauriInvokeStrict
} from './windowManager';

const SETTINGS_KEY = 'settings';
const STATS_KEY = 'stats';
const LOCAL_STORAGE_SETTINGS = 'breaktime_settings';
const LOCAL_STORAGE_STATS = 'breaktime_stats';

/** Identifies this window's emissions so its own Tauri event echoes are ignored. */
const SENDER_ID =
	typeof crypto !== 'undefined' && 'randomUUID' in crypto
		? crypto.randomUUID()
		: `w-${Math.random().toString(36).slice(2)}`;

interface Envelope<T> {
	sender: string;
	data: T;
}

class SettingsStore {
	settings = $state<Settings>({ ...normalizeSettings(undefined) });
	stats = $state<BreakStats>({ ...DEFAULT_STATS });
	isLoaded = $state(false);

	/** Last persistence error, surfaced in the settings UI. */
	lastError = $state<string | null>(null);

	private initialized = false;
	/** Bumped by destroy(); late-resolving unlistens from an older generation self-remove. */
	private generation = 0;
	private unlistenFns: (() => void)[] = [];
	private settingsChangeListeners: Array<(settings: Settings) => void> = [];
	private statsResetListeners: Array<() => void> = [];

	onSettingsChange(listener: (settings: Settings) => void): () => void {
		this.settingsChangeListeners.push(listener);
		return () => {
			this.settingsChangeListeners = this.settingsChangeListeners.filter((l) => l !== listener);
		};
	}

	onStatsReset(listener: () => void): () => void {
		this.statsResetListeners.push(listener);
		return () => {
			this.statsResetListeners = this.statsResetListeners.filter((l) => l !== listener);
		};
	}

	/** Registers an unlisten promise; if destroyed before it resolves, unlisten immediately. */
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

	async init() {
		if (this.initialized) return;
		this.initialized = true;

		await this.load();

		// Cross-window sync: apply only events from the OTHER window — Tauri's
		// emit() delivers back to the sender, and the sender already applied
		// the change locally (and notified its own listeners) before emitting.
		this.trackUnlisten(
			listenTauriEvent<Envelope<Settings>>(EV_SETTINGS_CHANGED, (payload) => {
				if (!payload || payload.sender === SENDER_ID) return;
				const next = normalizeSettings({ ...this.settings, ...payload.data });
				if (JSON.stringify(next) === JSON.stringify(this.settings)) return;
				this.settings = next;
				for (const listener of this.settingsChangeListeners) {
					try {
						listener(this.settings);
					} catch (e) {
						console.error('[SettingsStore] settings change listener error:', e);
					}
				}
			})
		);

		this.trackUnlisten(
			listenTauriEvent<Envelope<BreakStats>>(EV_STATS_CHANGED, (payload) => {
				if (!payload || payload.sender === SENDER_ID) return;
				this.stats = normalizeStats(payload.data);
			})
		);

		this.trackUnlisten(
			listenTauriEvent<Envelope<BreakStats>>(EV_STATS_RESET, (payload) => {
				if (payload && payload.sender === SENDER_ID) return;
				this.stats = { ...DEFAULT_STATS, events: [] };
				for (const listener of this.statsResetListeners) {
					try {
						listener();
					} catch (e) {
						console.error('[SettingsStore] stats reset listener error:', e);
					}
				}
			})
		);
	}

	async load() {
		let loadedSettings: unknown = null;
		let loadedStats: unknown = null;

		// 1. Try Tauri backend settings command (tauriInvoke resolves null on failure)
		const rawSettings = await tauriInvoke<Settings>('get_setting', { key: SETTINGS_KEY });
		if (rawSettings && typeof rawSettings === 'object' && Object.keys(rawSettings).length > 0) {
			loadedSettings = rawSettings;
		}
		const rawStats = await tauriInvoke<BreakStats>('get_setting', { key: STATS_KEY });
		if (rawStats && typeof rawStats === 'object' && Object.keys(rawStats).length > 0) {
			loadedStats = rawStats;
		}

		// 2. Fall back to localStorage (e.g. plain-browser dev mode)
		if (typeof localStorage !== 'undefined') {
			try {
				if (!loadedSettings) {
					const localS = localStorage.getItem(LOCAL_STORAGE_SETTINGS);
					if (localS) loadedSettings = JSON.parse(localS);
				}
				if (!loadedStats) {
					const localSt = localStorage.getItem(LOCAL_STORAGE_STATS);
					if (localSt) loadedStats = JSON.parse(localSt);
				}
			} catch (err) {
				console.warn('[SettingsStore] Error reading localStorage:', err);
			}
		}

		this.settings = normalizeSettings(loadedSettings);
		if (loadedStats) {
			this.stats = normalizeStats(loadedStats);
			// Keep the stats file inside the chosen retention window.
			this.stats = pruneStatsToRetention(this.stats, this.settings.statsRetention);
		}

		// Reconcile cache in localStorage
		if (typeof localStorage !== 'undefined') {
			try {
				localStorage.setItem(LOCAL_STORAGE_SETTINGS, JSON.stringify(this.settings));
				localStorage.setItem(LOCAL_STORAGE_STATS, JSON.stringify(this.stats));
			} catch {}
		}

		this.isLoaded = true;
	}

	async updateSettings(partial: Partial<Settings>) {
		if (!this.isLoaded) await this.load();

		const previous = this.settings;
		this.settings = normalizeSettings({ ...this.settings, ...partial });

		// Retention shortened → truncate the stats file to the new window.
		if (this.settings.statsRetention !== previous.statsRetention) {
			const pruned = pruneStatsToRetention(this.stats, this.settings.statsRetention);
			if (pruned.events.length !== this.stats.events.length) {
				this.stats = pruned;
				if (typeof localStorage !== 'undefined') {
					try {
						localStorage.setItem(LOCAL_STORAGE_STATS, JSON.stringify(pruned));
					} catch {}
				}
				await this.persistWithRetry(STATS_KEY, pruned, 'Break statistics');
				await emitTauriEvent(EV_STATS_CHANGED, { sender: SENDER_ID, data: pruned });
			}
		}

		// Notify local listeners immediately (the sender never re-applies its own echo)
		for (const listener of this.settingsChangeListeners) {
			try {
				listener(this.settings);
			} catch (e) {
				console.error('[SettingsStore] settings change listener error:', e);
			}
		}

		if (typeof localStorage !== 'undefined') {
			try {
				localStorage.setItem(LOCAL_STORAGE_SETTINGS, JSON.stringify(this.settings));
			} catch {}
		}

		await this.persistWithRetry(SETTINGS_KEY, this.settings, 'Settings');

		await emitTauriEvent(EV_SETTINGS_CHANGED, { sender: SENDER_ID, data: this.settings });
	}

	async recordBreak(eventData: Omit<BreakEvent, 'id' | 'timestamp'> & { timestamp?: number }) {
		if (!this.isLoaded) await this.load();

		const newEvent: BreakEvent = {
			id: `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
			timestamp: eventData.timestamp ?? Date.now(),
			type: eventData.type,
			action: eventData.action,
			durationSec: eventData.durationSec
		};

		const next = nextStats(this.stats, newEvent);
		this.stats = next;

		if (typeof localStorage !== 'undefined') {
			try {
				localStorage.setItem(LOCAL_STORAGE_STATS, JSON.stringify(next));
			} catch {}
		}

		await this.persistWithRetry(STATS_KEY, next, 'Break statistics');

		await emitTauriEvent(EV_STATS_CHANGED, { sender: SENDER_ID, data: next });
	}

	async resetStats() {
		if (!this.isLoaded) await this.load();

		const emptyStats: BreakStats = { ...DEFAULT_STATS, events: [] };
		this.stats = emptyStats;

		for (const listener of this.statsResetListeners) {
			try {
				listener();
			} catch (e) {
				console.error('[SettingsStore] stats reset listener error:', e);
			}
		}

		if (typeof localStorage !== 'undefined') {
			try {
				localStorage.setItem(LOCAL_STORAGE_STATS, JSON.stringify(emptyStats));
			} catch {}
		}

		await this.persistWithRetry(STATS_KEY, emptyStats, 'Statistics');

		await emitTauriEvent(EV_STATS_CHANGED, { sender: SENDER_ID, data: emptyStats });
		await emitTauriEvent(EV_STATS_RESET, { sender: SENDER_ID, data: emptyStats });
	}

	/** Persist with a single retry; surface persistent failures via lastError. */
	private async persistWithRetry(key: string, value: unknown, label: string) {
		const write = () => tauriInvokeStrict('set_setting', { key, value });
		try {
			try {
				await write();
			} catch {
				await write(); // one retry for transient IO
			}
			this.lastError = null;
		} catch (err) {
			console.error(`[SettingsStore] Failed to persist ${key}:`, err);
			this.lastError = `${label} could not be saved to disk.`;
		}
	}

	destroy() {
		this.generation++;
		for (const unlisten of this.unlistenFns) {
			try {
				unlisten();
			} catch {}
		}
		this.unlistenFns = [];
		this.settingsChangeListeners = [];
		this.statsResetListeners = [];
		this.initialized = false;
		this.isLoaded = false;
	}
}

export const settingsStore = new SettingsStore();
