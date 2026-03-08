/** Payment notification chime — Web Audio API tone generator.
 *  No audio files needed; generates short tones programmatically. */

export type ChimeStyle = 'bell' | 'cash' | 'ding' | 'none';

const CHIME_STYLES: ChimeStyle[] = ['bell', 'cash', 'ding', 'none'];

export function isChimeStyle(s: string): s is ChimeStyle {
  return (CHIME_STYLES as string[]).includes(s);
}

let audioCtx: AudioContext | null = null;

function getAudioContext(): AudioContext | null {
  if (typeof AudioContext === 'undefined') return null;
  if (!audioCtx || audioCtx.state === 'closed') {
    audioCtx = new AudioContext();
  }
  return audioCtx;
}

/** Play a chime at the given volume (0–1). */
export async function playChime(style: ChimeStyle, volume: number): Promise<void> {
  if (style === 'none' || volume <= 0) return;
  const ctx = getAudioContext();
  if (!ctx) return;

  // Resume context if suspended (browsers require user gesture first)
  if (ctx.state === 'suspended') {
    try { await ctx.resume(); } catch { return; }
  }

  const gain = ctx.createGain();
  gain.connect(ctx.destination);
  gain.gain.setValueAtTime(volume * 0.3, ctx.currentTime);

  switch (style) {
    case 'bell':
      playBell(ctx, gain);
      break;
    case 'cash':
      playCash(ctx, gain);
      break;
    case 'ding':
      playDing(ctx, gain);
      break;
  }
}

/** Bell: two-tone ascending chime (C5 → E5). */
function playBell(ctx: AudioContext, gain: GainNode): void {
  const t = ctx.currentTime;
  // First tone
  const osc1 = ctx.createOscillator();
  osc1.type = 'sine';
  osc1.frequency.setValueAtTime(523, t); // C5
  osc1.connect(gain);
  osc1.start(t);
  osc1.stop(t + 0.15);
  // Second tone
  const osc2 = ctx.createOscillator();
  osc2.type = 'sine';
  osc2.frequency.setValueAtTime(659, t + 0.12); // E5
  osc2.connect(gain);
  osc2.start(t + 0.12);
  osc2.stop(t + 0.3);
  // Fade out
  gain.gain.exponentialRampToValueAtTime(0.001, t + 0.35);
}

/** Cash register: quick descending two-tone (G5 → C5). */
function playCash(ctx: AudioContext, gain: GainNode): void {
  const t = ctx.currentTime;
  const osc1 = ctx.createOscillator();
  osc1.type = 'triangle';
  osc1.frequency.setValueAtTime(784, t); // G5
  osc1.connect(gain);
  osc1.start(t);
  osc1.stop(t + 0.1);

  const osc2 = ctx.createOscillator();
  osc2.type = 'triangle';
  osc2.frequency.setValueAtTime(523, t + 0.08); // C5
  osc2.connect(gain);
  osc2.start(t + 0.08);
  osc2.stop(t + 0.25);

  gain.gain.exponentialRampToValueAtTime(0.001, t + 0.3);
}

/** Ding: single clean tone (A5). */
function playDing(ctx: AudioContext, gain: GainNode): void {
  const t = ctx.currentTime;
  const osc = ctx.createOscillator();
  osc.type = 'sine';
  osc.frequency.setValueAtTime(880, t); // A5
  osc.connect(gain);
  osc.start(t);
  osc.stop(t + 0.2);
  gain.gain.exponentialRampToValueAtTime(0.001, t + 0.25);
}

/** Check whether chime should be suppressed due to quiet hours.
 *  muteStart/muteEnd are hours (0–23). Handles overnight ranges (e.g. 22–8). */
export function isInQuietHours(muteStart: number, muteEnd: number): boolean {
  if (muteStart === muteEnd) return false; // no quiet window
  const hour = new Date().getHours();
  if (muteStart < muteEnd) {
    // Same-day range: e.g. 9–17
    return hour >= muteStart && hour < muteEnd;
  }
  // Overnight range: e.g. 22–8
  return hour >= muteStart || hour < muteEnd;
}

/** Check whether a quick-mute is currently active.
 *  Returns true if muteUntil is in the future. */
export function isQuickMuted(muteUntil: number | null): boolean {
  if (muteUntil === null) return false;
  return Date.now() < muteUntil;
}
