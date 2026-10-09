<script lang="ts">
	interface Props {
		/** Countdown ring progress: 1.0 (full ring) down to 0.0 (empty). */
		progress?: number;
		/** Ring color (defaults to amber for stretch breaks). */
		color?: string;
		/** Extra classes for wrapper. */
		class?: string;
	}

	let { progress = 1, color = '#fbbf24', class: className = '' }: Props = $props();

	// Ring geometry centered on the figure's torso
	const CX = 12;
	const CY = 12;
	const RING_RADIUS = 10.6;
	const CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS; // ~66.6

	let clampedProgress = $derived(Math.max(0, Math.min(1, progress)));
	let visibleLength = $derived(CIRCUMFERENCE * clampedProgress);
	let startAngle = $derived(-90 + (1 - clampedProgress) * 360);
</script>

<!-- Standing person (lucide person-standing, currentColor) wrapped in a
     countdown ring. Sized larger than the idle grey person on purpose:
     this is the stretch-break centerpiece. -->
<div
	class="relative inline-flex items-center justify-center select-none {className}"
	data-tauri-drag-region
>
	<svg
		viewBox="0 0 24 24"
		class="w-full h-full overflow-visible select-none"
		fill="none"
		stroke="currentColor"
		stroke-width="1.6"
		stroke-linecap="round"
		stroke-linejoin="round"
		data-tauri-drag-region
	>
		<!-- Head -->
		<circle cx={CX} cy="4.6" r="1.4" fill="currentColor" stroke="none" />

		<!-- Body + legs + reaching arms (stretching pose) -->
		<path d="m8.6 19.8 3.4-6.4 3.4 6.4" />
		<path d="m5.6 7.8 6.4 2.2 6.4-2.2" />
		<path d="M12 10.2v3.2" />

		<!-- Countdown ring around the figure -->
		{#if clampedProgress > 0}
			<circle
				cx={CX}
				cy={CY}
				r={RING_RADIUS}
				stroke="rgba(251, 191, 36, 0.22)"
				stroke-width="1.6"
				fill="none"
			/>
			<circle
				cx={CX}
				cy={CY}
				r={RING_RADIUS}
				stroke={color}
				stroke-width="1.9"
				stroke-linecap="round"
				stroke-dasharray="{visibleLength} {CIRCUMFERENCE}"
				transform="rotate({startAngle} {CX} {CY})"
				fill="none"
				class="transition-[stroke-dasharray,transform] duration-300 ease-linear"
				style="filter: drop-shadow(0 0 1.6px {color});"
			/>
		{/if}
	</svg>
</div>
