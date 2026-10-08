import type { SoundPreset } from '$lib/types';

let audioCtx: AudioContext | null = null;
let currentMasterGain: GainNode | null = null;

function getAudioContext(): AudioContext {
	if (!audioCtx) {
		const AudioContextClass = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
		audioCtx = new AudioContextClass();
	}
	if (audioCtx.state === 'suspended') {
		audioCtx.resume();
	}
	return audioCtx;
}

export function stopSound(): void {
	if (currentMasterGain && audioCtx) {
		try {
			const now = audioCtx.currentTime;
			currentMasterGain.gain.cancelScheduledValues(now);
			currentMasterGain.gain.setValueAtTime(currentMasterGain.gain.value, now);
			currentMasterGain.gain.linearRampToValueAtTime(0.0001, now + 0.08);
			setTimeout(() => {
				currentMasterGain?.disconnect();
				currentMasterGain = null;
			}, 100);
		} catch {
			currentMasterGain = null;
		}
	}
}

/**
 * Helper to play a single tone with envelope
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
	harmonicOvertones: { mult: number; gain: number }[] = []
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
		hGain.gain.setValueAtTime(0.0001, startTime);
		hGain.gain.linearRampToValueAtTime(hPeak, startTime + attackSec);
		hGain.gain.exponentialRampToValueAtTime(0.0001, startTime + attackSec + decaySec * 0.7);

		hOsc.connect(hGain);
		hGain.connect(destination);

		hOsc.start(startTime);
		hOsc.stop(startTime + attackSec + decaySec + 0.1);
	}
}

export async function playSound(
	preset: SoundPreset,
	volume: number = 0.6,
	fadeIn: boolean = true
): Promise<void> {
	if (typeof window === 'undefined') return;

	stopSound();

	const ctx = getAudioContext();
	const now = ctx.currentTime;
	const clampedVol = Math.max(0.05, Math.min(1, volume));

	const master = ctx.createGain();
	currentMasterGain = master;
	master.connect(ctx.destination);

	// Fade-in envelope
	if (fadeIn) {
		master.gain.setValueAtTime(0.0001, now);
		master.gain.linearRampToValueAtTime(clampedVol, now + 0.65);
	} else {
		master.gain.setValueAtTime(0.0001, now);
		master.gain.linearRampToValueAtTime(clampedVol, now + 0.02);
	}

	switch (preset) {
		case 'soft-bell': {
			// Gentle exponential chime with rich bell resonance
			const freq = 659.25; // E5
			playTone(ctx, master, freq, now, 0.015, 2.2, 0.7, 'sine', [
				{ mult: 2.0, gain: 0.3 },
				{ mult: 2.76, gain: 0.15 },
				{ mult: 4.07, gain: 0.08 }
			]);
			// Low warmth undertone
			playTone(ctx, master, freq / 2, now, 0.02, 1.8, 0.35, 'triangle');
			break;
		}

		case 'gentle-chime': {
			// Two-tone melodic harmonic chime (E5 -> B5)
			const note1 = 659.25; // E5
			const note2 = 987.77; // B5

			playTone(ctx, master, note1, now, 0.012, 1.8, 0.6, 'sine', [
				{ mult: 2.0, gain: 0.2 },
				{ mult: 3.0, gain: 0.08 }
			]);

			playTone(ctx, master, note2, now + 0.18, 0.012, 2.1, 0.65, 'sine', [
				{ mult: 2.0, gain: 0.22 },
				{ mult: 3.0, gain: 0.09 }
			]);
			break;
		}

		case 'calm-drop': {
			// Soft droplet marimba tone with subtle pitch glide & water drop ping
			const osc = ctx.createOscillator();
			const gain = ctx.createGain();

			osc.type = 'sine';
			osc.frequency.setValueAtTime(960, now);
			osc.frequency.exponentialRampToValueAtTime(680, now + 0.06);

			gain.gain.setValueAtTime(0.0001, now);
			gain.gain.linearRampToValueAtTime(0.7, now + 0.01);
			gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.75);

			osc.connect(gain);
			gain.connect(master);
			osc.start(now);
			osc.stop(now + 0.8);

			// Resonant marimba body
			playTone(ctx, master, 680, now + 0.02, 0.01, 0.6, 0.4, 'triangle');

			// Delicate secondary ripple drop
			const osc2 = ctx.createOscillator();
			const gain2 = ctx.createGain();
			osc2.type = 'sine';
			osc2.frequency.setValueAtTime(1180, now + 0.09);
			osc2.frequency.exponentialRampToValueAtTime(890, now + 0.14);

			gain2.gain.setValueAtTime(0.0001, now + 0.09);
			gain2.gain.linearRampToValueAtTime(0.35, now + 0.1);
			gain2.gain.exponentialRampToValueAtTime(0.0001, now + 0.55);

			osc2.connect(gain2);
			gain2.connect(master);
			osc2.start(now + 0.09);
			osc2.stop(now + 0.6);
			break;
		}

		case 'calm-piano': {
			// Peaceful four-note piano chord phrase (Fmaj7: F4, A4, C5, E5)
			const chord = [349.23, 440.0, 523.25, 659.25];
			const stagger = 0.16;

			chord.forEach((freq, idx) => {
				const noteTime = now + idx * stagger;
				// Primary piano harmonic
				playTone(ctx, master, freq, noteTime, 0.008, 1.9, 0.5, 'sine', [
					{ mult: 2.0, gain: 0.25 },
					{ mult: 3.0, gain: 0.12 },
					{ mult: 4.0, gain: 0.05 }
				]);
				// Soft wooden mallet hammer
				playTone(ctx, master, freq, noteTime, 0.005, 0.4, 0.25, 'triangle');
			});
			break;
		}

		case 'morning-dew': {
			// Ascending 5-note pentatonic melody (C5, D5, E5, G5, A5)
			const notes = [523.25, 587.33, 659.25, 783.99, 880.0];
			const stagger = 0.15;

			notes.forEach((freq, idx) => {
				const noteTime = now + idx * stagger;
				playTone(ctx, master, freq, noteTime, 0.015, 1.6, 0.5, 'sine', [
					{ mult: 2.0, gain: 0.2 },
					{ mult: 3.0, gain: 0.08 }
				]);
			});
			break;
		}

		case 'still-water': {
			// Slower, deeper calming ambient resonance (D3, A3, D4, F#4, A4)
			const notes = [146.83, 220.0, 293.66, 369.99, 440.0];
			const stagger = 0.32;

			notes.forEach((freq, idx) => {
				const noteTime = now + idx * stagger;
				playTone(ctx, master, freq, noteTime, 0.06, 3.2, 0.45, 'sine', [
					{ mult: 2.0, gain: 0.25 },
					{ mult: 3.0, gain: 0.15 }
				]);
				playTone(ctx, master, freq, noteTime, 0.08, 2.5, 0.2, 'triangle');
			});
			break;
		}
	}
}

export function previewSound(
	preset: SoundPreset,
	volume: number = 0.6,
	fadeIn: boolean = true
): void {
	playSound(preset, volume, fadeIn);
}
