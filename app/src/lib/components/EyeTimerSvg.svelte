<script lang="ts">
	interface Props {
		/** Progress from 1.0 (full ring) down to 0.0 (empty ring) */
		progress?: number;
		/** Whether the eye break countdown is currently running */
		isBreakActive?: boolean;
		/** Whether user is idle/away */
		isIdle?: boolean;
		/** Whether timers are paused */
		isPaused?: boolean;
		/** Small grey text rendered at bottom-right of the eye */
		timeText?: string;
		/** Extra classes for wrapper */
		class?: string;
	}

	let {
		progress = 1,
		isBreakActive = false,
		isIdle = false,
		isPaused = false,
		timeText = '',
		class: className = ''
	}: Props = $props();

	// Iris center & countdown pomo ring geometry around iris
	const CX = 50;
	const CY = 34;
	const RING_RADIUS = 22.5;
	const CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS; // ~141.37

	// Clamp progress between 0 and 1
	let clampedProgress = $derived(Math.max(0, Math.min(1, progress)));

	// Ring countdown: visible length starts at full CIRCUMFERENCE and shrinks to 0.
	// As progress decreases, the start angle moves clockwise from 12 o'clock (-90deg),
	// pulling away from top and moving clockwise until empty.
	let visibleLength = $derived(CIRCUMFERENCE * clampedProgress);
	let startAngle = $derived(-90 + (1 - clampedProgress) * 360);
</script>

<div
	class="relative inline-flex items-center justify-center select-none {className}"
	data-tauri-drag-region
>
	<svg
		viewBox="0 0 100 66"
		class="w-full h-full overflow-visible select-none"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		data-tauri-drag-region
	>
		<defs>
			<!-- Glow for warm orange countdown ring -->
			<filter id="pomo-orange-glow" x="-30%" y="-30%" width="160%" height="160%">
				<feDropShadow dx="0" dy="0" stdDeviation="2" flood-color="#f97316" flood-opacity="0.9" />
			</filter>
			<!-- Soft contrast shadow for eye contour over any desktop wallpaper -->
			<filter id="eye-contrast-shadow" x="-20%" y="-20%" width="140%" height="140%">
				<feDropShadow dx="0" dy="1.5" stdDeviation="2.5" flood-color="#000000" flood-opacity="0.45" />
			</filter>
			<!-- Linear gradient for upper eyelid (warm orange into apricot) -->
			<linearGradient id="upperEyelidGrad" x1="0%" y1="0%" x2="100%" y2="0%">
				<stop offset="0%" stop-color="#ff9957" />
				<stop offset="50%" stop-color="#ffb15a" />
				<stop offset="100%" stop-color="#ff9957" />
			</linearGradient>
			<!-- Linear gradient for vibrant cyan iris -->
			<linearGradient id="irisGrad" x1="0%" y1="0%" x2="0%" y2="100%">
				<stop offset="0%" stop-color="#3bd3ef" />
				<stop offset="100%" stop-color="#22b5e2" />
			</linearGradient>
		</defs>

		<!-- Sclera (pure white eyeball fill with subtle contrast backing) -->
		<path
			d="M 6 34 C 24 6, 76 6, 94 34 C 76 60, 24 60, 6 34 Z"
			fill="#ffffff"
			filter="url(#eye-contrast-shadow)"
		/>

		<!-- Vibrant Cyan Iris (centered at 50, 34; radius: 21) -->
		<circle
			cx={CX}
			cy={CY}
			r="21"
			fill="url(#irisGrad)"
		/>

		<!-- Friendly Slate-Grey Pupil (centered at 50, 31.5; radius: 9.5) -->
		<circle
			cx={CX}
			cy="31.5"
			r="9.5"
			fill="#586471"
		/>

		<!-- Pure White Specular Highlight at top-right of pupil -->
		<circle
			cx="56.5"
			cy="24.5"
			r="3.2"
			fill="#ffffff"
		/>

		<!-- POMO TIMER RING around the iris -->
		<!-- Warm orange line matching eyelids that pulls back clockwise from top -->
		{#if isBreakActive && clampedProgress > 0}
			<!-- Subtle track underneath -->
			<circle
				cx={CX}
				cy={CY}
				r={RING_RADIUS}
				stroke="rgba(255, 153, 87, 0.25)"
				stroke-width="3"
				fill="none"
			/>

			<!-- Decreasing Orange Ring: pulls back from top then clockwise it moves away -->
			<circle
				cx={CX}
				cy={CY}
				r={RING_RADIUS}
				stroke="#f97316"
				stroke-width="3.5"
				stroke-linecap="round"
				stroke-dasharray="{visibleLength} {CIRCUMFERENCE}"
				stroke-dashoffset="0"
				transform="rotate({startAngle} {CX} {CY})"
				fill="none"
				filter="url(#pomo-orange-glow)"
				class="transition-[stroke-dasharray,transform] duration-300 ease-linear"
			/>
		{/if}

		<!-- Top Eyelid Arc (thick warm orange curve) -->
		<path
			d="M 6 34 C 24 6, 76 6, 94 34"
			stroke="url(#upperEyelidGrad)"
			stroke-width="6.5"
			stroke-linecap="round"
			fill="none"
		/>

		<!-- Bottom Eyelid Arc (thinner warm orange curve) -->
		<path
			d="M 94 34 C 76 60, 24 60, 6 34"
			stroke="#ffb15a"
			stroke-width="4"
			stroke-linecap="round"
			fill="none"
		/>
	</svg>

	<!-- Very small grey letters for time to close eye (at bottom-right of eye icon) -->
	{#if timeText}
		<span
			class="absolute right-1 bottom-0 text-[9.5px] font-mono font-medium text-zinc-400 tracking-tight leading-none pointer-events-none select-none drop-shadow-[0_1px_1px_rgba(0,0,0,0.8)]"
		>
			{timeText}
		</span>
	{/if}
</div>
