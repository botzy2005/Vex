// A short synthesised shutter click (no audio file to ship). Failure to play is never an error.

let ctx: AudioContext | null = null;

export function playShutter(): void {
  try {
    const AC = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AC) return;
    ctx ??= new AC();
    if (ctx.state === 'suspended') void ctx.resume();
    const now = ctx.currentTime;
    // Two quick clicks: a noisy tick and a lower thunk.
    for (const [t, freq, dur] of [[0, 1800, 0.035], [0.045, 600, 0.06]] as const) {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = 'square';
      osc.frequency.setValueAtTime(freq, now + t);
      gain.gain.setValueAtTime(0.0001, now + t);
      gain.gain.exponentialRampToValueAtTime(0.12, now + t + 0.004);
      gain.gain.exponentialRampToValueAtTime(0.0001, now + t + dur);
      osc.connect(gain).connect(ctx.destination);
      osc.start(now + t);
      osc.stop(now + t + dur + 0.02);
    }
  } catch {
    /* no sound is fine */
  }
}
