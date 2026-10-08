import type { AppWindowState } from '$lib/types';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';

export const IDLE_HUD_WIDTH = 300;
export const IDLE_HUD_HEIGHT = 56;

export function isTauriEnvironment(): boolean {
	if (typeof window === 'undefined') return false;
	const tauri = (window as unknown as { __TAURI__?: unknown }).__TAURI__;
	return !!tauri || !!(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
}

export async function tauriInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T | null> {
	if (typeof window === 'undefined') return null;
	try {
		return await invoke<T>(command, args);
	} catch (err) {
		console.warn(`[WindowManager] Tauri invoke '${command}' failed:`, err);
		return null;
	}
}

export async function emitTauriEvent(event: string, payload?: unknown): Promise<void> {
	if (typeof window === 'undefined') return;
	try {
		await emit(event, payload);
	} catch (err) {
		console.warn(`[WindowManager] Tauri emit '${event}' failed:`, err);
		window.dispatchEvent(new CustomEvent(event, { detail: payload }));
	}
}

export async function listenTauriEvent<T = unknown>(
	event: string,
	handler: (payload: T) => void
): Promise<() => void> {
	if (typeof window === 'undefined') return () => {};

	let unlistenNative: UnlistenFn | null = null;
	try {
		unlistenNative = await listen<T>(event, (evt) => {
			handler(evt.payload);
		});
	} catch (err) {
		console.warn(`[WindowManager] Tauri listen '${event}' failed:`, err);
	}

	const domHandler = (e: Event) => {
		handler((e as CustomEvent<T>).detail);
	};
	window.addEventListener(event, domHandler);

	return () => {
		if (unlistenNative) {
			try {
				unlistenNative();
			} catch {}
		}
		window.removeEventListener(event, domHandler);
	};
}

export async function enterOverlayMode(): Promise<AppWindowState | null> {
	return await tauriInvoke<AppWindowState>('enter_overlay_mode');
}

export async function exitOverlayMode(): Promise<AppWindowState | null> {
	return await tauriInvoke<AppWindowState>('exit_overlay_mode');
}

export async function setOverlayWindowSize(width: number, height: number): Promise<void> {
	const safeW = Math.max(IDLE_HUD_WIDTH, Math.round(width));
	const safeH = Math.max(IDLE_HUD_HEIGHT, Math.round(height));
	await tauriInvoke('set_overlay_size', { width: safeW, height: safeH });
}

export async function getWindowState(): Promise<AppWindowState | null> {
	return await tauriInvoke<AppWindowState>('get_window_state');
}

export async function saveWindowState(state: AppWindowState): Promise<void> {
	await tauriInvoke('save_window_state', { state });
}

export async function exitApp(): Promise<void> {
	await tauriInvoke('exit_app');
	if (typeof window !== 'undefined') {
		window.close();
	}
}
