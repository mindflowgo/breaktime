import type { SoundPreset } from '$lib/types';

let audioCtx: AudioContext | null = null;
let currentMasterGain: GainNode | null = null;
let activePlayTimeout: ReturnType<typeof setTimeout> | null = null;

function getAudioContext(): AudioContext {
	if (!audioCtx) {
		const AudioContextClass =
			window.AudioContext ||
			(window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
		audioCtx = new AudioContextClass();
	}
	if (audioCtx.state === 'suspended') {
		audioCtx.resume();
	}
	return audioCtx;
}

/**
 * WKWebView starts AudioContexts in "suspended" state until a user gesture
 * occurs. Call this from a pointerdown handler once per window so break
 * sounds can play even when the overlay was never interacted with.
 */
export function warmUpAudioContext(): void {
	if (typeof window === 'undefined') return;
	try {
		const ctx = getAudioContext();
		if (ctx.state === 'suspended') {
			void ctx.resume();
		}
	} catch (err) {
		console.warn('[SoundPlayer] Audio warm-up failed:', err);
	}
}

export function stopSound(): void {
	if (activePlayTimeout) {
		clearTimeout(activePlayTimeout);
		activePlayTimeout = null;
	}

	if (currentMasterGain && audioCtx) {
		try {
			const now = audioCtx.currentTime;
			currentMasterGain.gain.cancelScheduledValues(now);
			currentMasterGain.gain.setValueAtTime(currentMasterGain.gain.value, now);
			currentMasterGain.gain.linearRampToValueAtTime(0.0001, now + 0.08);
			const gainToDisconnect = currentMasterGain;
			setTimeout(() => {
				try {
					gainToDisconnect.disconnect();
				} catch {}
				if (currentMasterGain === gainToDisconnect) {
					currentMasterGain = null;
				}
			}, 100);
		} catch {
			currentMasterGain = null;
		}
	}
}

/**
 * Play a single acoustic tone with attack/decay envelope and optional harmonic overtones
 */
function playTone(
	ctx: AudioContext,
	destination: AudioNode,
	freq: number,
	startTime: number,
	attackSec: number,
	decaySec: number,
	peakGain: number = 0.5,
	waveType: OscillatorType = 'sine',
	harmonicOvertones: { mult: number; gain: number; decayMult?: number }[] = []
) {
	const osc = ctx.createOscillator();
	const gain = ctx.createGain();

	osc.type = waveType;
	osc.frequency.setValueAtTime(freq, startTime);

	gain.gain.setValueAtTime(0.0001, startTime);
	gain.gain.linearRampToValueAtTime(peakGain, startTime + attackSec);
	gain.gain.exponentialRampToValueAtTime(0.0001, startTime + attackSec + decaySec);

	osc.connect(gain);
	gain.connect(destination);

	osc.start(startTime);
	osc.stop(startTime + attackSec + decaySec + 0.1);

	// Overtones for richer acoustic timbre
	for (const overtone of harmonicOvertones) {
		const hOsc = ctx.createOscillator();
		const hGain = ctx.createGain();

		hOsc.type = 'sine';
		hOsc.frequency.setValueAtTime(freq * overtone.mult, startTime);

		const hPeak = peakGain * overtone.gain;
		const hDecay = decaySec * (overtone.decayMult ?? 0.7);

		hGain.gain.setValueAtTime(0.0001, startTime);
		hGain.gain.linearRampToValueAtTime(hPeak, startTime + attackSec);
		hGain.gain.exponentialRampToValueAtTime(0.0001, startTime + attackSec + hDecay);

		hOsc.connect(hGain);
		hGain.connect(destination);

		hOsc.start(startTime);
		hOsc.stop(startTime + attackSec + hDecay + 0.1);
	}
}

/**
 * Schedules one full preset phrase onto the audio graph, starting at
 * `t0` (AudioContext time). Returns the phrase duration in seconds.
 */
function schedulePresetPhrase(
	ctx: AudioContext,
	master: GainNode,
	lowpass: BiquadFilterNode,
	preset: SoundPreset,
	t0: number
): number {
	let presetDuration = 2.0;

	switch (preset) {
		case 'soft-bell': {
			// Gentle Tibetan singing bowl / meditation bell chime with non-harmonic acoustic partials
			lowpass.frequency.setValueAtTime(2800, t0);
			const freq = 587.33; // D5

			// Fundamental tone
			playTone(ctx, master, freq, t0, 0.025, 2.6, 0.65, 'sine', [
				{ mult: 2.0, gain: 0.25, decayMult: 0.8 },
				{ mult: 2.76, gain: 0.18, decayMult: 0.6 }, // Singing bowl modal tierce
				{ mult: 5.4, gain: 0.08, decayMult: 0.4 } // High shimmer
			]);

			// Low warm undertone (D4)
			playTone(ctx, master, freq / 2, t0, 0.035, 2.2, 0.35, 'triangle');
			presetDuration = 2.8;
			break;
		}

		case 'gentle-chime': {
			// Two-tone silver notification chime (E5 -> B5, perfect fifth)
			lowpass.frequency.setValueAtTime(3800, t0);
			const note1 = 659.25; // E5
			const note2 = 987.77; // B5

			playTone(ctx, master, note1, t0, 0.016, 2.0, 0.58, 'sine', [
				{ mult: 2.0, gain: 0.22, decayMult: 0.75 },
				{ mult: 3.0, gain: 0.09, decayMult: 0.5 }
			]);

			playTone(ctx, master, note2, t0 + 0.19, 0.016, 2.3, 0.62, 'sine', [
				{ mult: 2.0, gain: 0.24, decayMult: 0.8 },
				{ mult: 3.0, gain: 0.1, decayMult: 0.55 },
				{ mult: 4.0, gain: 0.04, decayMult: 0.4 }
			]);

			presetDuration = 2.5;
			break;
		}

		case 'calm-drop': {
			// Organic liquid droplet marimba tone with natural pitch glide & ripple echo
			lowpass.frequency.setValueAtTime(2600, t0);

			const osc1 = ctx.createOscillator();
			const gain1 = ctx.createGain();
			osc1.type = 'sine';
			osc1.frequency.setValueAtTime(1080, t0);
			osc1.frequency.exponentialRampToValueAtTime(720, t0 + 0.055);

			gain1.gain.setValueAtTime(0.0001, t0);
			gain1.gain.linearRampToValueAtTime(0.72, t0 + 0.012);
			gain1.gain.exponentialRampToValueAtTime(0.0001, t0 + 0.8);

			osc1.connect(gain1);
			gain1.connect(master);
			osc1.start(t0);
			osc1.stop(t0 + 0.85);

			// Wooden marimba / droplet body resonance at 720 Hz
			playTone(ctx, master, 720, t0 + 0.015, 0.01, 0.65, 0.42, 'triangle');

			// Secondary delicate ripple drop (+100ms)
			const osc2 = ctx.createOscillator();
			const gain2 = ctx.createGain();
			osc2.type = 'sine';
			osc2.frequency.setValueAtTime(1280, t0 + 0.1);
			osc2.frequency.exponentialRampToValueAtTime(940, t0 + 0.15);

			gain2.gain.setValueAtTime(0.0001, t0 + 0.1);
			gain2.gain.linearRampToValueAtTime(0.38, t0 + 0.11);
			gain2.gain.exponentialRampToValueAtTime(0.0001, t0 + 0.65);

			osc2.connect(gain2);
			gain2.connect(master);
			osc2.start(t0 + 0.1);
			osc2.stop(t0 + 0.7);

			presetDuration = 1.0;
			break;
		}

		case 'calm-piano': {
			// Felt upright piano four-note chord phrase (Fmaj7: F4, A4, C5, E5)
			lowpass.frequency.setValueAtTime(2200, t0); // Soft felt piano low-pass
			const chord = [349.23, 440.0, 523.25, 659.25];
			const stagger = 0.17;

			chord.forEach((freq, idx) => {
				const noteTime = t0 + idx * stagger;
				playTone(ctx, master, freq, noteTime, 0.012, 2.2, 0.5, 'sine', [
					{ mult: 2.0, gain: 0.24, decayMult: 0.75 },
					{ mult: 3.0, gain: 0.11, decayMult: 0.55 },
					{ mult: 4.0, gain: 0.04, decayMult: 0.4 }
				]);
				playTone(ctx, master, freq, noteTime, 0.005, 0.22, 0.22, 'triangle');
			});

			presetDuration = 0.17 * 4 + 2.2;
			break;
		}

		case 'morning-dew': {
			// Ascending 5-note pentatonic melody with blossoming celesta/glockenspiel overtones
			lowpass.frequency.setValueAtTime(3400, t0);
			const notes = [523.25, 587.33, 659.25, 783.99, 880.0];
			const stagger = 0.15;

			notes.forEach((freq, idx) => {
				const noteTime = t0 + idx * stagger;
				const notePeak = 0.42 + (idx / notes.length) * 0.18; // Slight rising crescendo
				playTone(ctx, master, freq, noteTime, 0.016, 1.8, notePeak, 'sine', [
					{ mult: 2.0, gain: 0.22, decayMult: 0.75 },
					{ mult: 3.0, gain: 0.08, decayMult: 0.5 }
				]);
			});

			presetDuration = 0.15 * 5 + 1.8;
			break;
		}

		case 'still-water': {
			// Slower, deeper ambient resonance with rich low-register chords (D3, A3, D4, F#4, A4)
			lowpass.frequency.setValueAtTime(1400, t0); // Deep warm ambient filter
			const notes = [146.83, 220.0, 293.66, 369.99, 440.0];
			const stagger = 0.32;

			notes.forEach((freq, idx) => {
				const noteTime = t0 + idx * stagger;
				playTone(ctx, master, freq, noteTime, 0.06, 3.4, 0.46, 'sine', [
					{ mult: 2.0, gain: 0.25, decayMult: 0.8 },
					{ mult: 3.0, gain: 0.14, decayMult: 0.6 }
				]);
				playTone(ctx, master, freq, noteTime, 0.08, 2.6, 0.22, 'triangle');
			});

			presetDuration = 0.32 * 5 + 3.4;
			break;
		}
	}

	return presetDuration;
}

/**
 * Schedules the triumphant end-of-break flourish: a quick ascending major
 * arpeggio (C5 → E5 → G5 → C6) in bell timbre, resolving high and bright so
 * it is clearly distinguishable from the plain start-of-break phrase.
 */
function scheduleFlourish(ctx: AudioContext, master: GainNode, t0: number) {
	const notes: Array<{ freq: number; peak: number; decay: number }> = [
		{ freq: 523.25, peak: 0.38, decay: 0.8 }, // C5
		{ freq: 659.25, peak: 0.42, decay: 0.9 }, // E5
		{ freq: 783.99, peak: 0.46, decay: 1.0 }, // G5
		{ freq: 1046.5, peak: 0.55, decay: 2.4 } // C6 — triumphant high bell
	];

	notes.forEach((note, i) => {
		const t = t0 + i * 0.09;
		playTone(ctx, master, note.freq, t, 0.012, note.decay, note.peak, 'sine', [
			{ mult: 2.0, gain: 0.25, decayMult: 0.7 },
			{ mult: 3.0, gain: 0.1, decayMult: 0.5 }
		]);
	});

	// Warm undertone grounding the final chord
	playTone(ctx, master, 261.63, t0 + 0.27, 0.02, 1.8, 0.22, 'triangle');
}

/** Builds the shared master chain (gain + warm low-pass + fade envelope). */
function buildMasterChain(
	ctx: AudioContext,
	volume: number,
	fadeIn: boolean,
	preset: SoundPreset
): { master: GainNode; lowpass: BiquadFilterNode } {
	const clampedVol = Math.max(0.05, Math.min(1, volume));

	const master = ctx.createGain();
	currentMasterGain = master;

	const lowpass = ctx.createBiquadFilter();
	lowpass.type = 'lowpass';
	lowpass.frequency.setValueAtTime(3200, ctx.currentTime);
	lowpass.Q.setValueAtTime(0.7, ctx.currentTime);

	master.connect(lowpass);
	lowpass.connect(ctx.destination);

	// Melodies get a luxurious gradual fade-in bloom; short alerts get a
	// smooth de-clicked onset.
	const now = ctx.currentTime;
	const isMelody = preset === 'calm-piano' || preset === 'morning-dew' || preset === 'still-water';
	if (fadeIn) {
		master.gain.setValueAtTime(0.0001, now);
		master.gain.linearRampToValueAtTime(clampedVol, now + (isMelody ? 0.75 : 0.08));
	} else {
		master.gain.setValueAtTime(0.0001, now);
		master.gain.linearRampToValueAtTime(clampedVol, now + 0.015);
	}

	return { master, lowpass };
}

/**
 * Play sound preset with Web Audio API synthesis
 * @param preset One of 6 calm presets
 * @param volume Master volume (0.0 to 1.0)
 * @param fadeIn Whether to fade in gradually instead of sudden onset
 * @param repeatCount Optional number of repetitions (default 1)
 * @param repeatGapSec Gap between repetitions in seconds (default 1.2s)
 */
export async function playSound(
	preset: SoundPreset,
	volume: number = 0.6,
	fadeIn: boolean = true,
	repeatCount: number = 1,
	repeatGapSec: number = 1.2
): Promise<void> {
	if (typeof window === 'undefined') return;

	stopSound();

	const ctx = getAudioContext();
	const { master, lowpass } = buildMasterChain(ctx, volume, fadeIn, preset);
	const presetDuration = schedulePresetPhrase(ctx, master, lowpass, preset, ctx.currentTime);

	// Support repeat behavior if repeatCount > 1
	if (repeatCount > 1) {
		const nextDelayMs = (presetDuration + repeatGapSec) * 1000;
		activePlayTimeout = setTimeout(() => {
			playSound(preset, volume, fadeIn, repeatCount - 1, repeatGapSec);
		}, nextDelayMs);
	}
}

/**
 * End-of-break sound: the same preset phrase the break started with, closing
 * with an ascending triumphant flourish so "break done" is instantly
 * recognizable against the start chime. Plays once (no repeat).
 */
export async function playBreakEnd(
	preset: SoundPreset,
	volume: number = 0.6,
	fadeIn: boolean = true
): Promise<void> {
	if (typeof window === 'undefined') return;

	stopSound();

	const ctx = getAudioContext();
	const { master, lowpass } = buildMasterChain(ctx, volume, fadeIn, preset);

	const now = ctx.currentTime;
	const presetDuration = schedulePresetPhrase(ctx, master, lowpass, preset, now);

	// Flourish lands on the tail of the phrase so it reads as one gesture.
	lowpass.frequency.setValueAtTime(4200, now + presetDuration * 0.6);
	scheduleFlourish(ctx, master, now + presetDuration * 0.8);
}

export function previewSound(
	preset: SoundPreset,
	volume: number = 0.6,
	fadeIn: boolean = true
): void {
	playSound(preset, volume, fadeIn, 1);
}
