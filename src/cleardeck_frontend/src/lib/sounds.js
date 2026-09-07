// Sound effects system for poker game
// Uses Web Audio API for programmatic sounds and can load audio files
//
// THE AUDIO GRAPH IS CREATED ON THE FIRST PLAY AND RESUMED ON THE FIRST GESTURE.
// Every shipping browser's autoplay policy hands out an AudioContext in the
// `suspended` state when it is constructed before a user gesture, and an
// oscillator scheduled on a suspended clock is never heard. The old singleton
// built its context at module load (import time), so on mainnet every beep,
// the your-turn chime included, was silent while the header's speaker said
// sound was on (the UI audit's critical finding on sound). Now: the context is
// built lazily, `resume()` is called on the first pointerdown / keydown /
// touchstart the document sees (a one-time listener, removed once it has
// fired), and again from `play()` whenever the context is still suspended (a
// play triggered by the player's own click IS a gesture, so that resume lands).
// `soundManager.state` says which of unsupported / suspended / running the
// graph is in, for any surface that wants to say "tap to enable sound".

import logger from './logger.js';

/** The events a browser accepts as a user gesture for audio. */
export const RESUME_GESTURES = Object.freeze(['pointerdown', 'keydown', 'touchstart']);

/** @returns {{ AudioContext?: Function, addEventListener?: Function, removeEventListener?: Function } | null} */
function defaultWindow() {
    return typeof window !== 'undefined' ? window : null;
}

export class SoundManager {
    /**
     * @param {{ window?: object | null }} [env] the window to read the AudioContext
     *   constructor and the gesture listeners from (tests pass a fake).
     */
    constructor(env = {}) {
        this.enabled = true;
        this.volume = 0.5;
        this.audioContext = null;
        this.sounds = new Map();
        this.win = env.window === undefined ? defaultWindow() : env.window;
        this.gestureListener = null;
        this.armResumeOnGesture();
    }

    /** The AudioContext constructor this browser offers, or null. */
    contextConstructor() {
        const w = this.win;
        if (!w) return null;
        return w.AudioContext || w.webkitAudioContext || null;
    }

    /** 'unsupported' | 'suspended' | 'running' | 'closed' | 'idle' (not built yet). */
    get state() {
        if (!this.contextConstructor()) return 'unsupported';
        if (!this.audioContext) return 'idle';
        return this.audioContext.state || 'running';
    }

    /** Build the context on first use; never at import time. Returns null when unsupported. */
    ensureContext() {
        if (this.audioContext) return this.audioContext;
        const Ctor = this.contextConstructor();
        if (!Ctor) return null;
        try {
            this.audioContext = new Ctor();
        } catch (e) {
            logger.warn('Web Audio API not supported:', e);
            this.audioContext = null;
        }
        return this.audioContext;
    }

    /** Ask a suspended context to run. Safe to call at any time; never throws. */
    resume() {
        const ctx = this.ensureContext();
        if (!ctx || ctx.state !== 'suspended' || typeof ctx.resume !== 'function') return;
        try {
            const p = ctx.resume();
            if (p && typeof p.catch === 'function') {
                p.catch((e) => logger.warn('AudioContext resume refused:', e));
            }
        } catch (e) {
            logger.warn('AudioContext resume threw:', e);
        }
    }

    /** One-time document listeners: the first gesture resumes the graph and removes them. */
    armResumeOnGesture() {
        const w = this.win;
        if (!w || typeof w.addEventListener !== 'function' || this.gestureListener) return;
        const listener = () => {
            this.disarmResumeOnGesture();
            this.resume();
        };
        this.gestureListener = listener;
        for (const type of RESUME_GESTURES) {
            w.addEventListener(type, listener, { passive: true });
        }
    }

    disarmResumeOnGesture() {
        const w = this.win;
        const listener = this.gestureListener;
        if (!w || !listener) return;
        this.gestureListener = null;
        if (typeof w.removeEventListener !== 'function') return;
        for (const type of RESUME_GESTURES) {
            w.removeEventListener(type, listener);
        }
    }

    setEnabled(enabled) {
        this.enabled = enabled;
    }

