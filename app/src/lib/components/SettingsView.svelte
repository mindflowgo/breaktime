<script lang="ts">
	import { onMount } from 'svelte';
	import { BarChart3, Clock, Minimize2, Moon, Sparkles, Volume2, AlertTriangle } from 'lucide-svelte';
	import { enterOverlayMode, listenTauriEvent } from '$lib/services/windowManager';
	import { settingsStore } from '$lib/services/settingsStore.svelte';
	import { timerManager } from '$lib/services/timerManager.svelte';
	import { stopSound } from '$lib/services/soundPlayer';
	import { EV_OPEN_TAB } from '$lib/services/events';
	import RemindersTab from '$lib/components/settings/RemindersTab.svelte';
	import SoundsTab from '$lib/components/settings/SoundsTab.svelte';
	import InactivityTab from '$lib/components/settings/InactivityTab.svelte';
	import StatsTab from '$lib/components/settings/StatsTab.svelte';

	const TAB_IDS = ['reminders', 'sounds', 'inactivity', 'stats'] as const;
	type TabId = (typeof TAB_IDS)[number];

	const TAB_META: Record<TabId, { icon: typeof Clock; label: string }> = {
		reminders: { icon: Clock, label: 'Reminders' },
		sounds: { icon: Volume2, label: 'Sounds' },
		inactivity: { icon: Moon, label: 'Inactivity' },
		stats: { icon: BarChart3, label: 'Stats & History' }
	};

	let currentTab = $state<TabId>('reminders');

	function isTabId(v: unknown): v is TabId {
		return typeof v === 'string' && (TAB_IDS as readonly string[]).includes(v);
	}

	onMount(() => {
		const savedTab =
			typeof localStorage !== 'undefined' ? localStorage.getItem('breaktime_active_tab') : null;
		if (isTabId(savedTab)) {
			currentTab = savedTab;
		}

		settingsStore.init();
		timerManager.init();

		const unlistenPromise = listenTauriEvent<unknown>(EV_OPEN_TAB, (tab) => {
			if (isTabId(tab)) {
				currentTab = tab;
			}
		});

		return () => {
			unlistenPromise.then((unlisten) => unlisten());
			timerManager.destroy();
			settingsStore.destroy();
		};
	});

	function switchTab(tab: TabId) {
		currentTab = tab;
		if (typeof localStorage !== 'undefined') {
			localStorage.setItem('breaktime_active_tab', tab);
		}
	}

	async function handleBackToOverlay() {
		stopSound();
		await enterOverlayMode();
	}
</script>

<div class="h-screen w-screen flex flex-col bg-zinc-950 text-zinc-100 overflow-hidden font-sans">
	<!-- Top Window Header -->
	<header
		class="h-14 border-b border-white/10 px-4 flex items-center justify-between flex-shrink-0 bg-zinc-900/60 backdrop-blur-md"
		data-tauri-drag-region
	>
		<div class="flex items-center gap-2.5">
			<div
				class="w-7 h-7 rounded-xl bg-gradient-to-br from-emerald-500/20 to-teal-500/20 border border-emerald-500/30 flex items-center justify-center shadow-sm"
			>
				<Sparkles class="w-4 h-4 text-emerald-400" />
			</div>
			<div class="flex items-baseline gap-2">
				<h1 class="text-sm font-semibold tracking-tight text-white">Breaktime</h1>
				<span class="text-[11px] font-mono text-zinc-500 font-medium">Settings & Stats</span>
			</div>
		</div>

		<button
			type="button"
			onclick={handleBackToOverlay}
			class="no-drag flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:text-white bg-white/10 hover:bg-white/15 border border-white/15 rounded-lg shadow-sm transition-all cursor-pointer group"
			title="Return to HUD overlay mode"
		>
			<Minimize2 class="w-3.5 h-3.5 text-emerald-400 group-hover:scale-95 transition-transform" />
			<span>Back to Overlay</span>
		</button>
	</header>

	{#if settingsStore.lastError}
		<!-- Persistence failure banner -->
		<div
			class="flex items-center gap-2 px-4 py-1.5 text-xs text-amber-200 bg-amber-500/10 border-b border-amber-500/20"
		>
			<AlertTriangle class="w-3.5 h-3.5 flex-shrink-0" />
			<span>{settingsStore.lastError}</span>
		</div>
	{/if}

	<!-- Segmented Tab Bar -->
	<div class="px-5 pt-3.5 pb-2 flex-shrink-0 bg-zinc-950">
		<div class="flex items-center bg-zinc-900 border border-white/10 p-0.5 rounded-xl max-w-lg">
			{#each TAB_IDS as tab (tab)}
				{@const meta = TAB_META[tab]}
				<button
					type="button"
					onclick={() => switchTab(tab)}
					class="flex-1 flex items-center justify-center gap-1.5 py-1.5 text-xs font-medium rounded-lg transition-all cursor-pointer {currentTab === tab
						? 'bg-white/10 text-white shadow-sm'
						: 'text-zinc-400 hover:text-zinc-200'}"
				>
					<meta.icon class="w-3.5 h-3.5" />
					<span>{meta.label}</span>
				</button>
			{/each}
		</div>
	</div>

	<!-- Main Content Area -->
	<main class="flex-1 overflow-y-auto px-5 py-3">
		{#if currentTab === 'reminders'}
			<RemindersTab />
		{:else if currentTab === 'sounds'}
			<SoundsTab />
		{:else if currentTab === 'inactivity'}
			<InactivityTab />
		{:else}
			<StatsTab />
		{/if}
	</main>
</div>
