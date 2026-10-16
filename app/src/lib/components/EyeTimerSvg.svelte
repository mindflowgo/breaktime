<script lang="ts">
	interface Props {
		/** Progress from 1.0 down to 0.0 for break timer (preserved for interface compatibility) */
		progress?: number;
		/**
		 * Upper eyelid closure 0.0 (fully open) to 1.0 (fully closed) — the
		 * idle eye-timer progress: 0.25 = 25% of the interval elapsed, 1.0 =
		 * the moment the break fires.
		 */
		droop?: number;
		/** Whether the eye break countdown is currently running (triggers quick eyelid blinking) */
		isBreakActive?: boolean;
		/** Whether user is idle/away */
		isIdle?: boolean;
		/** Extra classes for wrapper */
		class?: string;
	}

	let {
		progress = 1,
		droop = 0,
		isBreakActive = false,
		isIdle = false,
		class: className = ''
	}: Props = $props();

	// Iris center
	const CX = 50;
	const CY = 34;

	// Animated droop to simulate natural eye-blinking during active break
	let breakDroop = $state(0);

	$effect(() => {
		if (!isBreakActive) {
			breakDroop = 0;
			return;
		}

		let rafId: number;
		let startTime = performance.now();
		// Rhythmic blink cycle (~1.1s total period)
		const CYCLE_MS = 1100;
		// 325ms blink action: 140ms close + 35ms hold + 150ms open
		const BLINK_START = 600;
		const BLINK_CLOSE = 740;
		const BLINK_HOLD = 775;
		const BLINK_OPEN = 925;

		const tick = (now: number) => {
			const elapsed = (now - startTime) % CYCLE_MS;
			if (elapsed < BLINK_START) {
				breakDroop = 0;
			} else if (elapsed < BLINK_CLOSE) {
				// Eyelid droops down quickly to close
				const t = (elapsed - BLINK_START) / (BLINK_CLOSE - BLINK_START);
				breakDroop = t * t * (3 - 2 * t);
			} else if (elapsed < BLINK_HOLD) {
				breakDroop = 1;
			} else if (elapsed < BLINK_OPEN) {
				// Eyelid returns up quickly to open
				const t = (elapsed - BLINK_HOLD) / (BLINK_OPEN - BLINK_HOLD);
				breakDroop = 1 - t * t * (3 - 2 * t);
			} else {
				breakDroop = 0;
			}
			rafId = requestAnimationFrame(tick);
		};

		rafId = requestAnimationFrame(tick);
		return () => {
			cancelAnimationFrame(rafId);
			breakDroop = 0;
		};
	});

	// Use dynamic breakDroop during break mode, or idle countdown droop during normal tracking
	let effectiveDroop = $derived(isBreakActive ? breakDroop : droop);
	let clampedDroop = $derived(Math.max(0, Math.min(1, effectiveDroop)));

	// Reference-accurate spherical curve control point:
	// Quadratic formula fits: (droop=0 -> cpY=6), (droop=0.5 -> cpY=26), (droop=1.0 -> cpY=60).
	// - At droop=0.00 (100% time left): apex Y=13.0 (fully open)
	// - At droop=0.25 (75% time left): apex Y=19.2 (1/4 down)
	// - At droop=0.50 (50% time left): apex Y=28.0 (50% down, gently arched upward matching reference image)
	// - At droop=0.75 (25% time left): apex Y=39.4 (75% down, sleepy)
	// - At droop=1.00 (0% time left): apex Y=53.5 (100% closed against bottom contour)
	let eyelidCpY = $derived((6 + 26 * clampedDroop + 28 * clampedDroop * clampedDroop).toFixed(2));
	let eyelidPathD = $derived(`M 6 34 C 24 6, 76 6, 94 34 C 76 ${eyelidCpY}, 24 ${eyelidCpY}, 6 34 Z`);
</script>

<div
	class="relative inline-flex items-center justify-center select-none {className}"
	data-tauri-drag-region
>
	{#if isIdle}
		<!-- When away switch the eye into a zzz-icon (in static/zzz-away.svg) -->
		<img
			src="/zzz-away.svg"
			alt="Away (zzz)"
			class="w-[48px] h-[36px] object-contain select-none pointer-events-none drop-shadow-[0_2px_8px_rgba(0,0,0,0.6)] animate-pulse"
			data-tauri-drag-region
		/>
	{:else}
		<svg
			viewBox="0 0 100 66"
			class="w-full h-full overflow-visible select-none"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
			data-tauri-drag-region
		>
			<defs>
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
				<!-- Vibrant cyan iris -->
				<linearGradient id="irisGrad" x1="0%" y1="0%" x2="0%" y2="100%">
					<stop offset="0%" stop-color="#3bd3ef" />
					<stop offset="100%" stop-color="#22b5e2" />
				</linearGradient>
				<!-- Sclera clip boundary to ensure eyelid and eyeball contents stay inside socket -->
				<clipPath id="sclera-clip">
					<path d="M 6 34 C 24 6, 76 6, 94 34 C 76 60, 24 60, 6 34 Z" />
				</clipPath>
			</defs>

			<!-- Sclera (pure white eyeball fill with subtle contrast backing) -->
			<path
				d="M 6 34 C 24 6, 76 6, 94 34 C 76 60, 24 60, 6 34 Z"
				fill="#ffffff"
				filter="url(#eye-contrast-shadow)"
			/>

			<!-- Eyeball socket contents clipped to sclera. -->
			<g clip-path="url(#sclera-clip)">
				<!-- Vibrant Cyan Iris (centered at 50, 34; radius: 21) -->
				<circle cx={CX} cy={CY} r="21" fill="url(#irisGrad)" />

				<!-- Friendly Slate-Grey Pupil (centered at 50, 31.5; radius: 9.5) -->
				<circle cx={CX} cy="31.5" r="9.5" fill="#586471" />

				<!-- Pure White Specular Highlight at top-right of pupil -->
				<circle cx="56.5" cy="24.5" r="3.2" fill="#ffffff" />

				<!-- Upper Eyelid: droops as idle countdown or blinks down quickly during eye break -->
				{#if clampedDroop > 0.005}
					<path d={eyelidPathD} fill="url(#upperEyelidGrad)" />
				{/if}
			</g>

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
	{/if}
</div>
