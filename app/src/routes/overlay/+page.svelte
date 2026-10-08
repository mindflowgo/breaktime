<script lang="ts">
	import { onMount } from 'svelte';
	import {
		MoreVertical,
		Settings,
		BarChart3,
		Eye,
		Activity,
		Play,
		Pause,
		Power
	} from 'lucide-svelte';
	import {
		IDLE_HUD_WIDTH,
		IDLE_HUD_HEIGHT,
		exitOverlayMode,
		setOverlayWindowSize,
		startDragging,
		exitApp,
		emitTauriEvent,
		listenTauriEvent,
		isTauriEnvironment
	} from '$lib/services/windowManager';
	import { timerManager } from '$lib/services/timerManager.svelte';
	import { settingsStore } from '$lib/services/settingsStore.svelte';
	import { warmUpAudioContext } from '$lib/services/soundPlayer';
	import EyeTimerSvg from '$lib/components/EyeTimerSvg.svelte';

	let isMenuOpen = $state(false);

	const MENU_EXPANDED_WIDTH = 200;
	const MENU_EXPANDED_HEIGHT = 240;

	onMount(() => {
		void (async () => {
			await timerManager.init();

			// Clean up any stale oversize HUD dimension from previous builds
			if (settingsStore.settings.hudSize && settingsStore.settings.hudSize.width > 150) {
				await timerManager.setIdleHudSize(IDLE_HUD_WIDTH, IDLE_HUD_HEIGHT);
			}

			// Ensure overlay starts at compact dimensions
			if (!timerManager.activeBreak) {
				await setOverlayWindowSize(IDLE_HUD_WIDTH, IDLE_HUD_HEIGHT);
			}
		})();

		// Unlock WebAudio on the first user interaction
		const unlockAudio = () => {
			warmUpAudioContext();
			window.removeEventListener('pointerdown', unlockAudio);
		};
		window.addEventListener('pointerdown', unlockAudio);

		// Window dragging matching llm_gauge: allow dragging anywhere except interactive controls
		let dragStartPos: { x: number; y: number } | null = null;

		const handlePointerDown = async (e: PointerEvent) => {
			if (e.button !== 0) return;
			const target = e.target as HTMLElement;
			if (target.closest('button, .menu-container, .menu-dropdown')) {
				return;
			}
			dragStartPos = { x: e.screenX, y: e.screenY };
			await startDragging();
		};

		const handlePointerUp = (e: PointerEvent) => {
			if (dragStartPos) {
				const dist = Math.hypot(e.screenX - dragStartPos.x, e.screenY - dragStartPos.y);
				dragStartPos = null;
				// If user tapped/clicked without moving, perform click action
				if (dist < 5) {
					handleEyeClick();
				}
			}
		};

		window.addEventListener('pointerdown', handlePointerDown);
		window.addEventListener('pointerup', handlePointerUp);

		const handleClickOutside = (e: MouseEvent) => {
			if (isMenuOpen) {
				const target = e.target as HTMLElement;
				if (!target.closest('.menu-container') && !target.closest('.menu-dropdown')) {
					void closeMenu();
				}
			}
		};
		window.addEventListener('click', handleClickOutside);

		let unlistenFocus: (() => void) | null = null;
		let unlistenExited: (() => void) | null = null;

		void listenTauriEvent('overlay-exited', () => {
			void closeMenu();
		}).then((fn) => (unlistenExited = fn));

		if (isTauriEnvironment()) {
			void import('@tauri-apps/api/window')
				.then(({ getCurrentWindow }) =>
					getCurrentWindow()
						.onFocusChanged(({ payload: focused }) => {
							if (!focused) void closeMenu();
						})
						.then((fn) => {
							unlistenFocus = fn;
						})
				)
				.catch(() => {});
		}

		return () => {
			window.removeEventListener('pointerdown', handlePointerDown);
			window.removeEventListener('pointerup', handlePointerUp);
			window.removeEventListener('click', handleClickOutside);
			window.removeEventListener('pointerdown', unlockAudio);
			if (unlistenFocus) unlistenFocus();
			if (unlistenExited) unlistenExited();
			timerManager.destroy();
			settingsStore.destroy();
		};
	});

	// Dismiss menu if a break begins
	$effect(() => {
		if (timerManager.activeBreak && isMenuOpen) {
			void closeMenu();
		}
	});

	async function openMenu() {
		isMenuOpen = true;
		await setOverlayWindowSize(MENU_EXPANDED_WIDTH, MENU_EXPANDED_HEIGHT);
	}

	async function closeMenu() {
		if (!isMenuOpen) return;
		isMenuOpen = false;
		await setOverlayWindowSize(IDLE_HUD_WIDTH, IDLE_HUD_HEIGHT);
	}

	async function toggleMenu(e: MouseEvent) {
		e.stopPropagation();
		if (isMenuOpen) {
			await closeMenu();
		} else {
			await openMenu();
		}
	}

	function handleEyeClick() {
		if (timerManager.activeBreak) {
			// Clicking the eye while break is running completes it
			void timerManager.completeBreak();
		} else {
			// Clicking while idle triggers a 20s eye break
			void timerManager.triggerBreak('eye');
		}
	}

	function handleEyeKeyDown(e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			handleEyeClick();
		}
	}

	async function handleOpenSettings(tab: 'reminders' | 'sounds' | 'inactivity' | 'stats' = 'reminders') {
		await closeMenu();
		if (typeof localStorage !== 'undefined') {
			localStorage.setItem('breaktime_active_tab', tab);
		}
		await emitTauriEvent('breaktime:open-tab', tab);
		await exitOverlayMode();
	}

	async function handleTriggerEye() {
		await closeMenu();
		await timerManager.triggerBreak('eye');
	}

	async function handleTriggerStretch() {
		await closeMenu();
		await timerManager.triggerBreak('stretch');
	}

	async function handleTogglePause() {
		await closeMenu();
		timerManager.togglePause();
	}

	async function handleQuit() {
		await closeMenu();
		await exitApp();
	}

	function handleGlobalKeyDown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			if (isMenuOpen) {
				void closeMenu();
			} else if (timerManager.activeBreak) {
				void timerManager.skipBreak();
			}
		} else if (e.key === 'Enter') {
			if (timerManager.activeBreak && !isMenuOpen) {
				e.preventDefault();
				void timerManager.completeBreak();
			}
		}
	}

	// 20s pomo timer progress: 1.0 (start) down to 0.0 (end)
	let breakProgress = $derived.by(() => {
		if (!timerManager.activeBreak) return 0;
		const total =
			timerManager.activeBreak === 'eye'
				? settingsStore.settings.eyeDurationSec || 20
				: settingsStore.settings.stretchDurationSec || 60;
		if (total <= 0) return 0;
		return Math.max(0, Math.min(1, timerManager.breakTimeRemaining / total));
	});

	// Very small grey letters for the time to close eye (at bottom-right of the eye icon)
	let timeDisplay = $derived.by(() => {
		if (timerManager.activeBreak) {
			return `${timerManager.breakTimeRemaining}s`;
		}
		if (timerManager.isIdle) {
			return 'away';
		}
		if (timerManager.isPaused) {
			return 'paused';
		}
		if (settingsStore.settings.eyeEnabled) {
			if (timerManager.eyeTimeRemaining > 60) {
				return `${Math.ceil(timerManager.eyeTimeRemaining / 60)}m`;
			}
			return `${Math.max(0, timerManager.eyeTimeRemaining)}s`;
		}
		if (settingsStore.settings.stretchEnabled) {
			if (timerManager.stretchTimeRemaining > 60) {
				return `${Math.ceil(timerManager.stretchTimeRemaining / 60)}m`;
			}
			return `${Math.max(0, timerManager.stretchTimeRemaining)}s`;
		}
		return 'off';
	});

	let eyeTooltip = $derived.by(() => {
		if (timerManager.activeBreak) {
			return `Break active (${timerManager.breakTimeRemaining}s) - Click or drag`;
		}
		if (timerManager.isPaused) return 'Breaktime paused - Click or drag';
		if (timerManager.isIdle) return 'Away (paused) - Click or drag';
		return `Time to next eye break: ${timeDisplay} - Click to start, drag to move`;
	});
