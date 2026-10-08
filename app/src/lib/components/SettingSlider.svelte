<script lang="ts">
	/**
	 * Settings slider that updates its display live while dragging but only
	 * persists on release (change event) — avoids a disk write per pixel.
	 */
	let {
		label,
		desc,
		min,
		max,
		step,
		value,
		accent = 'emerald',
		format,
		onCommit
	}: {
		label: string;
		desc?: string;
		min: number;
		max: number;
		step: number;
		value: number;
		accent?: 'emerald' | 'amber' | 'indigo';
		format: (v: number) => string;
		onCommit: (v: number) => void;
	} = $props();

	const ACCENTS: Record<'emerald' | 'amber' | 'indigo', { range: string; text: string }> = {
		emerald: { range: 'accent-emerald-400', text: 'text-emerald-300' },
		amber: { range: 'accent-amber-400', text: 'text-amber-300' },
		indigo: { range: 'accent-indigo-400', text: 'text-indigo-300' }
	};

	// Deliberately capture the initial prop value; the effect below follows
	// later external updates without stomping in-progress drags.
	// svelte-ignore state_referenced_locally
	let local = $state(value);

	$effect(() => {
		local = value;
	});
</script>

<div class="flex items-center justify-between gap-4">
	<div class="min-w-0">
		<span class="text-xs font-medium text-zinc-200">{label}</span>
		{#if desc}
			<p class="text-[11px] text-zinc-500">{desc}</p>
		{/if}
	</div>
	<div class="flex items-center gap-3 w-56">
		<input
			type="range"
			{min}
			{max}
			{step}
			value={local}
			oninput={(e) => (local = Number((e.target as HTMLInputElement).value))}
			onchange={() => onCommit(local)}
			class="flex-1 {ACCENTS[accent].range} cursor-pointer"
		/>
		<span class="w-14 text-right text-xs font-mono font-medium {ACCENTS[accent].text}">
			{format(local)}
		</span>
	</div>
</div>