    setVolume(volume) {
        this.volume = Math.max(0, Math.min(1, volume));
    }

    // Generate a simple beep sound using Web Audio API
    generateBeep(frequency = 440, duration = 100, type = 'sine') {
        if (!this.enabled) return;
        const ctx = this.ensureContext();
        if (!ctx) return;
        if (ctx.state === 'suspended') this.resume();

        const oscillator = ctx.createOscillator();
        const gainNode = ctx.createGain();

        oscillator.connect(gainNode);
        gainNode.connect(ctx.destination);

        oscillator.frequency.value = frequency;
        oscillator.type = type;

        gainNode.gain.setValueAtTime(0, ctx.currentTime);
        gainNode.gain.linearRampToValueAtTime(this.volume * 0.3, ctx.currentTime + 0.01);
        gainNode.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + duration / 1000);

        oscillator.start(ctx.currentTime);
        oscillator.stop(ctx.currentTime + duration / 1000);
    }

    // Play a sound effect
    play(soundName, options = {}) {
        if (!this.enabled) return;

        const {
            frequency = 440,
            duration = 100,
            type = 'sine',
            volume = this.volume
        } = options;

        // Try to load from preloaded sounds first
        if (this.sounds.has(soundName)) {
            const audio = this.sounds.get(soundName).cloneNode();
            audio.volume = volume;
                audio.play().catch(e => {
                    logger.warn(`Failed to play sound ${soundName}:`, e);
                    // Fallback to generated sound
                    this.generateBeep(frequency, duration, type);
                });
            return;
        }

        // Fallback to generated sounds
        switch (soundName) {
            case 'deal':
                this.generateBeep(300, 50, 'sine');
                break;
            case 'fold':
                this.generateBeep(200, 80, 'sawtooth');
                break;
            case 'check':
                this.generateBeep(400, 60, 'sine');
                break;
            case 'call':
                this.generateBeep(500, 70, 'sine');
                break;
            case 'bet':
                this.generateBeep(600, 100, 'sine');
                break;
            case 'raise':
                this.generateBeep(700, 120, 'sine');
                break;
            case 'allin':
                // Two-tone sound for all-in
                this.generateBeep(600, 80, 'sine');
                setTimeout(() => this.generateBeep(800, 100, 'sine'), 80);
                break;
            case 'win':
                // Ascending tones for win
                [400, 500, 600, 700].forEach((freq, i) => {
                    setTimeout(() => this.generateBeep(freq, 100, 'sine'), i * 80);
                });
                break;
            case 'chips':
                this.generateBeep(350, 40, 'square');
                break;
            case 'timer':
                this.generateBeep(800, 30, 'sine');
                break;
            case 'yourTurn':
                // Two rising notes: distinct from every action beep, short
                // enough to sit under a tabbed-out player's other audio.
                this.generateBeep(660, 90, 'sine');
                setTimeout(() => this.generateBeep(880, 140, 'sine'), 110);
                break;
            case 'error':
                this.generateBeep(200, 200, 'sawtooth');
                break;
            default:
                this.generateBeep(frequency, duration, type);
        }
    }

    // Preload audio files (call this with actual audio file paths when available)
    async loadSound(name, url) {
        try {
            const audio = new Audio(url);
            audio.preload = 'auto';
            await audio.load();
            this.sounds.set(name, audio);
            } catch (e) {
                logger.warn(`Failed to load sound ${name} from ${url}:`, e);
            }
    }

    // Preload multiple sounds
    async loadSounds(soundMap) {
        const promises = Object.entries(soundMap).map(([name, url]) =>
            this.loadSound(name, url)
        );
        await Promise.all(promises);
    }
}

// Create singleton instance
export const soundManager = new SoundManager();

// Export convenience functions
export const playSound = (name, options) => soundManager.play(name, options);
export const setSoundEnabled = (enabled) => soundManager.setEnabled(enabled);
export const setSoundVolume = (volume) => soundManager.setVolume(volume);
export const isSoundEnabled = () => soundManager.enabled;
export const soundState = () => soundManager.state;
