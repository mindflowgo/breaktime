import {
	DEFAULT_SETTINGS,
	DEFAULT_STATS,
	type BreakEvent,
	type BreakStats,
	type Settings
} from '$lib/types';
import { nextStats, normalizeStats } from '$lib/services/breakLogic';
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

class SettingsStore {
	settings = $state<Settings>({ ...DEFAULT_SETTINGS });
	stats = $state<BreakStats>({ ...DEFAULT_STATS });
	isLoaded = $state(false);

	/** Last persistence error, surfaced in the settings UI. */
	lastError = $state<string | null>(null);

	private initialized = false;
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

	async init() {
		if (this.initialized) return;
		this.initialized = true;

		await this.load();

		// Listen to cross-window sync events
		const unlistenSettings = listenTauriEvent<Settings>('breaktime:settings-changed', (newSettings) => {
			if (newSettings && typeof newSettings === 'object') {
				this.settings = { ...this.settings, ...newSettings };
				for (const listener of this.settingsChangeListeners) {
					try {
						listener(this.settings);
					} catch (e) {
						console.error('[SettingsStore] Error in settings change listener:', e);
					}
				}
			}
		});
		unlistenSettings.then((fn) => this.unlistenFns.push(fn));

		const unlistenStats = listenTauriEvent<unknown>('breaktime:stats-changed', (newStats) => {
			this.stats = normalizeStats(newStats);
		});
		unlistenStats.then((fn) => this.unlistenFns.push(fn));

		const unlistenReset = listenTauriEvent('breaktime:stats-reset', () => {
			this.stats = { ...DEFAULT_STATS, events: [] };
			for (const listener of this.statsResetListeners) {
				try {
					listener();
				} catch (e) {
					console.error('[SettingsStore] Error in stats reset listener:', e);
				}
			}
		});
		unlistenReset.then((fn) => this.unlistenFns.push(fn));
	}

	async load() {
		let loadedSettings: Partial<Settings> | null = null;
		let loadedStats: Partial<BreakStats> | null = null;

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

		if (loadedSettings) {
			this.settings = { ...DEFAULT_SETTINGS, ...loadedSettings };
		}
		if (loadedStats) {
			this.stats = normalizeStats(loadedStats);
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

		this.settings = { ...this.settings, ...partial };

		// Notify local listeners immediately
		for (const listener of this.settingsChangeListeners) {
			try {
				listener(this.settings);
			} catch (e) {
				console.error('[SettingsStore] Error in settings change listener:', e);
			}
		}

		// Save to localStorage
		if (typeof localStorage !== 'undefined') {
			try {
				localStorage.setItem(LOCAL_STORAGE_SETTINGS, JSON.stringify(this.settings));
			} catch {}
		}

		// Save to Tauri backend (surface failures to the user)
		try {
			await tauriInvokeStrict('set_setting', { key: SETTINGS_KEY, value: this.settings });
			this.lastError = null;
		} catch (err) {
			console.error('[SettingsStore] Failed to persist settings:', err);
			this.lastError = 'Settings could not be saved to disk.';
		}

		// Broadcast cross-window
		await emitTauriEvent('breaktime:settings-changed', this.settings);
	}

	async recordBreak(eventData: Omit<BreakEvent, 'id' | 'timestamp'>) {
		if (!this.isLoaded) await this.load();

		const newEvent: BreakEvent = {
			id: `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
			timestamp: Date.now(),
			...eventData
		};

		const nextStatsResult = nextStats(this.stats, newEvent);
		this.stats = nextStatsResult;

		// Save to localStorage
		if (typeof localStorage !== 'undefined') {
			try {
				localStorage.setItem(LOCAL_STORAGE_STATS, JSON.stringify(nextStatsResult));
			} catch {}
		}

		// Save to Tauri (surface failures to the user)
		try {
			await tauriInvokeStrict('set_setting', { key: STATS_KEY, value: nextStatsResult });
			this.lastError = null;
		} catch (err) {
			console.error('[SettingsStore] Failed to persist stats:', err);
			this.lastError = 'Break statistics could not be saved to disk.';
		}

		// Broadcast cross-window
		await emitTauriEvent('breaktime:stats-changed', nextStatsResult);
	}

	async resetStats() {
		if (!this.isLoaded) await this.load();

		const emptyStats: BreakStats = { ...DEFAULT_STATS, events: [] };
		this.stats = emptyStats;

		// Notify local listeners immediately
		for (const listener of this.statsResetListeners) {
			try {
				listener();
			} catch (e) {
				console.error('[SettingsStore] Error in stats reset listener:', e);
			}
		}

		if (typeof localStorage !== 'undefined') {
			try {
				localStorage.setItem(LOCAL_STORAGE_STATS, JSON.stringify(emptyStats));
			} catch {}
		}

		try {
			await tauriInvokeStrict('set_setting', { key: STATS_KEY, value: emptyStats });
			this.lastError = null;
		} catch (err) {
			console.error('[SettingsStore] Failed to persist stats reset:', err);
			this.lastError = 'Statistics could not be reset on disk.';
		}

		await emitTauriEvent('breaktime:stats-changed', emptyStats);
		await emitTauriEvent('breaktime:stats-reset', emptyStats);
	}

	destroy() {
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
