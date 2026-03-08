// Tests for animated QR frame splitting, parsing, and reassembly.

import { describe, it, expect } from 'vitest';
import { parseFrame, collectFrame, createCollector } from '../../components/AnimatedQrCode.tsx';

describe('animated QR frames', () => {
  it('parseFrame returns null for non-frame data', () => {
    expect(parseFrame('hello')).toBeNull();
    expect(parseFrame('{"v":1,"type":"key_sync"}')).toBeNull();
    expect(parseFrame('')).toBeNull();
  });

  it('parseFrame parses valid frame', () => {
    const frame = parseFrame(JSON.stringify({ f: 0, n: 3, d: 'chunk0' }));
    expect(frame).toEqual({ f: 0, n: 3, d: 'chunk0' });
  });

  it('collectFrame returns null until all frames collected', () => {
    const collector = createCollector();
    const f0 = JSON.stringify({ f: 0, n: 3, d: 'aaa' });
    const f1 = JSON.stringify({ f: 1, n: 3, d: 'bbb' });
    const f2 = JSON.stringify({ f: 2, n: 3, d: 'ccc' });

    expect(collectFrame(collector, f0)).toBeNull();
    expect(collector.frames.size).toBe(1);

    expect(collectFrame(collector, f1)).toBeNull();
    expect(collector.frames.size).toBe(2);

    const result = collectFrame(collector, f2);
    expect(result).toBe('aaabbbccc');
  });

  it('handles out-of-order frames', () => {
    const collector = createCollector();
    const f2 = JSON.stringify({ f: 2, n: 3, d: 'cc' });
    const f0 = JSON.stringify({ f: 0, n: 3, d: 'aa' });
    const f1 = JSON.stringify({ f: 1, n: 3, d: 'bb' });

    expect(collectFrame(collector, f2)).toBeNull();
    expect(collectFrame(collector, f0)).toBeNull();
    const result = collectFrame(collector, f1);
    expect(result).toBe('aabbcc');
  });

  it('handles duplicate frames', () => {
    const collector = createCollector();
    const f0 = JSON.stringify({ f: 0, n: 2, d: 'xx' });
    const f1 = JSON.stringify({ f: 1, n: 2, d: 'yy' });

    collectFrame(collector, f0);
    collectFrame(collector, f0); // duplicate
    expect(collector.frames.size).toBe(1);

    const result = collectFrame(collector, f1);
    expect(result).toBe('xxyy');
  });

  it('returns null for non-frame strings', () => {
    const collector = createCollector();
    expect(collectFrame(collector, 'not a frame')).toBeNull();
    expect(collector.frames.size).toBe(0);
  });

  it('handles single-frame payload', () => {
    const collector = createCollector();
    const f0 = JSON.stringify({ f: 0, n: 1, d: 'entire payload' });
    const result = collectFrame(collector, f0);
    expect(result).toBe('entire payload');
  });

  it('rejects frames with f >= n', () => {
    expect(parseFrame(JSON.stringify({ f: 3, n: 3, d: 'x' }))).toBeNull();
    expect(parseFrame(JSON.stringify({ f: 5, n: 2, d: 'x' }))).toBeNull();
  });

  it('rejects frames with non-integer f or n', () => {
    expect(parseFrame(JSON.stringify({ f: 0.5, n: 3, d: 'x' }))).toBeNull();
    expect(parseFrame(JSON.stringify({ f: 0, n: 2.5, d: 'x' }))).toBeNull();
  });

  it('rejects frames with n <= 0 or negative f', () => {
    expect(parseFrame(JSON.stringify({ f: 0, n: 0, d: 'x' }))).toBeNull();
    expect(parseFrame(JSON.stringify({ f: -1, n: 3, d: 'x' }))).toBeNull();
    expect(parseFrame(JSON.stringify({ f: 0, n: -1, d: 'x' }))).toBeNull();
  });

  it('rejects frames with n exceeding MAX_FRAMES (200)', () => {
    expect(parseFrame(JSON.stringify({ f: 0, n: 201, d: 'x' }))).toBeNull();
    // 200 is ok
    expect(parseFrame(JSON.stringify({ f: 0, n: 200, d: 'x' }))).not.toBeNull();
  });

  it('resets collector when frames disagree on total', () => {
    const collector = createCollector();
    collectFrame(collector, JSON.stringify({ f: 0, n: 3, d: 'aa' }));
    expect(collector.frames.size).toBe(1);

    // Frame claims n=2 instead of n=3 — should reset
    collectFrame(collector, JSON.stringify({ f: 0, n: 2, d: 'bb' }));
    expect(collector.total).toBe(2);
    expect(collector.frames.size).toBe(1);

    // Complete the n=2 set
    const result = collectFrame(collector, JSON.stringify({ f: 1, n: 2, d: 'cc' }));
    expect(result).toBe('bbcc');
  });
});
