<script lang="ts">
	import { Activity, CheckCircle2, Eye, Moon, RotateCcw, XCircle } from 'lucide-svelte';
	import { settingsStore } from '$lib/services/settingsStore.svelte';
	import type { Settings } from '$lib/types';

	const pct = (completed: number, total: number): number =>
		total <= 0 ? 100 : Math.round((completed / total) * 100);

	let overallAdherence = $derived(
		pct(
			settingsStore.stats.totalCompleted,
			settingsStore.stats.totalCompleted + settingsStore.stats.totalSkipped
		)
	);
	let eyeAdherence = $derived(
		pct(
			settingsStore.stats.eyeCompleted,
			settingsStore.stats.eyeCompleted + settingsStore.stats.eyeSkipped
		)
	);
	let stretchAdherence = $derived(
		pct(
			settingsStore.stats.stretchCompleted,
			settingsStore.stats.stretchCompleted + settingsStore.stats.stretchSkipped
		)
	);

	function formatTimeAgo(ts: number): string {
		const diffMs = Date.now() - ts;
		const diffMins = Math.floor(diffMs / 60000);
		if (diffMins < 1) return 'Just now';
		if (diffMins < 60) return `${diffMins}m ago`;
		const diffHours = Math.floor(diffMins / 60);
		if (diffHours < 24) return `${diffHours}h ago`;
		return new Date(ts).toLocaleDateString([], { month: 'short', day: 'numeric' });
	}

	/** e.g. Jan-3 16:34 */
	function formatActivityTimestamp(ts: number): string {
		const d = new Date(ts);
		const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
		const hours = String(d.getHours()).padStart(2, '0');
		const mins = String(d.getMinutes()).padStart(2, '0');
		return `${months[d.getMonth()]}-${d.getDate()} ${hours}:${mins}`;
	}

	function formatDuration(sec: number): string {
		if (sec <= 0) return '0s';
		if (sec < 60) return `${sec}s`;
		const m = Math.floor(sec / 60);
		const s = sec % 60;
		if (s === 0) return `${m}m`;
		return `${m}m ${s}s`;
	}
</script>

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

	<!-- Away summary -->
	{#if (settingsStore.stats.awayCount || 0) > 0}
		<div class="rounded-xl border border-white/10 bg-zinc-900/40 p-3 flex items-center justify-between">
			<span class="text-[11px] font-medium text-zinc-400 flex items-center gap-1.5">
				<Moon class="w-3.5 h-3.5 text-indigo-400" /> Away Sessions
			</span>
			<span class="text-xs font-mono text-indigo-300">
				{settingsStore.stats.awayCount} × · {formatDuration(settingsStore.stats.totalAwaySec || 0)} total
			</span>
		</div>
	{/if}

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
					<div class="h-full bg-emerald-400 rounded-full transition-all" style="width: {eyeAdherence}%"></div>
				</div>
				<div class="flex justify-between text-[11px] text-zinc-500">
					<span>Completed: {settingsStore.stats.eyeCompleted}</span>
					<span>Skipped: {settingsStore.stats.eyeSkipped}</span>
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
					<div class="h-full bg-amber-400 rounded-full transition-all" style="width: {stretchAdherence}%"></div>
				</div>
				<div class="flex justify-between text-[11px] text-zinc-500">
					<span>Completed: {settingsStore.stats.stretchCompleted}</span>
					<span>Skipped: {settingsStore.stats.stretchSkipped}</span>
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
			<div class="flex items-center gap-3">
				<!-- Retention window: older events are pruned from stats.json -->
				<label class="flex items-center gap-1.5 text-[11px] text-zinc-400">
					<span>Track back</span>
					<select
						value={settingsStore.settings.statsRetention}
						onchange={(e) =>
							settingsStore.updateSettings({
								statsRetention: (e.target as HTMLSelectElement).value as Settings['statsRetention']
							})}
						class="bg-zinc-900 border border-white/10 rounded-lg px-1.5 py-1 text-[11px] text-zinc-200 cursor-pointer focus:outline-none focus:border-emerald-500/40"
					>
						<option value="1w">1 week</option>
						<option value="1m">1 month</option>
						<option value="6m">6 months</option>
						<option value="forever">Forever</option>
					</select>
				</label>
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
		</div>

		{#if settingsStore.stats.events.length === 0}
			<div class="py-8 text-center text-xs text-zinc-500">
				No break activity recorded yet. Take your first break to see stats!
			</div>
		{:else}
			<div class="divide-y divide-white/5 max-h-48 overflow-y-auto">
				{#each settingsStore.stats.events.slice(0, 20) as item}
					<div class="py-2 flex items-center justify-between text-xs hover:bg-white/[0.02] px-1 rounded-md transition-colors">
						<div class="flex items-center gap-2 min-w-0">
							{#if item.type === 'eye'}
								<span class="text-emerald-400 flex items-center gap-1.5 flex-shrink-0">
									<Eye class="w-3.5 h-3.5" />
									<span class="font-medium">Eye Reset</span>
								</span>
							{:else if item.type === 'stretch'}
								<span class="text-amber-400 flex items-center gap-1.5 flex-shrink-0">
									<Activity class="w-3.5 h-3.5" />
									<span class="font-medium">Move Break</span>
								</span>
							{:else}
								<span class="text-indigo-400 flex items-center gap-1.5 flex-shrink-0">
									<Moon class="w-3.5 h-3.5" />
									<span class="font-medium">Away / Idle</span>
								</span>
							{/if}
							{#if item.durationSec > 0}
								<span class="text-[11px] text-zinc-500 font-mono">
									({formatDuration(item.durationSec)})
								</span>
							{/if}
						</div>

						<div class="flex items-center gap-3 flex-shrink-0">
							{#if item.type === 'away'}
								<!-- Away sessions record one event on return; legacy logs may
								     still carry an old 'away'-action start marker. -->
								<span class="flex items-center gap-1 text-[11px] text-indigo-300 font-medium">
									{(item.action as string) === 'away' ? 'Started' : 'Returned'}
								</span>
							{:else if item.action === 'completed'}
								<span class="flex items-center gap-1 text-[11px] text-emerald-400 font-medium">
									<CheckCircle2 class="w-3 h-3" /> Completed
								</span>
							{:else if item.action === 'skipped'}
								<span class="flex items-center gap-1 text-[11px] text-zinc-400 font-medium">
									<XCircle class="w-3 h-3" /> Skipped
								</span>
							{:else}
								<span class="flex items-center gap-1 text-[11px] text-emerald-400 font-medium">
									<CheckCircle2 class="w-3 h-3" /> Completed
								</span>
							{/if}

							<span
								class="text-[11px] font-mono font-medium text-zinc-300 bg-zinc-800/60 px-1.5 py-0.5 rounded border border-white/5"
								title="{formatTimeAgo(item.timestamp)}"
							>
								{formatActivityTimestamp(item.timestamp)}
							</span>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</section>
</div>
