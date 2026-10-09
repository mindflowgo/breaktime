import type { AppWindowState } from '$lib/types';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';

export const IDLE_HUD_WIDTH = 104;
export const IDLE_HUD_HEIGHT = 48;

let broadcastChannel: BroadcastChannel | null = null;
if (typeof window !== 'undefined' && typeof BroadcastChannel !== 'undefined') {
	try {
		broadcastChannel = new BroadcastChannel('breaktime_ipc_bus');
	} catch {}
}

export function isOverlayWindow(): boolean {
	if (typeof window === 'undefined') return false;
	const path = window.location.pathname;
	if (path === '/overlay' || path.startsWith('/overlay') || path.includes('overlay')) {
		return true;
	}
	try {
		const internals = (window as unknown as { __TAURI_INTERNALS__?: { metadata?: { currentWindow?: { label?: string } } } })
			.__TAURI_INTERNALS__;
		if (internals?.metadata?.currentWindow?.label === 'overlay') {
			return true;
		}
	} catch {}
	return false;
}

export function isMasterWindow(): boolean {
	return isOverlayWindow();
}

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

/**
 * Strict invoke: rejects on failure. Use when the caller needs to surface the
 * error to the user (e.g. settings persistence).
 */
export async function tauriInvokeStrict<T>(
	command: string,
	args?: Record<string, unknown>
): Promise<T> {
	return await invoke<T>(command, args);
}

export async function emitTauriEvent(event: string, payload?: unknown): Promise<void> {
	if (typeof window === 'undefined') return;
	if (isTauriEnvironment()) {
		try {
			await emit(event, payload);
			return;
		} catch (err) {
			console.warn(`[WindowManager] Tauri emit '${event}' failed:`, err);
		}
	}
	window.dispatchEvent(new CustomEvent(event, { detail: payload }));
	if (broadcastChannel) {
		try {
			broadcastChannel.postMessage({ event, payload });
		} catch {}
	}
}

export async function listenTauriEvent<T = unknown>(
	event: string,
	handler: (payload: T) => void
): Promise<() => void> {
	if (typeof window === 'undefined') return () => {};

	let unlistenNative: UnlistenFn | null = null;
	if (isTauriEnvironment()) {
		try {
			unlistenNative = await listen<T>(event, (evt) => {
				handler(evt.payload);
			});
		} catch (err) {
			console.warn(`[WindowManager] Tauri listen '${event}' failed:`, err);
		}
	}

	const domHandler = (e: Event) => {
		handler((e as CustomEvent<T>).detail);
	};
	window.addEventListener(event, domHandler);

	const channelHandler = (e: MessageEvent) => {
		if (e.data && e.data.event === event) {
			handler(e.data.payload as T);
		}
	};
	if (broadcastChannel) {
		broadcastChannel.addEventListener('message', channelHandler);
	}

	return () => {
		if (unlistenNative) {
			try {
				unlistenNative();
			} catch {}
		}
		window.removeEventListener(event, domHandler);
		if (broadcastChannel) {
			broadcastChannel.removeEventListener('message', channelHandler);
		}
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

export async function exitApp(): Promise<void> {
	await tauriInvoke('exit_app');
	if (typeof window !== 'undefined') {
		window.close();
	}
}

/**
 * Initiate native window drag on the overlay panel (matching llm_gauge)
 */
export async function startDragging(): Promise<void> {
	const res = await tauriInvoke<boolean>('native_overlay_start_drag');
	if (!res && isTauriEnvironment()) {
		try {
			const { getCurrentWindow } = await import('@tauri-apps/api/window');
			await getCurrentWindow().startDragging();
		} catch {}
	}
}

/**
 * Panel-level opacity via native setAlphaValue (0–1), matching llm_gauge.
 * CSS opacity on a transparent NSPanel's webview is unreliable on macOS
 * (flickers / drops while compositing); the panel's own alpha is solid.
 */
export async function setOverlayOpacity(alpha: number): Promise<void> {
	await tauriInvoke('native_overlay_set_opacity', { alpha });
}

