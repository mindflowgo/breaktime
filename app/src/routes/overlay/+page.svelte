<script lang="ts">
	import { onMount } from 'svelte';
	import {
		MoreVertical,
		Check,
		X,
		Play,
		Pause,
		Settings,
		Power,
		Eye,
		Activity,
		Sparkles,
		Moon
	} from 'lucide-svelte';
	import {
		IDLE_HUD_WIDTH,
		IDLE_HUD_HEIGHT,
		exitOverlayMode,
		setOverlayWindowSize,
		exitApp
	} from '$lib/services/windowManager';
	import { timerManager, STRETCH_EXERCISES, EYE_INSTRUCTIONS } from '$lib/services/timerManager.svelte';
	import { settingsStore } from '$lib/services/settingsStore.svelte';

	let isHovered = $state(false);
	let isMenuOpen = $state(false);
	let priorHeight = $state<number | null>(null);

	const MENU_EXPANDED_HEIGHT = 210;

	onMount(() => {
		timerManager.init();

		const handleClickOutside = (e: MouseEvent) => {
			if (isMenuOpen) {
				const target = e.target as HTMLElement;
				if (!target.closest('.menu-container')) {
					closeMenu();
				}
			}
		};

		window.addEventListener('click', handleClickOutside);
		return () => {
			window.removeEventListener('click', handleClickOutside);
		};
	});

	async function openMenu() {
		isMenuOpen = true;
		const currentH = window.innerHeight;
		if (currentH < MENU_EXPANDED_HEIGHT) {
			priorHeight = currentH;
			await setOverlayWindowSize(Math.max(IDLE_HUD_WIDTH, window.innerWidth), MENU_EXPANDED_HEIGHT);
		}
	}

	async function closeMenu() {
		isMenuOpen = false;
		if (priorHeight !== null) {
			const restoreH = priorHeight;
			priorHeight = null;
			await setOverlayWindowSize(window.innerWidth, restoreH);
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

	async function handleOpenSettings() {
		await closeMenu();
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

	// Resize handle
	function handleResizePointerDown(e: PointerEvent) {
		if (e.button !== 0) return;
		e.stopPropagation();
		e.preventDefault();

		if (isMenuOpen) {
			closeMenu();
		}

		const startX = e.screenX;
		const startY = e.screenY;
		const startWidth = window.innerWidth;
		const startHeight = window.innerHeight;
		let lastW = startWidth;
		let lastH = startHeight;
		let rafId: number | null = null;

		const target = e.currentTarget as HTMLElement;
		target.setPointerCapture(e.pointerId);

		function onPointerMove(moveEvent: PointerEvent) {
			const dx = moveEvent.screenX - startX;
			const dy = moveEvent.screenY - startY;

			const minW = timerManager.activeBreak ? 320 : IDLE_HUD_WIDTH;
			const minH = timerManager.activeBreak ? 90 : IDLE_HUD_HEIGHT;
			const newW = Math.max(minW, Math.min(600, Math.round(startWidth + dx)));
			const newH = Math.max(minH, Math.min(400, Math.round(startHeight + dy)));
			lastW = newW;
			lastH = newH;

			if (!rafId) {
				rafId = requestAnimationFrame(() => {
					rafId = null;
					setOverlayWindowSize(lastW, lastH);
				});
			}
		}

		function onPointerUp(upEvent: PointerEvent) {
			target.removeEventListener('pointermove', onPointerMove);
			target.removeEventListener('pointerup', onPointerUp);
			target.releasePointerCapture(upEvent.pointerId);
			if (rafId) {
				cancelAnimationFrame(rafId);
				rafId = null;
			}
			setOverlayWindowSize(lastW, lastH);
		}

		target.addEventListener('pointermove', onPointerMove);
		target.addEventListener('pointerup', onPointerUp);
	}

	// Active break helpers
	let eyeProgressPercent = $derived.by(() => {
		const total = settingsStore.settings.eyeDurationSec || 20;
		if (total <= 0) return 0;
		return Math.max(0, Math.min(100, (timerManager.breakTimeRemaining / total) * 100));
	});

	let stretchProgressPercent = $derived.by(() => {
		const total = settingsStore.settings.stretchDurationSec || 60;
		if (total <= 0) return 0;
		return Math.max(0, Math.min(100, (timerManager.breakTimeRemaining / total) * 100));
	});

	let currentExercise = $derived(
		STRETCH_EXERCISES[timerManager.currentExerciseIndex] || STRETCH_EXERCISES[0]
	);
</script>

<!-- Outer HUD Container -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="overlay-container relative w-full h-full flex flex-col justify-center p-2 box-border transition-all duration-200"
	class:window-hovered={isHovered}
	onmouseenter={() => (isHovered = true)}
	onmouseleave={() => (isHovered = false)}
	data-tauri-drag-region
>
	{#if timerManager.activeBreak === 'eye'}
		<!-- Active Eye Break View -->
		<div class="flex flex-col justify-between h-full w-full px-2 py-1" data-tauri-drag-region>
			<!-- Top row: Header & Controls -->
			<div class="flex items-center justify-between gap-2" data-tauri-drag-region>
				<div class="flex items-center gap-2">
					<div
						class="w-7 h-7 rounded-lg bg-emerald-500/20 border border-emerald-500/40 text-emerald-300 flex items-center justify-center relative animate-pulse"
					>
						<Eye class="w-4 h-4" />
					</div>
					<div>
						<div class="flex items-center gap-2">
							<span class="text-xs font-semibold text-emerald-200 tracking-wide uppercase">
								Eye Reset
							</span>
							<span
								class="px-1.5 py-0.2 rounded-full text-[10px] font-mono font-medium bg-emerald-500/20 text-emerald-300 border border-emerald-500/30"
							>
								Blink {timerManager.blinkCount}/10
							</span>
						</div>
					</div>
				</div>

				<div class="flex items-center gap-1.5 no-drag">
					<span class="text-xs font-mono font-semibold text-zinc-300 mr-1">
						{timerManager.breakTimeRemaining}s
					</span>
					<button
						type="button"
						onclick={() => timerManager.completeBreak()}
						class="w-6 h-6 rounded-md bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-300 border border-emerald-500/40 flex items-center justify-center transition-colors cursor-pointer"
						title="Complete Eye Break"
					>
						<Check class="w-3.5 h-3.5" />
					</button>
					<button
						type="button"
						onclick={() => timerManager.skipBreak()}
						class="w-6 h-6 rounded-md bg-white/5 hover:bg-white/10 text-zinc-400 hover:text-zinc-200 border border-white/10 flex items-center justify-center transition-colors cursor-pointer"
						title="Skip Eye Break"
					>
						<X class="w-3.5 h-3.5" />
					</button>
				</div>
			</div>

			<!-- Middle row: Instructions -->
			<div class="text-[11px] text-zinc-400 tracking-tight flex items-center gap-1.5 truncate mt-0.5">
				<span class="text-emerald-400 font-medium">Tip:</span>
				<span class="truncate">{EYE_INSTRUCTIONS[timerManager.blinkCount % EYE_INSTRUCTIONS.length]}</span>
			</div>

			<!-- Bottom row: Countdown Progress Bar -->
			<div class="w-full h-1 bg-zinc-800/80 rounded-full overflow-hidden mt-1.5">
				<div
					class="h-full bg-emerald-400 rounded-full transition-all duration-1000 ease-linear"
					style="width: {eyeProgressPercent}%"
				></div>
			</div>
		</div>
	{:else if timerManager.activeBreak === 'stretch'}
		<!-- Active Stretch Break View -->
		<div class="flex flex-col justify-between h-full w-full px-2 py-1.5" data-tauri-drag-region>
			<!-- Top row: Header, Exercise step badges, & Controls -->
			<div class="flex items-center justify-between gap-2" data-tauri-drag-region>
				<div class="flex items-center gap-2">
					<div
						class="w-7 h-7 rounded-lg bg-amber-500/20 border border-amber-500/40 text-amber-300 flex items-center justify-center animate-pulse"
					>
						<Activity class="w-4 h-4" />
					</div>
					<div>
						<span class="text-xs font-semibold text-amber-200 tracking-wide uppercase">
							Move Break
						</span>
					</div>
				</div>

				<!-- Exercise 4-step progress dots -->
				<div class="flex items-center gap-1">
					{#each STRETCH_EXERCISES as _, idx}
						<div
							class="w-1.5 h-1.5 rounded-full transition-colors duration-300"
							class:bg-amber-400={timerManager.currentExerciseIndex === idx}
							class:bg-zinc-700={timerManager.currentExerciseIndex !== idx}
						></div>
					{/each}
				</div>

				<div class="flex items-center gap-1.5 no-drag">
					<span class="text-xs font-mono font-semibold text-zinc-300 mr-1">
						{timerManager.breakTimeRemaining}s
					</span>
					<button
						type="button"
						onclick={() => timerManager.completeBreak()}
						class="w-6 h-6 rounded-md bg-amber-500/20 hover:bg-amber-500/30 text-amber-300 border border-amber-500/40 flex items-center justify-center transition-colors cursor-pointer"
						title="Complete Stretch Break"
					>
						<Check class="w-3.5 h-3.5" />
					</button>
					<button
						type="button"
						onclick={() => timerManager.skipBreak()}
						class="w-6 h-6 rounded-md bg-white/5 hover:bg-white/10 text-zinc-400 hover:text-zinc-200 border border-white/10 flex items-center justify-center transition-colors cursor-pointer"
						title="Skip Stretch Break"
					>
						<X class="w-3.5 h-3.5" />
					</button>
				</div>
			</div>

			<!-- Guided rotating exercise title & desc -->
			<div class="flex flex-col mt-1 px-0.5">
				<span class="text-xs font-medium text-zinc-200 tracking-tight">
					{currentExercise.title}
				</span>
				<span class="text-[11px] text-zinc-400 tracking-tight truncate">
					{currentExercise.desc}
				</span>
			</div>

			<!-- Bottom row: Countdown Progress Bar -->
			<div class="w-full h-1 bg-zinc-800/80 rounded-full overflow-hidden mt-1.5">
				<div
					class="h-full bg-amber-400 rounded-full transition-all duration-1000 ease-linear"
					style="width: {stretchProgressPercent}%"
				></div>
			</div>
		</div>
	{:else}
		<!-- Compact Idle HUD (300x56) -->
		<div class="flex items-center justify-between w-full h-full px-2" data-tauri-drag-region>
			<!-- Left: Breaktime brand / toggle -->
			<button
				type="button"
				onclick={handleTogglePause}
				class="no-drag flex items-center justify-center w-8 h-8 rounded-xl bg-white/5 hover:bg-white/10 border border-white/10 transition-colors cursor-pointer group"
				title={timerManager.isPaused ? 'Resume Timers' : 'Breaktime (Click to Pause)'}
			>
				{#if timerManager.isIdle}
					<Moon class="w-4 h-4 text-indigo-400" />
				{:else if timerManager.isPaused}
					<Pause class="w-4 h-4 text-amber-400" />
				{:else}
					<Sparkles class="w-4 h-4 text-emerald-400 group-hover:scale-110 transition-transform" />
				{/if}
			</button>

			<!-- Center: Countdown Pill -->
			<div
				class="flex items-center justify-center px-3 py-1 rounded-full bg-white/5 border border-white/10 shadow-sm text-xs font-mono font-medium tracking-tight text-zinc-200 select-none cursor-default"
				data-tauri-drag-region
			>
				{#if timerManager.isIdle}
					<span class="flex items-center gap-1.5 text-indigo-300">
						<span class="inline-block animate-pulse">💤</span> Away
					</span>
				{:else if timerManager.isPaused}
					<span class="flex items-center gap-1.5 text-amber-300">
						<span>⏸</span> Paused
					</span>
				{:else}
					<div class="flex items-center gap-2">
						{#if settingsStore.settings.eyeEnabled}
							<span class="flex items-center gap-1 text-emerald-300">
								<span>👁️</span>
								<span>{timerManager.formatTimeShort(timerManager.eyeTimeRemaining)}</span>
							</span>
						{/if}
						{#if settingsStore.settings.eyeEnabled && settingsStore.settings.stretchEnabled}
							<span class="text-zinc-600 font-sans">•</span>
						{/if}
						{#if settingsStore.settings.stretchEnabled}
							<span class="flex items-center gap-1 text-amber-300">
								<span>🧍</span>
								<span>{timerManager.formatTimeShort(timerManager.stretchTimeRemaining)}</span>
							</span>
						{/if}
					</div>
				{/if}
			</div>

			<!-- Right: Menu button ⋮ -->
			<div class="relative menu-container no-drag">
				<button
					type="button"
					onclick={toggleMenu}
					title="Menu"
					class="flex items-center justify-center w-8 h-8 rounded-xl text-zinc-400 hover:text-white hover:bg-white/10 transition-colors cursor-pointer"
				>
					<MoreVertical class="w-4 h-4" />
				</button>

				<!-- Dropdown Menu -->
				{#if isMenuOpen}
					<div
						class="menu-dropdown absolute right-0 top-full mt-1.5 w-48 rounded-xl border border-white/15 bg-zinc-950/95 backdrop-blur-md shadow-2xl p-1 z-50 flex flex-col gap-0.5"
					>
						<button
							type="button"
							onclick={handleOpenSettings}
							class="flex items-center gap-2.5 w-full px-2.5 py-1.5 text-xs text-zinc-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors cursor-pointer text-left"
						>
							<Settings class="w-3.5 h-3.5 text-indigo-400" />
							<span>Settings & Stats</span>
						</button>

						<div class="border-t border-white/10 my-0.5"></div>

						<button
							type="button"
							onclick={handleTriggerEye}
							class="flex items-center gap-2.5 w-full px-2.5 py-1.5 text-xs text-emerald-300 hover:text-emerald-100 hover:bg-emerald-500/15 rounded-lg transition-colors cursor-pointer text-left"
						>
							<Eye class="w-3.5 h-3.5 text-emerald-400" />
							<span>Take Eye Break Now</span>
						</button>

						<button
							type="button"
							onclick={handleTriggerStretch}
							class="flex items-center gap-2.5 w-full px-2.5 py-1.5 text-xs text-amber-300 hover:text-amber-100 hover:bg-amber-500/15 rounded-lg transition-colors cursor-pointer text-left"
						>
							<Activity class="w-3.5 h-3.5 text-amber-400" />
							<span>Take Stretch Break Now</span>
						</button>

						<button
							type="button"
							onclick={handleTogglePause}
							class="flex items-center gap-2.5 w-full px-2.5 py-1.5 text-xs text-zinc-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors cursor-pointer text-left"
						>
							{#if timerManager.isPaused}
								<Play class="w-3.5 h-3.5 text-emerald-400" />
								<span>Resume Timers</span>
							{:else}
								<Pause class="w-3.5 h-3.5 text-amber-400" />
								<span>Pause Timers</span>
							{/if}
						</button>

						<div class="border-t border-white/10 my-0.5"></div>

						<button
							type="button"
							onclick={handleQuit}
							class="flex items-center gap-2.5 w-full px-2.5 py-1.5 text-xs text-rose-400 hover:text-rose-200 hover:bg-rose-500/15 rounded-lg transition-colors cursor-pointer text-left"
						>
							<Power class="w-3.5 h-3.5 text-rose-400" />
							<span>Quit Breaktime</span>
						</button>
					</div>
				{/if}
			</div>
		</div>
	{/if}

	<!-- Resize Grip Handle (bottom-right 6-dot grip) -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		onpointerdown={handleResizePointerDown}
		class="resize-handle absolute bottom-1 right-1 p-0.5 text-zinc-500 hover:text-zinc-300 select-none pointer-events-auto cursor-nwse-resize transition-opacity"
		class:opacity-100={isHovered}
		class:opacity-0={!isHovered}
		title="Drag to resize HUD"
	>
		<svg class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="currentColor">
			<circle cx="20" cy="20" r="2" />
			<circle cx="14" cy="20" r="2" />
			<circle cx="20" cy="14" r="2" />
			<circle cx="8" cy="20" r="2" />
			<circle cx="14" cy="14" r="2" />
			<circle cx="20" cy="8" r="2" />
		</svg>
	</div>
</div>

<style>
	.overlay-container {
		background: rgba(18, 18, 22, 0.88) !important;
		backdrop-filter: blur(24px) !important;
		-webkit-backdrop-filter: blur(24px) !important;
		border: 1px solid rgba(255, 255, 255, 0.1);
		border-radius: 16px;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
	}

	.overlay-container.window-hovered {
		border: 1px solid rgba(255, 255, 255, 0.22);
		box-shadow: 0 10px 40px rgba(0, 0, 0, 0.55), inset 0 0 0 1px rgba(255, 255, 255, 0.05);
	}

	.resize-handle {
		-webkit-app-region: no-drag;
	}

	.menu-dropdown {
		animation: menuFadeIn 0.12s ease-out;
	}

	@keyframes menuFadeIn {
		from {
			opacity: 0;
			transform: scale(0.96) translateY(-4px);
		}
		to {
			opacity: 1;
			transform: scale(1) translateY(0);
		}
	}
</style>
