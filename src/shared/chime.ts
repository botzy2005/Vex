// A short synthesised three-note chime (no audio file to ship). Failure to play is never an error.

let ctx: AudioContext | null = null;

/** Notified on every chime (the browser test stand-in counts them; nothing registers in the shipped app). */
export const chimeListeners = new Set<() => void>();

export function playChime(): void {
  chimeListeners.forEach((f) => f());
  try {
    const AC = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AC) return;
    ctx ??= new AC();
    if (ctx.state === 'suspended') void ctx.resume();
    const now = ctx.currentTime;
    [[0, 784], [0.18, 988], [0.36, 1319]].forEach(([t, freq]) => {
      const osc = ctx!.createOscillator();
      const gain = ctx!.createGain();
      osc.type = 'sine';
      osc.frequency.setValueAtTime(freq, now + t);
      gain.gain.setValueAtTime(0.0001, now + t);
      gain.gain.exponentialRampToValueAtTime(0.18, now + t + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, now + t + 0.5);
      osc.connect(gain).connect(ctx!.destination);
      osc.start(now + t);
      osc.stop(now + t + 0.55);
    });
  } catch {
    /* no sound is fine */
  }
}
