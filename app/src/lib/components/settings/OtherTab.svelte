<script lang="ts">
	import { onMount } from 'svelte';
	import { Activity, Eye, Moon, RotateCcw } from 'lucide-svelte';
	import { isTauriEnvironment } from '$lib/services/windowManager';
	import { settingsStore } from '$lib/services/settingsStore.svelte';
	import { timerManager } from '$lib/services/timerManager.svelte';
	import {
		EYE_DURATION_SEC_OPTIONS,
		EYE_INTERVAL_MIN_OPTIONS,
		STRETCH_DURATION_SEC_OPTIONS,
		STRETCH_INTERVAL_MIN_OPTIONS
	} from '$lib/types';
	import SettingSlider from '$lib/components/SettingSlider.svelte';
	import Toggle from '$lib/components/Toggle.svelte';

	const selectCls =
		'w-20 bg-zinc-900 border border-white/10 rounded-lg px-1.5 py-1 text-[11px] text-zinc-200 cursor-pointer focus:outline-none focus:border-emerald-500/40';

	/** 60 → "1 min", 90 → "1.5 min", 300 → "5 min" */
	const stretchDurationLabel = (sec: number): string =>
		sec % 60 === 0 ? `${sec / 60} min` : `${(sec / 60).toFixed(1)} min`;

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
			<div class="py-2.5 flex items-center justify-between gap-3">
				<div class="flex items-center gap-2 min-w-0">
					<Eye class="w-3.5 h-3.5 text-emerald-400 flex-shrink-0" />
					<span class="text-xs font-medium text-zinc-200 whitespace-nowrap">Eye Break</span>
				</div>
				<div class="flex items-center gap-3 flex-shrink-0">
					<select
						title="How often to remind you"
						class={selectCls}
						value={settingsStore.settings.eyeEnabled
							? settingsStore.settings.eyeIntervalMin
							: 'off'}
						onchange={(e) => {
							const v = (e.target as HTMLSelectElement).value;
							settingsStore.updateSettings(
								v === 'off'
									? { eyeEnabled: false }
									: { eyeEnabled: true, eyeIntervalMin: Number(v) }
							);
						}}
					>
						<option value="off">Off</option>
						{#each EYE_INTERVAL_MIN_OPTIONS as m (m)}
							<option value={m}>{m} min</option>
						{/each}
					</select>
					<label class="flex items-center gap-1.5">
						<span class="text-[11px] text-zinc-500 whitespace-nowrap">Break Duration</span>
						<select
							class={selectCls}
							value={settingsStore.settings.eyeDurationSec}
							onchange={(e) =>
								settingsStore.updateSettings({
									eyeDurationSec: Number((e.target as HTMLSelectElement).value)
								})}
						>
							{#each EYE_DURATION_SEC_OPTIONS as sec (sec)}
								<option value={sec}>{sec} s</option>
							{/each}
						</select>
					</label>
				</div>
			</div>

			<div class="py-2.5 flex items-center justify-between gap-3">
				<div class="flex items-center gap-2 min-w-0">
					<Activity class="w-3.5 h-3.5 text-amber-400 flex-shrink-0" />
					<span class="text-xs font-medium text-zinc-200 whitespace-nowrap">Move Break</span>
				</div>
				<div class="flex items-center gap-3 flex-shrink-0">
					<select
						title="How often to remind you"
						class={selectCls}
						value={settingsStore.settings.stretchEnabled
							? settingsStore.settings.stretchIntervalMin
							: 'off'}
						onchange={(e) => {
							const v = (e.target as HTMLSelectElement).value;
							settingsStore.updateSettings(
								v === 'off'
									? { stretchEnabled: false }
									: { stretchEnabled: true, stretchIntervalMin: Number(v) }
							);
						}}
					>
						<option value="off">Off</option>
						{#each STRETCH_INTERVAL_MIN_OPTIONS as m (m)}
							<option value={m}>{m} min</option>
						{/each}
					</select>
					<label class="flex items-center gap-1.5">
						<span class="text-[11px] text-zinc-500 whitespace-nowrap">Break Duration</span>
						<select
							class={selectCls}
							value={settingsStore.settings.stretchDurationSec}
							onchange={(e) =>
								settingsStore.updateSettings({
									stretchDurationSec: Number((e.target as HTMLSelectElement).value)
								})}
						>
							{#each STRETCH_DURATION_SEC_OPTIONS as sec (sec)}
								<option value={sec}>{stretchDurationLabel(sec)}</option>
							{/each}
						</select>
					</label>
				</div>
			</div>

			<div class="py-2.5">
				<SettingSlider
					label="Overlay Opacity"
					desc="Overlay transparency to appear less intrusive"
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
