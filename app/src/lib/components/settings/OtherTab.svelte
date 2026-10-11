<script lang="ts">
	import { onMount } from 'svelte';
	import { Activity, Eye, Moon, RotateCcw } from 'lucide-svelte';
	import { isTauriEnvironment } from '$lib/services/windowManager';
	import { settingsStore } from '$lib/services/settingsStore.svelte';
	import { timerManager } from '$lib/services/timerManager.svelte';
	import SettingSlider from '$lib/components/SettingSlider.svelte';
	import Toggle from '$lib/components/Toggle.svelte';

	let launchAtLogin = $state(false);
	let available = $state(false);
	let busy = $state(false);
	let errorMsg = $state<string | null>(null);

	async function loadPlugin() {
		return await import('@tauri-apps/plugin-autostart');
	}

	onMount(async () => {
		if (!isTauriEnvironment()) return;
		try {
			const autostart = await loadPlugin();
			launchAtLogin = await autostart.isEnabled();
			available = true;
		} catch (err) {
			console.warn('[OtherTab] autostart plugin unavailable:', err);
		}
	});

	async function toggleLaunchAtLogin(value: boolean) {
		if (busy) return;
		busy = true;
		errorMsg = null;
		const previous = launchAtLogin;
		launchAtLogin = value; // optimistic; reverted if the OS write fails
		try {
			const autostart = await loadPlugin();
			if (value) {
				await autostart.enable();
			} else {
				await autostart.disable();
			}
			launchAtLogin = await autostart.isEnabled();
		} catch (err) {
			console.error('[OtherTab] Failed to update launch-at-login:', err);
			launchAtLogin = previous;
			errorMsg = 'Could not update the launch-at-login setting.';
		} finally {
			busy = false;
		}
	}
</script>

<div class="max-w-2xl space-y-6">
	<!-- Reminders -->
	<section>
		<div class="flex items-center justify-between pb-1.5">
			<h2 class="text-xs font-semibold uppercase tracking-wider text-zinc-400">Reminders</h2>
			<button
				type="button"
				onclick={() => timerManager.resetTimers()}
				class="flex items-center gap-1.5 px-2 py-1 text-[11px] font-medium text-zinc-300 hover:text-white bg-white/5 hover:bg-white/10 border border-white/10 rounded-lg transition-colors cursor-pointer"
				title="Restart countdowns from their full intervals"
			>
				<RotateCcw class="w-3 h-3 text-zinc-400" />
				<span>Reset Cycles</span>
			</button>
		</div>

		<div class="divide-y divide-white/5">
			<div class="py-2.5 flex items-center justify-between gap-4">
				<div class="flex items-center gap-2 min-w-0">
					<Eye class="w-3.5 h-3.5 text-emerald-400 flex-shrink-0" />
					<div class="min-w-0">
						<span class="text-xs font-medium text-zinc-200">Eye Reset</span>
						<p class="text-[11px] text-zinc-500">20-20-20 screen break to relax focus</p>
					</div>
				</div>
				<Toggle
					checked={settingsStore.settings.eyeEnabled}
					onChange={(v) => settingsStore.updateSettings({ eyeEnabled: v })}
				/>
			</div>

			<div class="py-2.5">
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
			</div>

			<div class="py-2.5">
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
			</div>

			<div class="py-2.5 flex items-center justify-between gap-4">
				<div class="flex items-center gap-2 min-w-0">
					<Activity class="w-3.5 h-3.5 text-amber-400 flex-shrink-0" />
					<div class="min-w-0">
						<span class="text-xs font-medium text-zinc-200">Move Break</span>
						<p class="text-[11px] text-zinc-500">Postural reset & light movement</p>
					</div>
				</div>
				<Toggle
					accent="amber"
					checked={settingsStore.settings.stretchEnabled}
					onChange={(v) => settingsStore.updateSettings({ stretchEnabled: v })}
				/>
			</div>

			<div class="py-2.5">
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
			</div>

			<div class="py-2.5">
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
			</div>

			<div class="py-2.5">
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
			</div>
		</div>
	</section>

	<!-- Inactivity -->
	<section>
		<div class="flex items-center justify-between pb-1.5">
			<h2 class="text-xs font-semibold uppercase tracking-wider text-zinc-400 flex items-center gap-1.5">
				<Moon class="w-3.5 h-3.5 text-indigo-400" />
				Inactivity
			</h2>

			<!-- Live idle status pill -->
			<div
				class="px-2 py-0.5 rounded-full border text-[11px] font-medium flex items-center gap-1.5"
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

		<div class="divide-y divide-white/5">
			<div class="py-2.5">
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
			</div>

			<div class="py-2.5">
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
		</div>
	</section>

	<!-- General -->
	<section>
		<div class="pb-1.5">
			<h2 class="text-xs font-semibold uppercase tracking-wider text-zinc-400">General</h2>
		</div>

		<div class="divide-y divide-white/5">
			<div class="py-2.5 flex items-center justify-between gap-4">
				<div class="min-w-0">
					<span class="text-xs font-medium text-zinc-200">Launch at Login</span>
					<p class="text-[11px] text-zinc-500">
						{#if !available}
							Available in the installed desktop app.
						{:else if errorMsg}
							<span class="text-amber-300">{errorMsg}</span>
						{:else}
							Start Breaktime automatically when you log in.
						{/if}
					</p>
				</div>
				<Toggle
					checked={launchAtLogin}
					disabled={!available || busy}
					onChange={toggleLaunchAtLogin}
				/>
			</div>
		</div>
	</section>
</div>
