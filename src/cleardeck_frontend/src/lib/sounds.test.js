// The audio graph must be built lazily and resumed on the first gesture: a
// context constructed at import time is `suspended` under every browser's
// autoplay policy and every beep scheduled on it is silent (the UI audit's
// critical sound finding). These tests drive SoundManager with a fake window
// and a fake AudioContext and assert the lifecycle, not the tones.
import { describe, it, expect, vi } from 'vitest';
import { SoundManager, RESUME_GESTURES } from './sounds.js';

function fakeParam() {
    return {
        value: 0,
        setValueAtTime: vi.fn(),
        linearRampToValueAtTime: vi.fn(),
        exponentialRampToValueAtTime: vi.fn(),
    };
}

class FakeAudioContext {
    static built = 0;
    constructor() {
        FakeAudioContext.built += 1;
        this.state = 'suspended';
        this.currentTime = 0;
        this.destination = {};
        this.resumed = 0;
        this.started = 0;
    }
    resume() {
        this.resumed += 1;
        this.state = 'running';
        return Promise.resolve();
    }
    createOscillator() {
        const ctx = this;
        return {
            frequency: fakeParam(),
            type: 'sine',
            connect: vi.fn(),
            start: () => { ctx.started += 1; },
            stop: vi.fn(),
        };
    }
    createGain() {
        return { gain: fakeParam(), connect: vi.fn() };
    }
}

function fakeWindow({ supported = true } = {}) {
    const listeners = new Map();
    return {
        AudioContext: supported ? FakeAudioContext : undefined,
        addEventListener: (type, fn) => listeners.set(type, fn),
        removeEventListener: (type) => listeners.delete(type),
        listeners,
        fire(type) {
            const fn = listeners.get(type);
            if (fn) fn({ type });
        },
    };
}

describe('SoundManager: the audio graph and the gesture', () => {
    it('builds no AudioContext at construction; the first play builds it', () => {
        const before = FakeAudioContext.built;
        const win = fakeWindow();
        const sm = new SoundManager({ window: win });
        expect(FakeAudioContext.built).toBe(before);
        expect(sm.state).toBe('idle');
        sm.play('deal');
        expect(FakeAudioContext.built).toBe(before + 1);
        expect(sm.audioContext.started).toBe(1);
    });

    it('arms one listener per gesture type and the first gesture resumes the graph and disarms them', () => {
        const win = fakeWindow();
        const sm = new SoundManager({ window: win });
        for (const type of RESUME_GESTURES) expect(win.listeners.has(type)).toBe(true);
        win.fire('pointerdown');
        expect(sm.audioContext).not.toBeNull();
        expect(sm.audioContext.resumed).toBe(1);
        expect(sm.state).toBe('running');
        for (const type of RESUME_GESTURES) expect(win.listeners.has(type)).toBe(false);
        win.fire('keydown');
        expect(sm.audioContext.resumed).toBe(1);
    });

    it('a play on a suspended context asks it to resume (the click that played it is a gesture)', () => {
        const win = fakeWindow();
        const sm = new SoundManager({ window: win });
        sm.play('call');
        expect(sm.audioContext.resumed).toBe(1);
        expect(sm.state).toBe('running');
        sm.play('fold');
        expect(sm.audioContext.resumed).toBe(1);
    });

    it('reports unsupported and plays nothing, without throwing, where there is no Web Audio', () => {
        const win = fakeWindow({ supported: false });
        const sm = new SoundManager({ window: win });
        expect(sm.state).toBe('unsupported');
        expect(() => sm.play('yourTurn')).not.toThrow();
        expect(sm.audioContext).toBeNull();
    });

    it('tolerates no window at all (a server render or a unit test)', () => {
        const sm = new SoundManager({ window: null });
        expect(sm.state).toBe('unsupported');
        expect(() => sm.play('error')).not.toThrow();
    });

    it('a muted manager builds nothing and touches no listener state', () => {
        const before = FakeAudioContext.built;
        const win = fakeWindow();
        const sm = new SoundManager({ window: win });
        sm.setEnabled(false);
        sm.play('raise');
        expect(FakeAudioContext.built).toBe(before);
        expect(sm.state).toBe('idle');
    });
});
