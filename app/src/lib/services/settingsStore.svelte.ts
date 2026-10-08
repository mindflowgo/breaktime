import {
	DEFAULT_SETTINGS,
	DEFAULT_STATS,
	type BreakEvent,
	type BreakStats,
	type Settings
} from '$lib/types';
import { emitTauriEvent, listenTauriEvent, tauriInvoke } from './windowManager';

const SETTINGS_KEY = 'settings';
const STATS_KEY = 'stats';
const LOCAL_STORAGE_SETTINGS = 'breaktime_settings';
const LOCAL_STORAGE_STATS = 'breaktime_stats';

class SettingsStore {
	settings = $state<Settings>({ ...DEFAULT_SETTINGS });
	stats = $state<BreakStats>({ ...DEFAULT_STATS });
	isLoaded = $state(false);

	private initialized = false;

	async init() {
		if (this.initialized) return;
		this.initialized = true;

		await this.load();

		// Listen to cross-window sync events
		listenTauriEvent<Settings>('breaktime:settings-changed', (newSettings) => {
			if (newSettings && typeof newSettings === 'object') {
				this.settings = { ...this.settings, ...newSettings };
			}
		});

		listenTauriEvent<BreakStats>('breaktime:stats-changed', (newStats) => {
			if (newStats && typeof newStats === 'object') {
				this.stats = { ...newStats };
			}
		});
	}

	async load() {
		let loadedSettings: Partial<Settings> | null = null;
		let loadedStats: Partial<BreakStats> | null = null;

		// 1. Try Tauri backend settings command
		try {
			const rawSettings = await tauriInvoke<Settings>('get_setting', { key: SETTINGS_KEY });
			if (rawSettings && typeof rawSettings === 'object' && Object.keys(rawSettings).length > 0) {
				loadedSettings = rawSettings;
			}
			const rawStats = await tauriInvoke<BreakStats>('get_setting', { key: STATS_KEY });
			if (rawStats && typeof rawStats === 'object' && Object.keys(rawStats).length > 0) {
				loadedStats = rawStats;
			}
		} catch (err) {
			console.warn('[SettingsStore] Error querying Tauri settings:', err);
		}

		// 2. Fall back to localStorage
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
			this.stats = {
				...DEFAULT_STATS,
				...loadedStats,
				events: Array.isArray(loadedStats.events) ? loadedStats.events : []
			};
		}

		this.isLoaded = true;
	}

	async updateSettings(partial: Partial<Settings>) {
		this.settings = { ...this.settings, ...partial };

		// Save to localStorage
		if (typeof localStorage !== 'undefined') {
			try {
				localStorage.setItem(LOCAL_STORAGE_SETTINGS, JSON.stringify(this.settings));
			} catch {}
		}

		// Save to Tauri backend
		await tauriInvoke('set_setting', { key: SETTINGS_KEY, value: this.settings });

		// Broadcast cross-window
		await emitTauriEvent('breaktime:settings-changed', this.settings);
	}

	async recordBreak(eventData: Omit<BreakEvent, 'id' | 'timestamp'>) {
		const newEvent: BreakEvent = {
			id: `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
			timestamp: Date.now(),
			...eventData
		};

		const nextEvents = [newEvent, ...(this.stats.events || [])].slice(0, 100);

		const isCompleted = eventData.action === 'completed';
		const isEye = eventData.type === 'eye';

		const nextStats: BreakStats = {
			totalCompleted: this.stats.totalCompleted + (isCompleted ? 1 : 0),
			totalSkipped: this.stats.totalSkipped + (isCompleted ? 0 : 1),
			eyeCompleted: this.stats.eyeCompleted + (isEye && isCompleted ? 1 : 0),
			eyeSkipped: this.stats.eyeSkipped + (isEye && !isCompleted ? 1 : 0),
			stretchCompleted: this.stats.stretchCompleted + (!isEye && isCompleted ? 1 : 0),
			stretchSkipped: this.stats.stretchSkipped + (!isEye && !isCompleted ? 1 : 0),
			events: nextEvents
		};

		this.stats = nextStats;

		// Save to localStorage
		if (typeof localStorage !== 'undefined') {
			try {
				localStorage.setItem(LOCAL_STORAGE_STATS, JSON.stringify(nextStats));
			} catch {}
		}

		// Save to Tauri
		await tauriInvoke('set_setting', { key: STATS_KEY, value: nextStats });

		// Broadcast cross-window
		await emitTauriEvent('breaktime:stats-changed', nextStats);
	}

	async resetStats() {
		const emptyStats: BreakStats = { ...DEFAULT_STATS, events: [] };
		this.stats = emptyStats;

		if (typeof localStorage !== 'undefined') {
			try {
				localStorage.setItem(LOCAL_STORAGE_STATS, JSON.stringify(emptyStats));
			} catch {}
		}

		await tauriInvoke('set_setting', { key: STATS_KEY, value: emptyStats });
		await emitTauriEvent('breaktime:stats-changed', emptyStats);
	}
}

export const settingsStore = new SettingsStore();
