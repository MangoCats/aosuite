// Animated QR code for large payloads — splits data into numbered frames
// and cycles through them. Spec: specs/WalletSync.md §3.2
//
// Frame format: JSON {"f": frameIndex, "n": totalFrames, "d": chunkData}
// Receiver reassembles by collecting all frames.

import { useState, useEffect, useCallback } from 'react';
import { QrCode } from './QrCode.tsx';

interface AnimatedQrCodeProps {
  value: string;
  size?: number;
  /** Max bytes per QR frame (default 1800 — safe for QR medium EC). */
  chunkSize?: number;
  /** Milliseconds per frame (default 600). */
  frameInterval?: number;
}

/** Threshold in bytes below which a single static QR is used. */
const STATIC_THRESHOLD = 1800;

function splitIntoFrames(data: string, chunkSize: number): string[] {
  const chunks: string[] = [];
  for (let i = 0; i < data.length; i += chunkSize) {
    chunks.push(data.slice(i, i + chunkSize));
  }
  // Wrap each chunk in a frame envelope
  return chunks.map((d, i) =>
    JSON.stringify({ f: i, n: chunks.length, d })
  );
}

export function AnimatedQrCode({
  value,
  size = 280,
  chunkSize = STATIC_THRESHOLD,
  frameInterval = 600,
}: AnimatedQrCodeProps) {
  const [frameIndex, setFrameIndex] = useState(0);

  // If payload fits in a single QR, render statically
  if (value.length <= chunkSize) {
    return <QrCode value={value} size={size} />;
  }

  const frames = splitIntoFrames(value, chunkSize);

  // Cycle through frames
  useEffect(() => {
    const timer = setInterval(() => {
      setFrameIndex(i => (i + 1) % frames.length);
    }, frameInterval);
    return () => clearInterval(timer);
  }, [frames.length, frameInterval]);

  return (
    <div>
      <QrCode value={frames[frameIndex]} size={size} />
      <div style={{ textAlign: 'center', fontSize: 11, color: '#666', marginTop: 4 }}>
        Frame {frameIndex + 1} / {frames.length}
      </div>
    </div>
  );
}

// ── Reassembly helper for AnimatedQrScanner ──────────────────────────

export interface FrameCollector {
  frames: Map<number, string>;
  total: number | null;
}

/** Maximum number of animated QR frames we'll accept (prevents memory abuse). */
const MAX_FRAMES = 200;

/** Try to parse a scanned QR as an animated frame. Returns null if not a frame. */
export function parseFrame(data: string): { f: number; n: number; d: string } | null {
  try {
    const obj = JSON.parse(data);
    if (
      typeof obj.f === 'number' && Number.isInteger(obj.f) && obj.f >= 0 &&
      typeof obj.n === 'number' && Number.isInteger(obj.n) && obj.n > 0 && obj.n <= MAX_FRAMES &&
      obj.f < obj.n &&
      typeof obj.d === 'string'
    ) {
      return obj;
    }
  } catch { /* not a frame */ }
  return null;
}

/** Add a frame to the collector. Returns the reassembled payload when complete, null otherwise.
 *  Returns null and resets the collector if frames disagree on total count. */
export function collectFrame(collector: FrameCollector, data: string): string | null {
  const frame = parseFrame(data);
  if (!frame) return null;

  // If we already have a total, reject frames that disagree
  if (collector.total !== null && collector.total !== frame.n) {
    // Mismatch — reset and start fresh with this frame
    collector.frames.clear();
    collector.total = frame.n;
    collector.frames.set(frame.f, frame.d);
    return null;
  }

  collector.total = frame.n;
  collector.frames.set(frame.f, frame.d);

  if (collector.frames.size === frame.n) {
    // All frames collected — reassemble in order
    const parts: string[] = [];
    for (let i = 0; i < frame.n; i++) {
      parts.push(collector.frames.get(i)!);
    }
    return parts.join('');
  }
  return null;
}

/** Create a fresh frame collector. */
export function createCollector(): FrameCollector {
  return { frames: new Map(), total: null };
}
