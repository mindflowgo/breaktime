<script lang="ts">
	import { onMount } from 'svelte';
	import {
		Minimize2,
		Eye,
		Activity,
		Volume2,
		Clock,
		Moon,
		BarChart3,
		CheckCircle2,
		XCircle,
		Play,
		RotateCcw,
		Sparkles,
		Minus,
		Plus,
		AlertTriangle,
		AlarmClockOff
	} from 'lucide-svelte';
	import { enterOverlayMode, listenTauriEvent } from '$lib/services/windowManager';
	import { settingsStore } from '$lib/services/settingsStore.svelte';
	import { timerManager } from '$lib/services/timerManager.svelte';
	import { previewSound, stopSound } from '$lib/services/soundPlayer';
	import { SOUND_PRESETS, type SoundPreset } from '$lib/types';
	import SettingSlider from '$lib/components/SettingSlider.svelte';

	type TabId = 'reminders' | 'sounds' | 'inactivity' | 'stats';
	let currentTab = $state<TabId>('reminders');
	let isPlayingPreview = $state(false);

	onMount(() => {
		const savedTab = typeof localStorage !== 'undefined' ? localStorage.getItem('breaktime_active_tab') : null;
		if (savedTab && ['reminders', 'sounds', 'inactivity', 'stats'].includes(savedTab)) {
			currentTab = savedTab as TabId;
		}

		settingsStore.init();
		timerManager.init();

		const unlistenPromise = listenTauriEvent<TabId>('breaktime:open-tab', (tab) => {
			if (tab && ['reminders', 'sounds', 'inactivity', 'stats'].includes(tab)) {
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

	function handlePreview(preset?: SoundPreset) {
		const targetPreset = preset || settingsStore.settings.soundPreset;
		isPlayingPreview = true;
		previewSound(
			targetPreset,
			settingsStore.settings.soundVolume,
			settingsStore.settings.soundFadeIn
		);
		setTimeout(() => {
			isPlayingPreview = false;
		}, 3000);
	}

	async function handleBackToOverlay() {
		stopSound();
		await enterOverlayMode();
	}

	// Adherence calculations — expired (ignored) prompts count as non-adherent attempts
	let totalAttempts = $derived(
		settingsStore.stats.totalCompleted +
			settingsStore.stats.totalSkipped +
			settingsStore.stats.totalExpired
	);

	let overallAdherence = $derived.by(() => {
		if (totalAttempts <= 0) return 100;
		return Math.round((settingsStore.stats.totalCompleted / totalAttempts) * 100);
	});

	let eyeTotal = $derived(
		settingsStore.stats.eyeCompleted + settingsStore.stats.eyeSkipped + settingsStore.stats.eyeExpired
	);

	let eyeAdherence = $derived.by(() => {
		if (eyeTotal <= 0) return 100;
		return Math.round((settingsStore.stats.eyeCompleted / eyeTotal) * 100);
	});

	let stretchTotal = $derived(
		settingsStore.stats.stretchCompleted +
			settingsStore.stats.stretchSkipped +
			settingsStore.stats.stretchExpired
	);

	function adjustSoundRepeat(delta: number) {
		const next = Math.max(1, Math.min(5, (settingsStore.settings.soundRepeat || 1) + delta));
		if (next !== settingsStore.settings.soundRepeat) {
			settingsStore.updateSettings({ soundRepeat: next });
		}
	}

	let stretchAdherence = $derived.by(() => {
		if (stretchTotal <= 0) return 100;
		return Math.round((settingsStore.stats.stretchCompleted / stretchTotal) * 100);
	});

	function formatTimeAgo(ts: number): string {
		const diffMs = Date.now() - ts;
		const diffMins = Math.floor(diffMs / 60000);
		if (diffMins < 1) return 'Just now';
		if (diffMins < 60) return `${diffMins}m ago`;
		const diffHours = Math.floor(diffMins / 60);
		if (diffHours < 24) return `${diffHours}h ago`;
		return new Date(ts).toLocaleDateString([], { month: 'short', day: 'numeric' });
	}
</script>

<div class="h-screen w-screen flex flex-col bg-zinc-950 text-zinc-100 overflow-hidden font-sans">
	<!-- Top Window Header -->
	<header
		class="h-14 border-b border-white/10 px-4 flex items-center justify-between flex-shrink-0 bg-zinc-900/60 backdrop-blur-md"
		data-tauri-drag-region
	>
		<!-- Left: Brand / Title -->
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

		<!-- Right: Back to Overlay Button -->
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
			<button
				type="button"
				onclick={() => switchTab('reminders')}
				class="flex-1 flex items-center justify-center gap-1.5 py-1.5 text-xs font-medium rounded-lg transition-all cursor-pointer {currentTab ===
				'reminders'
					? 'bg-white/10 text-white shadow-sm'
					: 'text-zinc-400 hover:text-zinc-200'}"
			>
				<Clock class="w-3.5 h-3.5" />
				<span>Reminders</span>
			</button>

			<button
				type="button"
				onclick={() => switchTab('sounds')}
				class="flex-1 flex items-center justify-center gap-1.5 py-1.5 text-xs font-medium rounded-lg transition-all cursor-pointer {currentTab ===
				'sounds'
					? 'bg-white/10 text-white shadow-sm'
					: 'text-zinc-400 hover:text-zinc-200'}"
			>
				<Volume2 class="w-3.5 h-3.5" />
				<span>Sounds</span>
			</button>

			<button
				type="button"
				onclick={() => switchTab('inactivity')}
				class="flex-1 flex items-center justify-center gap-1.5 py-1.5 text-xs font-medium rounded-lg transition-all cursor-pointer {currentTab ===
				'inactivity'
					? 'bg-white/10 text-white shadow-sm'
					: 'text-zinc-400 hover:text-zinc-200'}"
			>
				<Moon class="w-3.5 h-3.5" />
				<span>Inactivity</span>
			</button>

			<button
				type="button"
				onclick={() => switchTab('stats')}
				class="flex-1 flex items-center justify-center gap-1.5 py-1.5 text-xs font-medium rounded-lg transition-all cursor-pointer {currentTab ===
				'stats'
					? 'bg-white/10 text-white shadow-sm'
					: 'text-zinc-400 hover:text-zinc-200'}"
			>
				<BarChart3 class="w-3.5 h-3.5" />
				<span>Stats & History</span>
			</button>
		</div>
	</div>

	<!-- Main Content Area -->
	<main class="flex-1 overflow-y-auto px-5 py-3">
		{#if currentTab === 'reminders'}
			<!-- REMINDERS TAB -->
			<div class="space-y-4 max-w-2xl">
				<!-- Header Row with Reset Cycles -->
				<div class="flex items-center justify-between pb-0.5">
					<div>
						<h2 class="text-xs font-semibold uppercase tracking-wider text-zinc-400">
							Reminder Schedules
						</h2>
						<p class="text-[11px] text-zinc-500">Custom intervals and guided break durations</p>
					</div>
					<button
						type="button"
						onclick={() => timerManager.resetTimers()}
						class="flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium text-zinc-300 hover:text-white bg-white/5 hover:bg-white/10 border border-white/10 rounded-lg transition-colors cursor-pointer"
						title="Restart countdowns from their full intervals"
					>
						<RotateCcw class="w-3.5 h-3.5 text-zinc-400" />
						<span>Reset Cycles</span>
					</button>
				</div>

				<!-- Eye Reset Card -->
				<section class="rounded-xl border border-white/10 bg-zinc-900/40 p-4">
					<div class="flex items-center justify-between pb-3 border-b border-white/5">
						<div class="flex items-center gap-2.5">
							<div
								class="w-7 h-7 rounded-lg bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 flex items-center justify-center"
							>
								<Eye class="w-4 h-4" />
							</div>
							<div>
								<h2 class="text-sm font-semibold text-white">Eye Reset</h2>
								<p class="text-xs text-zinc-400">20-20-20 screen break to relax focus</p>
							</div>
						</div>

						<!-- Enabled Toggle -->
						<label class="relative inline-flex items-center cursor-pointer">
							<input
								type="checkbox"
								checked={settingsStore.settings.eyeEnabled}
								onchange={(e) =>
									settingsStore.updateSettings({
										eyeEnabled: (e.target as HTMLInputElement).checked
									})}
								class="sr-only peer"
							/>
							<div
								class="w-9 h-5 bg-zinc-800 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-zinc-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-emerald-500"
							></div>
						</label>
					</div>

					<div class="pt-3.5 space-y-3.5">
						<!-- Interval row -->
						<SettingSlider
							label="Interval"
							desc="How often to trigger reminder"
							min={5}
							max={60}
							step={5}
							value={settingsStore.settings.eyeIntervalMin}
							accent="emerald"
							format={(v) => `${v} min`}
							onCommit={(v) => settingsStore.updateSettings({ eyeIntervalMin: v })}
						/>

						<!-- Duration row -->
						<SettingSlider
							label="Duration"
							desc="Length of the guided break"
							min={10}
							max={60}
							step={5}
							value={settingsStore.settings.eyeDurationSec}
							accent="emerald"
							format={(v) => `${v} sec`}
							onCommit={(v) => settingsStore.updateSettings({ eyeDurationSec: v })}
						/>

						<p class="text-[11px] text-zinc-500 italic pt-1 border-t border-white/5">
							* Includes guided animation with 10 counted blinks to relieve eye fatigue.
						</p>
					</div>
				</section>

				<!-- Stretch Break Card -->
				<section class="rounded-xl border border-white/10 bg-zinc-900/40 p-4">
					<div class="flex items-center justify-between pb-3 border-b border-white/5">
						<div class="flex items-center gap-2.5">
							<div
								class="w-7 h-7 rounded-lg bg-amber-500/20 text-amber-300 border border-amber-500/30 flex items-center justify-center"
							>
								<Activity class="w-4 h-4" />
							</div>
							<div>
								<h2 class="text-sm font-semibold text-white">Move Break</h2>
								<p class="text-xs text-zinc-400">Postural reset & light movement</p>
							</div>
						</div>

						<!-- Enabled Toggle -->
						<label class="relative inline-flex items-center cursor-pointer">
							<input
								type="checkbox"
								checked={settingsStore.settings.stretchEnabled}
								onchange={(e) =>
									settingsStore.updateSettings({
										stretchEnabled: (e.target as HTMLInputElement).checked
									})}
								class="sr-only peer"
							/>
							<div
								class="w-9 h-5 bg-zinc-800 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-zinc-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-amber-500"
							></div>
						</label>
					</div>

					<div class="pt-3.5 space-y-3.5">
						<!-- Interval row -->
						<SettingSlider
							label="Interval"
							desc="How often to trigger reminder"
							min={15}
							max={120}
							step={5}
							value={settingsStore.settings.stretchIntervalMin}
							accent="amber"
							format={(v) => `${v} min`}
							onCommit={(v) => settingsStore.updateSettings({ stretchIntervalMin: v })}
						/>

						<!-- Duration row -->
						<SettingSlider
							label="Duration"
							desc="Length of the movement break"
							min={30}
							max={180}
							step={15}
							value={settingsStore.settings.stretchDurationSec}
							accent="amber"
							format={(v) => `${v} sec`}
							onCommit={(v) => settingsStore.updateSettings({ stretchDurationSec: v })}
						/>

						<p class="text-[11px] text-zinc-500 italic pt-1 border-t border-white/5">
							* Rotates guided exercises: Stand & walk • Stretch arms • Roll shoulders • Gentle squats.
						</p>
					</div>
				</section>
			</div>
		{:else if currentTab === 'sounds'}
			<!-- SOUNDS TAB -->
			<div class="space-y-4 max-w-2xl">
				<!-- Sound Presets Grid -->
				<section class="rounded-xl border border-white/10 bg-zinc-900/40 p-4">
					<div class="flex items-center justify-between pb-3 border-b border-white/5">
						<div>
							<h2 class="text-sm font-semibold text-white">Calm Sound Presets</h2>
							<p class="text-xs text-zinc-400">Gentle acoustic tones, not jarring alarms</p>
						</div>
						<button
							type="button"
							onclick={() => handlePreview()}
							class="flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-300 border border-emerald-500/30 rounded-lg transition-colors cursor-pointer"
						>
							<Play class="w-3 h-3 fill-current" />
							<span>{isPlayingPreview ? 'Playing...' : 'Play Preview'}</span>
						</button>
					</div>

					<div class="grid grid-cols-2 gap-2.5 pt-3">
						{#each SOUND_PRESETS as preset}
							{@const isSelected = settingsStore.settings.soundPreset === preset.id}
							<!-- svelte-ignore a11y_click_events_have_key_events -->
							<div
								role="button"
								tabindex="0"
								onclick={() => settingsStore.updateSettings({ soundPreset: preset.id })}
								onkeydown={(e) => {
									if (e.key === 'Enter' || e.key === ' ') {
										e.preventDefault();
										settingsStore.updateSettings({ soundPreset: preset.id });
									}
								}}
								class="flex flex-col text-left p-2.5 rounded-lg border transition-all cursor-pointer relative {isSelected
									? 'border-emerald-500/50 bg-emerald-500/10'
									: 'border-white/10 bg-zinc-900/50 hover:border-white/20'}"
							>
								<div class="flex items-center justify-between w-full">
									<span class="text-xs font-semibold" class:text-emerald-300={isSelected}>
										{preset.name}
									</span>
									<button
										type="button"
										onclick={(e) => {
											e.stopPropagation();
											handlePreview(preset.id);
										}}
										class="p-1 rounded text-zinc-400 hover:text-white hover:bg-white/10 transition-colors"
										title="Preview {preset.name}"
									>
										<Volume2 class="w-3.5 h-3.5" />
									</button>
								</div>
								<span class="text-[11px] text-zinc-400 mt-0.5">{preset.style}</span>
								<span class="text-[10px] text-zinc-500 mt-1 line-clamp-1">
									{preset.description}
								</span>
							</div>
						{/each}
					</div>
				</section>

				<!-- Volume, Repeat & Fade Controls -->
				<section class="rounded-xl border border-white/10 bg-zinc-900/40 p-4 space-y-3.5">
					<!-- Volume slider -->
					<SettingSlider
						label="Volume"
						desc="Synthesizer master gain level"
						min={0.1}
						max={1.0}
						step={0.05}
						value={settingsStore.settings.soundVolume}
						accent="emerald"
						format={(v) => `${Math.round(v * 100)}%`}
						onCommit={(v) => settingsStore.updateSettings({ soundVolume: v })}
					/>

					<!-- Repeat stepper -->
					<div class="flex items-center justify-between gap-4">
						<div>
							<span class="text-xs font-medium text-zinc-200">Repeat</span>
							<p class="text-[11px] text-zinc-500">
								How many times the phrase plays per reminder (1–5)
							</p>
						</div>
						<div class="flex items-center gap-2">
							<button
								type="button"
								onclick={() => adjustSoundRepeat(-1)}
								disabled={(settingsStore.settings.soundRepeat || 1) <= 1}
								class="w-7 h-7 rounded-lg bg-white/5 hover:bg-white/10 border border-white/10 text-zinc-300 flex items-center justify-center transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-default"
								title="Fewer repetitions"
							>
								<Minus class="w-3.5 h-3.5" />
							</button>
							<span class="w-16 text-center text-xs font-mono font-medium text-emerald-300">
								{settingsStore.settings.soundRepeat || 1}×
							</span>
							<button
								type="button"
								onclick={() => adjustSoundRepeat(1)}
								disabled={(settingsStore.settings.soundRepeat || 1) >= 5}
								class="w-7 h-7 rounded-lg bg-white/5 hover:bg-white/10 border border-white/10 text-zinc-300 flex items-center justify-center transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-default"
								title="More repetitions"
							>
								<Plus class="w-3.5 h-3.5" />
							</button>
						</div>
					</div>

					<!-- Fade-in Toggle -->
					<div class="flex items-center justify-between gap-4 pt-2 border-t border-white/5">
						<div>
							<span class="text-xs font-medium text-zinc-200">Fade-In Gradually</span>
							<p class="text-[11px] text-zinc-500">
								Ramps volume smoothly over 600ms so alerts never startle you
							</p>
						</div>
						<label class="relative inline-flex items-center cursor-pointer">
							<input
								type="checkbox"
								checked={settingsStore.settings.soundFadeIn}
								onchange={(e) =>
									settingsStore.updateSettings({
										soundFadeIn: (e.target as HTMLInputElement).checked
									})}
								class="sr-only peer"
							/>
							<div
								class="w-9 h-5 bg-zinc-800 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-zinc-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-emerald-500"
							></div>
						</label>
					</div>
				</section>
			</div>
		{:else if currentTab === 'inactivity'}
			<!-- INACTIVITY TAB -->
			<div class="space-y-4 max-w-2xl">
				<!-- Inactivity monitor info card -->
				<section class="rounded-xl border border-white/10 bg-zinc-900/40 p-4">
					<div class="flex items-center justify-between pb-3 border-b border-white/5">
						<div class="flex items-center gap-2.5">
							<div
								class="w-7 h-7 rounded-lg bg-indigo-500/20 text-indigo-300 border border-indigo-500/30 flex items-center justify-center"
							>
								<Moon class="w-4 h-4" />
							</div>
							<div>
								<h2 class="text-sm font-semibold text-white">System Idle Monitoring</h2>
								<p class="text-xs text-zinc-400">
									Pauses timers while you are away from your workstation
								</p>
							</div>
						</div>

						<!-- Live Idle Status Pill -->
						<div
							class="px-2.5 py-1 rounded-full border text-[11px] font-medium flex items-center gap-1.5"
							class:border-indigo-500-30={timerManager.isIdle}
							class:bg-indigo-500-15={timerManager.isIdle}
							class:text-indigo-300={timerManager.isIdle}
							class:border-emerald-500-30={!timerManager.isIdle}
							class:bg-emerald-500-15={!timerManager.isIdle}
							class:text-emerald-300={!timerManager.isIdle}
							style={timerManager.isIdle
								? 'background: rgba(99, 102, 241, 0.15); border-color: rgba(99, 102, 241, 0.3);'
								: 'background: rgba(16, 185, 129, 0.15); border-color: rgba(16, 185, 129, 0.3);'}
						>
							<span
								class="w-1.5 h-1.5 rounded-full"
								class:bg-indigo-400={timerManager.isIdle}
								class:bg-emerald-400={!timerManager.isIdle}
							></span>
							<span>{timerManager.isIdle ? 'Away (Paused)' : 'Active (Tracking)'}</span>
						</div>
					</div>

					<div class="pt-3.5 space-y-4">
						<!-- Idle Threshold -->
						<SettingSlider
							label="Idle Threshold"
							desc="Inactivity duration before entering away state"
							min={60}
							max={900}
							step={60}
							value={settingsStore.settings.idleThresholdSec}
							accent="indigo"
							format={(v) => `${Math.round(v / 60)} min`}
							onCommit={(v) => settingsStore.updateSettings({ idleThresholdSec: v })}
						/>

						<!-- Away Reset Threshold -->
						<div class="pt-2 border-t border-white/5">
							<SettingSlider
								label="Away Reset Threshold"
								desc="If away for this duration or longer, reset timers on return"
								min={5}
								max={30}
								step={1}
								value={settingsStore.settings.awayResetMin}
								accent="indigo"
								format={(v) => `${v} min`}
								onCommit={(v) => settingsStore.updateSettings({ awayResetMin: v })}
							/>
						</div>

						<div class="text-[11px] text-zinc-500 bg-zinc-950/60 p-2.5 rounded-lg border border-white/5">
							💡 When away, the HUD displays <strong>💤 Away</strong>. Stepping away for a coffee or
							meeting will reset your timer cycles automatically when you return.
						</div>
					</div>
				</section>
			</div>
		{:else if currentTab === 'stats'}
			<!-- STATS & HISTORY TAB -->
			<div class="space-y-4 max-w-2xl">
				<!-- Overview Cards -->
				<div class="grid grid-cols-4 gap-3">
					<div class="rounded-xl border border-white/10 bg-zinc-900/40 p-3 flex flex-col">
						<span class="text-[11px] font-medium text-zinc-400">Total Completed</span>
						<span class="text-xl font-bold font-mono text-emerald-400 mt-1">
							{settingsStore.stats.totalCompleted}
						</span>
						<span class="text-[10px] text-zinc-500 mt-0.5">Breaks finished</span>
					</div>

					<div class="rounded-xl border border-white/10 bg-zinc-900/40 p-3 flex flex-col">
						<span class="text-[11px] font-medium text-zinc-400">Total Skipped</span>
						<span class="text-xl font-bold font-mono text-zinc-400 mt-1">
							{settingsStore.stats.totalSkipped}
						</span>
						<span class="text-[10px] text-zinc-500 mt-0.5">Dismissed breaks</span>
					</div>

					<div class="rounded-xl border border-white/10 bg-zinc-900/40 p-3 flex flex-col">
						<span class="text-[11px] font-medium text-zinc-400">Expired</span>
						<span class="text-xl font-bold font-mono text-indigo-400 mt-1">
							{settingsStore.stats.totalExpired}
						</span>
						<span class="text-[10px] text-zinc-500 mt-0.5">Ignored prompts</span>
					</div>

					<div class="rounded-xl border border-white/10 bg-zinc-900/40 p-3 flex flex-col">
						<span class="text-[11px] font-medium text-zinc-400">Adherence Rate</span>
						<span
							class="text-xl font-bold font-mono mt-1"
							class:text-emerald-400={overallAdherence >= 80}
							class:text-amber-400={overallAdherence >= 50 && overallAdherence < 80}
							class:text-zinc-400={overallAdherence < 50}
						>
							{overallAdherence}%
						</span>
						<span class="text-[10px] text-zinc-500 mt-0.5">Completion ratio</span>
					</div>
				</div>

				<!-- Breakdown by Type -->
				<section class="rounded-xl border border-white/10 bg-zinc-900/40 p-4">
					<h2 class="text-xs font-semibold uppercase tracking-wider text-zinc-400 mb-3">
						Breakdown by Type
					</h2>

					<div class="grid grid-cols-2 gap-3">
						<!-- Eye Breaks Breakdown -->
						<div class="p-3 rounded-lg bg-zinc-950/60 border border-white/5 space-y-2">
							<div class="flex items-center justify-between">
								<span class="text-xs font-medium text-emerald-300 flex items-center gap-1.5">
									<Eye class="w-3.5 h-3.5" /> Eye Reset
								</span>
								<span class="text-xs font-mono font-semibold text-emerald-300">
									{eyeAdherence}%
								</span>
							</div>
							<div class="w-full h-1.5 bg-zinc-800 rounded-full overflow-hidden">
								<div
									class="h-full bg-emerald-400 rounded-full transition-all"
									style="width: {eyeAdherence}%"
								></div>
							</div>
							<div class="flex justify-between text-[11px] text-zinc-500">
								<span>Completed: {settingsStore.stats.eyeCompleted}</span>
								<span>Skipped: {settingsStore.stats.eyeSkipped}</span>
								<span>Expired: {settingsStore.stats.eyeExpired}</span>
							</div>
						</div>

						<!-- Move Breaks Breakdown -->
						<div class="p-3 rounded-lg bg-zinc-950/60 border border-white/5 space-y-2">
							<div class="flex items-center justify-between">
								<span class="text-xs font-medium text-amber-300 flex items-center gap-1.5">
									<Activity class="w-3.5 h-3.5" /> Move Break
								</span>
								<span class="text-xs font-mono font-semibold text-amber-300">
									{stretchAdherence}%
								</span>
							</div>
							<div class="w-full h-1.5 bg-zinc-800 rounded-full overflow-hidden">
								<div
									class="h-full bg-amber-400 rounded-full transition-all"
									style="width: {stretchAdherence}%"
								></div>
							</div>
							<div class="flex justify-between text-[11px] text-zinc-500">
								<span>Completed: {settingsStore.stats.stretchCompleted}</span>
								<span>Skipped: {settingsStore.stats.stretchSkipped}</span>
								<span>Expired: {settingsStore.stats.stretchExpired}</span>
							</div>
						</div>
					</div>
				</section>

				<!-- Recent Activity History Log -->
				<section class="rounded-xl border border-white/10 bg-zinc-900/40 p-4">
					<div class="flex items-center justify-between pb-2.5 border-b border-white/5">
						<h2 class="text-xs font-semibold uppercase tracking-wider text-zinc-400">
							Recent Activity
						</h2>
						{#if settingsStore.stats.events.length > 0}
							<button
								type="button"
								onclick={() => {
									if (confirm('Clear all break statistics and activity logs?')) {
										settingsStore.resetStats();
									}
								}}
								class="flex items-center gap-1.5 text-[11px] text-zinc-400 hover:text-rose-400 transition-colors cursor-pointer"
								title="Clear all cumulative stats and history logs"
							>
								<RotateCcw class="w-3 h-3" />
								<span>Reset Stats</span>
							</button>
						{/if}
					</div>

					{#if settingsStore.stats.events.length === 0}
						<div class="py-8 text-center text-xs text-zinc-500">
							No break activity recorded yet. Take your first break to see stats!
						</div>
					{:else}
						<div class="divide-y divide-white/5 max-h-48 overflow-y-auto">
							{#each settingsStore.stats.events.slice(0, 15) as item}
								<div class="py-2 flex items-center justify-between text-xs">
									<div class="flex items-center gap-2">
										{#if item.type === 'eye'}
											<span class="text-emerald-400 flex items-center gap-1">
												<Eye class="w-3.5 h-3.5" />
												<span class="font-medium">Eye Reset</span>
											</span>
										{:else}
											<span class="text-amber-400 flex items-center gap-1">
												<Activity class="w-3.5 h-3.5" />
												<span class="font-medium">Move Break</span>
											</span>
										{/if}
										<span class="text-[11px] text-zinc-500 font-mono">
											({item.durationSec}s)
										</span>
									</div>

										<div class="flex items-center gap-3">
											{#if item.action === 'completed'}
												<span class="flex items-center gap-1 text-[11px] text-emerald-400 font-medium">
													<CheckCircle2 class="w-3 h-3" /> Completed
												</span>
											{:else if item.action === 'skipped'}
												<span class="flex items-center gap-1 text-[11px] text-zinc-400 font-medium">
													<XCircle class="w-3 h-3" /> Skipped
												</span>
											{:else}
												<span class="flex items-center gap-1 text-[11px] text-indigo-400 font-medium">
													<AlarmClockOff class="w-3 h-3" /> Expired
												</span>
											{/if}
										<span class="text-[11px] font-mono text-zinc-500">
											{formatTimeAgo(item.timestamp)}
										</span>
									</div>
								</div>
							{/each}
						</div>
					{/if}
				</section>
			</div>
		{/if}
	</main>
</div>
