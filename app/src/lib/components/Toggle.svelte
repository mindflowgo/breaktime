<script lang="ts">
	/** Accessible toggle switch (was copy-pasted three times across tabs). */
	let {
		checked,
		onChange,
		accent = 'emerald',
		disabled = false
	}: {
		checked: boolean;
		onChange: (value: boolean) => void;
		accent?: 'emerald' | 'amber';
		disabled?: boolean;
	} = $props();

	const ON: Record<'emerald' | 'amber', string> = {
		emerald: 'peer-checked:bg-emerald-500',
		amber: 'peer-checked:bg-amber-500'
	};
</script>

<label
	class="relative inline-flex items-center {disabled
		? 'opacity-40 pointer-events-none'
		: 'cursor-pointer'}"
>
	<input
		type="checkbox"
		{checked}
		{disabled}
		onchange={(e) => onChange((e.target as HTMLInputElement).checked)}
		class="sr-only peer"
	/>
	<div
		class="w-9 h-5 bg-zinc-800 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-zinc-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all {ON[accent]}"
	></div>
</label>
