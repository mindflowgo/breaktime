<script lang="ts">
	import { Moon } from 'lucide-svelte';
	import { settingsStore } from '$lib/services/settingsStore.svelte';
	import { timerManager } from '$lib/services/timerManager.svelte';
	import SettingSlider from '$lib/components/SettingSlider.svelte';
</script>

<div class="space-y-4 max-w-2xl">
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
				style={timerManager.isIdle
					? 'background: rgba(99, 102, 241, 0.15); border-color: rgba(99, 102, 241, 0.3); color: rgb(165 180 252);'
					: 'background: rgba(16, 185, 129, 0.15); border-color: rgba(16, 185, 129, 0.3); color: rgb(110 231 183);'}
			>
				<span
					class="w-1.5 h-1.5 rounded-full {timerManager.isIdle ? 'bg-indigo-400' : 'bg-emerald-400'}"
				></span>
				<span>{timerManager.isIdle ? 'Away (Paused)' : 'Active (Tracking)'}</span>
			</div>
		</div>

		<div class="pt-3.5 space-y-4">
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
				meeting will reset your timer cycles automatically when you return. Away sessions are
				counted in the stats tracker.
			</div>
		</div>
	</section>
</div>
