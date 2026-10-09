<script lang="ts">
	import { onMount } from 'svelte';
	import {
		MoreVertical,
		Settings,
		BarChart3,
		Eye,
		Activity,
		Power,
		Play,
		Pause as PauseIcon,
		Check
	} from 'lucide-svelte';
	import {
		exitOverlayMode,
		setOverlayWindowSize,
		setOverlayOpacity,
		startDragging,
		exitApp,
		emitTauriEvent,
		listenTauriEvent,
		isTauriEnvironment
	} from '$lib/services/windowManager';
	import { timerManager } from '$lib/services/timerManager.svelte';
	import { settingsStore } from '$lib/services/settingsStore.svelte';
	import { warmUpAudioContext } from '$lib/services/soundPlayer';
	import { EV_OPEN_TAB, EV_OVERLAY_MODE_CHANGED } from '$lib/services/events';
	import EyeTimerSvg from '$lib/components/EyeTimerSvg.svelte';
	import StretchFigure from '$lib/components/StretchFigure.svelte';

	let isMenuOpen = $state(false);
	let isMouseOver = $state(false);

	const MENU_EXPANDED_WIDTH = 200;
	const MENU_EXPANDED_HEIGHT = 330;

	// 2× HUD mode: window, eye, figure and text all scale together.
	let hudScale = $derived(settingsStore.settings.hud2x ? 2 : 1);
	let idleSize = $derived(timerManager.idleHudSize);
	let eyeClass = $derived(
		hudScale === 2 ? 'w-[96px] h-[63px]' : 'w-[48px] h-[32px]'
	);
	let timeClass = $derived(hudScale === 2 ? 'text-[13px]' : 'text-[9px]');
	// At 2× the eye box has ~6px of empty space under the drawn bottom curve;
	// pull the time up into it so it sits on the eye's base instead of
	// stretching the window taller. (The stretch person's time stays put.)
	let timeTuckClass = $derived(hudScale === 2 ? '-mt-2' : '-mt-0.5');
	let personClass = $derived(hudScale === 2 ? 'w-[26px] h-[26px]' : 'w-[14px] h-[14px]');
	let personTimeClass = $derived(hudScale === 2 ? 'text-[11px]' : 'text-[8px]');
	let stretchFigClass = $derived(hudScale === 2 ? 'w-[62px] h-[62px]' : 'w-[34px] h-[34px]');

	async function resizeToIdle() {
		await setOverlayWindowSize(idleSize.width, idleSize.height);
	}

	onMount(() => {
		void (async () => {
			await timerManager.init();

			// Ensure overlay starts at compact dimensions
			if (!timerManager.activeBreak) {
				await resizeToIdle();
			}
		})();

		// Unlock WebAudio on the first user interaction
		const unlockAudio = () => {
			warmUpAudioContext();
			window.removeEventListener('pointerdown', unlockAudio);
		};
		window.addEventListener('pointerdown', unlockAudio);

		// Window dragging: drag anywhere except interactive controls
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
				void dist; // taps intentionally do nothing: breaks start only from
				// the timer firing or the ⋮ menu, never from tapping the eye.
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
		let unlistenEntered: (() => void) | null = null;
		let unlistenExited: (() => void) | null = null;
		let unlistenMode: (() => void) | null = null;

		void listenTauriEvent('overlay-exited', () => {
			isMouseOver = false;
			void closeMenu();
		}).then((fn) => (unlistenExited = fn));

		// Hover state from the native tracking area (reliable for a
		// transparent panel, unlike DOM pointerenter/leave).
		void listenTauriEvent('overlay-entered', () => {
			isMouseOver = true;
		}).then((fn) => (unlistenEntered = fn));

		// Returning to overlay mode (e.g. "Back to Overlay" from settings):
		// re-apply the idle size — Rust sizes the window from its own settings
		// read, this is the belt-and-braces frontend pass.
		void listenTauriEvent<string>(EV_OVERLAY_MODE_CHANGED, (mode) => {
			isNormalMode = mode !== 'minimalist';
			if (mode === 'minimalist' && !timerManager.activeBreak && !isMenuOpen) {
				void resizeToIdle();
			}
		}).then((fn) => (unlistenMode = fn));

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
			if (unlistenEntered) unlistenEntered();
			if (unlistenExited) unlistenExited();
			if (unlistenMode) unlistenMode();
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

	// Panel opacity — applied natively (panel setAlphaValue, matching
	// llm_gauge); CSS opacity on the transparent panel's webview is
	// unreliable on macOS. 100% while hovered, during any break, and while
	// the settings/stats window is open; otherwise the user's default.
	let isNormalMode = $state(false);
	let hudOpacity = $derived.by(() => {
		if (isMouseOver || timerManager.activeBreak || isNormalMode) return 1;
		return settingsStore.settings.overlayOpacity ?? 0.85;
	});

	$effect(() => {
		void setOverlayOpacity(hudOpacity);
	});

	async function openMenu() {
		isMenuOpen = true;
		// Never shrink below the idle size when expanding for the menu.
		await setOverlayWindowSize(
			Math.max(MENU_EXPANDED_WIDTH, idleSize.width),
			Math.max(MENU_EXPANDED_HEIGHT, idleSize.height)
		);
	}

	async function closeMenu() {
		if (!isMenuOpen) return;
		isMenuOpen = false;
		await resizeToIdle();
	}

	/** Toggle 2× HUD mode — close the menu and resize immediately when idle. */
	async function handleToggle2x() {
		await settingsStore.updateSettings({ hud2x: !settingsStore.settings.hud2x });
		if (isMenuOpen) {
			await closeMenu(); // closeMenu resizes to the new idle size
		} else if (!timerManager.activeBreak) {
			await resizeToIdle();
		}
	}

	async function toggleMenu(e: MouseEvent) {
		e.stopPropagation();
		if (isMenuOpen) {
			await closeMenu();
		} else {
			await openMenu();
		}
	}

	// The eye and the person are drag regions only — pressing them never
	// triggers or completes a break. Breaks start exclusively from the timer
	// firing or the ⋮ menu (Eye/Stretch Now); an active break completes via
	// Enter and skips via the ✕ button or Esc.

	async function handleOpenSettings(tab: 'reminders' | 'sounds' | 'inactivity' | 'stats' = 'reminders') {
		await closeMenu();
		if (typeof localStorage !== 'undefined') {
			localStorage.setItem('breaktime_active_tab', tab);
		}
		await emitTauriEvent(EV_OPEN_TAB, tab);
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

	function fmtTime(seconds: number): string {
		if (seconds > 60) return `${Math.ceil(seconds / 60)}m`;
		return `${Math.max(0, seconds)}s`;
	}

	// Break countdown ring progress: 1.0 (start) → 0.0 (end)
	let breakProgress = $derived.by(() => {
		if (!timerManager.activeBreak) return 0;
		const total = timerManager.durationFor(timerManager.activeBreak);
		if (total <= 0) return 0;
		return Math.max(0, Math.min(1, timerManager.breakTimeRemaining / total));
	});

	// Idle eyelid droop: fraction of the eye interval that has elapsed
	let eyeDroop = $derived.by(() => {
		if (timerManager.activeBreak || timerManager.isIdle || timerManager.isPaused) return 0;
		if (!settingsStore.settings.eyeEnabled) return 0;
		const interval = timerManager.intervalSecFor('eye');
		if (interval <= 0) return 0;
		return Math.max(0, Math.min(1, 1 - timerManager.eyeTimeRemaining / interval));
	});

	// Left-side time: break countdown during breaks, else time to next eye break
	let timeDisplay = $derived.by(() => {
		if (timerManager.activeBreak) {
			return {
				text: `${timerManager.breakTimeRemaining}s`,
				accent: timerManager.activeBreak === 'eye' ? 'text-emerald-300' : 'text-amber-300'
			};
		}
		if (timerManager.isIdle) return { text: 'away', accent: 'text-indigo-300' };
		if (timerManager.isPaused) return { text: 'paused', accent: 'text-zinc-400' };
		if (settingsStore.settings.eyeEnabled) {
			return { text: fmtTime(timerManager.eyeTimeRemaining), accent: 'text-emerald-300' };
		}
		if (settingsStore.settings.stretchEnabled) {
			return { text: fmtTime(timerManager.stretchTimeRemaining), accent: 'text-amber-300' };
		}
		return { text: 'off', accent: 'text-zinc-500' };
	});

	// Right-side stretch time (idle only)
	let stretchTimeDisplay = $derived.by(() => {
		if (timerManager.activeBreak || !settingsStore.settings.stretchEnabled) return '';
		if (timerManager.isIdle || timerManager.isPaused) return '–';
		return fmtTime(timerManager.stretchTimeRemaining);
	});

	let centerTooltip = $derived.by(() => {
		if (timerManager.activeBreak === 'stretch') {
			return `Stretch break (${timerManager.breakTimeRemaining}s) — click to finish, Esc to skip`;
		}
		if (timerManager.activeBreak) {
			return `Eye break (${timerManager.breakTimeRemaining}s) — click to finish, Esc to skip`;
		}
		if (timerManager.isIdle) return 'Away — drag to move';
		if (timerManager.isPaused) return 'Paused — click eye to resume cycle';
		return `Next eye break in ${timeDisplay.text} — click to start now, drag to move`;
	});
</script>

<svelte:window onkeydown={handleGlobalKeyDown} />

<!-- Transparent overlay: default panel opacity from settings, 100% on hover,
     during breaks, and while settings/stats is open. Opacity is applied
     natively via panel setAlphaValue (see hudOpacity effect). -->
<div
	role="presentation"
	class="overlay-container relative w-full h-full box-border select-none overflow-hidden"
	data-tauri-drag-region
>
	<!-- Subtle outline of the overlay while hovered (llm_gauge style) -->
	{#if isMouseOver}
		<div
			class="pointer-events-none absolute inset-0 rounded-2xl border border-white/20 bg-white/[0.04]"
		></div>
	{/if}
	<!-- HUD Row: [eye (time tucked under its curve) | person (stretch time below) / skip] -->
	<div
		class="hud-row flex items-center justify-between w-full h-full px-1.5 gap-1 box-border"
		data-tauri-drag-region
	>
		<!-- Center: eye (idle + eye breaks) or the stretch figure; time sits
		     directly below, just under the bottom curve of the eye. -->
		<div
			class="relative flex flex-col items-center justify-center flex-1 min-w-0 self-stretch py-0.5"
			title={centerTooltip}
			data-tauri-drag-region
		>
			{#if timerManager.activeBreak === 'stretch'}
				<StretchFigure progress={breakProgress} class="{stretchFigClass} text-amber-300" />
			{:else}
				<EyeTimerSvg
					progress={breakProgress}
					droop={eyeDroop}
					isBreakActive={timerManager.activeBreak === 'eye'}
					isIdle={timerManager.isIdle}
					class={eyeClass}
				/>
			{/if}
			<span
				class="{timeTuckClass} {timeClass} font-mono font-semibold tracking-tight leading-none {timeDisplay.accent} drop-shadow-[0_1px_1px_rgba(0,0,0,0.9)] select-none"
				data-tauri-drag-region
			>
				{timeDisplay.text}
			</span>
		</div>

		<!-- Right: stretch person with its countdown below (idle); skip button
		     (breaks); menu toggle always at the far right. -->
		<div class="flex items-center gap-0.5 flex-shrink-0 no-drag self-stretch">
			{#if timerManager.activeBreak}
				<button
					type="button"
					onclick={() => timerManager.skipBreak()}
					title="Skip break (Esc)"
					class="flex items-center justify-center w-6 h-7 rounded-lg text-zinc-400 hover:text-rose-300 hover:bg-rose-500/15 active:scale-95 transition-all cursor-pointer no-drag"
				>
					<span class="text-[13px] font-bold leading-none">✕</span>
				</button>
			{:else if settingsStore.settings.stretchEnabled}
				<!-- Subtle grey standing person: display-only (breaks start from the
				     timer or the ⋮ menu, never from pressing the person). -->
				<div
					class="flex flex-col items-center justify-center gap-0.5 px-0.5 h-full text-zinc-500 no-drag"
					title="Next stretch break in {stretchTimeDisplay}"
				>
					<!-- Subtle grey standing person -->
					<svg
						viewBox="0 0 24 24"
						class={personClass}
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
					>
						<circle cx="12" cy="5" r="1" />
						<path d="m9 20 3-6 3 6" />
						<path d="m6 8 6 2 6-2" />
						<path d="M12 10v4" />
					</svg>
					{#if stretchTimeDisplay}
						<span class="{personTimeClass} font-mono font-medium leading-none">{stretchTimeDisplay}</span>
					{/if}
				</div>
			{/if}

			<div class="relative menu-container no-drag flex-shrink-0">
				<button
					type="button"
					onclick={toggleMenu}
					title="Menu"
					class="flex items-center justify-center w-6 h-7 rounded-lg text-zinc-400 hover:text-white hover:bg-white/10 active:scale-95 transition-all cursor-pointer"
				>
					<MoreVertical class="w-4 h-4" />
				</button>
			</div>
		</div>
	</div>

	<!-- Floating Dropdown Menu -->
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
					<span class="font-medium truncate">Eye Break Now</span>
				</div>
				<span class="text-[10px] text-emerald-400/70 font-mono">{timerManager.durationFor('eye')}s</span>
			</button>

			<button
				type="button"
				onclick={handleTriggerStretch}
				class="flex items-center justify-between w-full px-2.5 py-1.5 text-xs text-amber-300 hover:text-amber-100 hover:bg-amber-500/15 rounded-xl transition-colors cursor-pointer text-left no-drag"
			>
				<div class="flex items-center gap-2 min-w-0">
					<Activity class="w-3.5 h-3.5 text-amber-400 flex-shrink-0" />
					<span class="font-medium truncate">Stretch Now</span>
				</div>
				<span class="text-[10px] text-amber-400/70 font-mono">{timerManager.durationFor('stretch')}s</span>
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
					<PauseIcon class="w-3.5 h-3.5 text-amber-400 flex-shrink-0" />
					<span class="font-medium">Pause Timers</span>
				{/if}
			</button>

			<button
				type="button"
				onclick={handleToggle2x}
				class="flex items-center gap-2.5 w-full px-2.5 py-1.5 text-xs text-zinc-300 hover:text-white hover:bg-white/10 rounded-xl transition-colors cursor-pointer text-left no-drag"
				title="Double the HUD size (window, eye and text)"
			>
				<span class="w-3.5 h-3.5 flex-shrink-0 flex items-center justify-center">
					{#if settingsStore.settings.hud2x}
						<Check class="w-3.5 h-3.5 text-sky-400" />
					{/if}
				</span>
				<span class="font-medium">2× Size</span>
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
