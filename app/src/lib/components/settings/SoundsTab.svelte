<script lang="ts">
	import { Minus, Play, Plus, Volume2 } from 'lucide-svelte';
	import { settingsStore } from '$lib/services/settingsStore.svelte';
	import { previewSound } from '$lib/services/soundPlayer';
	import { SOUND_PRESETS } from '$lib/types';
	import SettingSlider from '$lib/components/SettingSlider.svelte';
	import Toggle from '$lib/components/Toggle.svelte';

	let isPlayingPreview = $state(false);
	let previewTimeout: ReturnType<typeof setTimeout> | null = null;

	function handlePreview(presetId?: (typeof SOUND_PRESETS)[number]['id']) {
		const targetPreset = presetId || settingsStore.settings.soundPreset;
		isPlayingPreview = true;
		previewSound(
			targetPreset,
			settingsStore.settings.soundVolume,
			settingsStore.settings.soundFadeIn
		);
		if (previewTimeout) clearTimeout(previewTimeout);
		// still-water is the longest phrase (~5s); keep the label honest
		previewTimeout = setTimeout(() => {
			isPlayingPreview = false;
		}, 5200);
	}

	function adjustSoundRepeat(delta: number) {
		const next = Math.max(1, Math.min(5, (settingsStore.settings.soundRepeat || 1) + delta));
		if (next !== settingsStore.settings.soundRepeat) {
			settingsStore.updateSettings({ soundRepeat: next });
		}
	}
</script>

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
			<Toggle
				checked={settingsStore.settings.soundFadeIn}
				onChange={(v) => settingsStore.updateSettings({ soundFadeIn: v })}
			/>
		</div>

		<p class="text-[11px] text-zinc-500 italic pt-1 border-t border-white/5">
			* Breaks end with the same chime plus an ascending flourish, so "break done" sounds
			different from "break start". Skipped breaks play no end sound.
		</p>
	</section>
</div>
