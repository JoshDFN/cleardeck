// Sound effects system for poker game
// Uses Web Audio API for programmatic sounds and can load audio files

import logger from './logger.js';

class SoundManager {
    constructor() {
        this.enabled = true;
        this.volume = 0.5;
        this.audioContext = null;
        this.sounds = new Map();
        this.initAudioContext();
    }

    initAudioContext() {
        try {
            this.audioContext = new (window.AudioContext || window.webkitAudioContext)();
            } catch (e) {
                logger.warn('Web Audio API not supported:', e);
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
        if (!this.audioContext || !this.enabled) return;

        const oscillator = this.audioContext.createOscillator();
        const gainNode = this.audioContext.createGain();

        oscillator.connect(gainNode);
        gainNode.connect(this.audioContext.destination);

        oscillator.frequency.value = frequency;
        oscillator.type = type;

        gainNode.gain.setValueAtTime(0, this.audioContext.currentTime);
        gainNode.gain.linearRampToValueAtTime(this.volume * 0.3, this.audioContext.currentTime + 0.01);
        gainNode.gain.exponentialRampToValueAtTime(0.01, this.audioContext.currentTime + duration / 1000);

        oscillator.start(this.audioContext.currentTime);
        oscillator.stop(this.audioContext.currentTime + duration / 1000);
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