</script>

<svelte:window onkeydown={handleGlobalKeyDown} />

<!-- Main Overlay Window: 100% transparent background with full drag support matching llm_gauge -->
<div
	class="overlay-container relative w-full h-full box-border select-none overflow-hidden"
	data-tauri-drag-region
>
	<!-- Visible Minimalist Row: Eye SVG + Vertical Menu Dots -->
	<div
		class="hud-row flex items-center justify-between w-full h-[48px] px-1.5 box-border"
		data-tauri-drag-region
	>
		<!-- Eye Icon (Draggable anywhere; quick click without dragging starts/completes break) -->
		<div
			class="relative flex items-center justify-center transition-transform duration-150 hover:scale-[1.03] active:scale-[0.97]"
			title={eyeTooltip}
			data-tauri-drag-region
		>
			<EyeTimerSvg
				progress={breakProgress}
				isBreakActive={!!timerManager.activeBreak}
				isIdle={timerManager.isIdle}
				isPaused={timerManager.isPaused}
				timeText={timeDisplay}
				class="w-[68px] h-[41px]"
			/>
		</div>

		<!-- Vertical Dots Menu Button ⋮ -->
		<div class="relative menu-container no-drag flex-shrink-0">
			<button
				type="button"
				onclick={toggleMenu}
				title="Menu"
				class="flex items-center justify-center w-6 h-7 rounded-lg text-zinc-400 hover:text-white hover:bg-white/10 active:scale-95 transition-all cursor-pointer no-drag"
			>
				<MoreVertical class="w-4 h-4" />
			</button>
		</div>
	</div>

	<!-- Floating Dropdown Menu (only visible when isMenuOpen is true) -->
	{#if isMenuOpen}
		<div
			class="menu-dropdown absolute right-1.5 top-[46px] w-[186px] rounded-2xl border border-white/15 bg-zinc-950/95 backdrop-blur-xl shadow-2xl p-1.5 z-50 flex flex-col gap-0.5 no-drag"
		>
			<button
				type="button"
				onclick={() => handleOpenSettings('reminders')}
				class="flex items-center gap-2.5 w-full px-2.5 py-1.5 text-xs text-zinc-300 hover:text-white hover:bg-white/10 rounded-xl transition-colors cursor-pointer text-left no-drag"
			>
				<Settings class="w-3.5 h-3.5 text-indigo-400 flex-shrink-0" />
				<span class="font-medium">Settings</span>
			</button>

			<button
				type="button"
				onclick={() => handleOpenSettings('stats')}
				class="flex items-center gap-2.5 w-full px-2.5 py-1.5 text-xs text-zinc-300 hover:text-white hover:bg-white/10 rounded-xl transition-colors cursor-pointer text-left no-drag"
			>
				<BarChart3 class="w-3.5 h-3.5 text-emerald-400 flex-shrink-0" />
				<span class="font-medium">Stats Tracker</span>
			</button>

			<div class="border-t border-white/10 my-0.5"></div>

			<button
				type="button"
				onclick={handleTriggerEye}
				class="flex items-center justify-between w-full px-2.5 py-1.5 text-xs text-emerald-300 hover:text-emerald-100 hover:bg-emerald-500/15 rounded-xl transition-colors cursor-pointer text-left no-drag"
			>
				<div class="flex items-center gap-2 min-w-0">
					<Eye class="w-3.5 h-3.5 text-emerald-400 flex-shrink-0" />
					<span class="font-medium truncate">Take Eye Break</span>
				</div>
				<span class="text-[10px] text-emerald-400/70 font-mono">20s</span>
			</button>

			<button
				type="button"
				onclick={handleTriggerStretch}
				class="flex items-center justify-between w-full px-2.5 py-1.5 text-xs text-amber-300 hover:text-amber-100 hover:bg-amber-500/15 rounded-xl transition-colors cursor-pointer text-left no-drag"
			>
				<div class="flex items-center gap-2 min-w-0">
					<Activity class="w-3.5 h-3.5 text-amber-400 flex-shrink-0" />
					<span class="font-medium truncate">Take Stretch</span>
				</div>
				<span class="text-[10px] text-amber-400/70 font-mono">60s</span>
			</button>

			<button
				type="button"
				onclick={handleTogglePause}
				class="flex items-center gap-2.5 w-full px-2.5 py-1.5 text-xs text-zinc-300 hover:text-white hover:bg-white/10 rounded-xl transition-colors cursor-pointer text-left no-drag"
			>
				{#if timerManager.isPaused}
					<Play class="w-3.5 h-3.5 text-emerald-400 flex-shrink-0" />
					<span class="font-medium">Resume Timers</span>
				{:else}
					<Pause class="w-3.5 h-3.5 text-amber-400 flex-shrink-0" />
					<span class="font-medium">Pause Timers</span>
				{/if}
			</button>

			<div class="border-t border-white/10 my-0.5"></div>

			<button
				type="button"
				onclick={handleQuit}
				class="flex items-center gap-2.5 w-full px-2.5 py-2 text-xs text-rose-400 hover:text-rose-200 hover:bg-rose-500/15 rounded-xl transition-colors cursor-pointer text-left no-drag"
			>
				<Power class="w-3.5 h-3.5 text-rose-400 flex-shrink-0" />
				<span class="font-medium">Quit Breaktime</span>
			</button>
		</div>
	{/if}
</div>

<style>
	/* 100% transparent container matching llm_gauge borderless draggable window */
	.overlay-container {
		height: 100vh;
		width: 100vw;
		background: transparent !important;
		backdrop-filter: none !important;
		-webkit-backdrop-filter: none !important;
		border: none !important;
		overflow: hidden;
		box-shadow: none !important;
		-webkit-app-region: drag;
		cursor: grab;
	}

	.overlay-container:active {
		cursor: grabbing;
	}

	.hud-row {
		background: transparent !important;
		-webkit-app-region: drag;
	}

	.no-drag,
	.menu-container,
	.menu-dropdown {
		-webkit-app-region: no-drag;
		cursor: default;
	}

	.menu-dropdown {
		animation: menuFadeIn 0.12s ease-out;
	}

	@keyframes menuFadeIn {
		from {
			opacity: 0;
			transform: scale(0.95) translateY(-3px);
		}
		to {
			opacity: 1;
			transform: scale(1) translateY(0);
		}
	}
</style>
