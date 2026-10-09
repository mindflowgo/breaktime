<script lang="ts">
	import { Activity, Eye, RotateCcw } from 'lucide-svelte';
	import { settingsStore } from '$lib/services/settingsStore.svelte';
	import { timerManager } from '$lib/services/timerManager.svelte';
	import SettingSlider from '$lib/components/SettingSlider.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
</script>

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

			<Toggle
				checked={settingsStore.settings.eyeEnabled}
				onChange={(v) => settingsStore.updateSettings({ eyeEnabled: v })}
			/>
		</div>

		<div class="pt-3.5 space-y-3.5">
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
				* The HUD eyelid droops as the interval elapses and the eye blinks through the break.
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

			<Toggle
				accent="amber"
				checked={settingsStore.settings.stretchEnabled}
				onChange={(v) => settingsStore.updateSettings({ stretchEnabled: v })}
			/>
		</div>

		<div class="pt-3.5 space-y-3.5">
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
				* The HUD shows a stretching figure with a countdown ring during the break.
			</p>
		</div>
	</section>

	<!-- HUD Overlay Card -->
	<section class="rounded-xl border border-white/10 bg-zinc-900/40 p-4">
		<div class="pb-3.5">
			<h2 class="text-sm font-semibold text-white">HUD Overlay</h2>
			<p class="text-xs text-zinc-400">Transparency of the floating eye HUD</p>
		</div>

		<SettingSlider
			label="Idle Opacity"
			desc="HUD transparency while idle — always 100% on hover and during breaks"
			min={0.2}
			max={1.0}
			step={0.05}
			value={settingsStore.settings.overlayOpacity}
			accent="indigo"
			format={(v) => `${Math.round(v * 100)}%`}
			onCommit={(v) => settingsStore.updateSettings({ overlayOpacity: v })}
		/>
	</section>
</div>
