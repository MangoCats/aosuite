import { describe, it, expect, vi, afterEach } from 'vitest';
import { isInQuietHours, isQuickMuted, isChimeStyle } from '../chime.ts';

describe('isChimeStyle', () => {
  it('accepts valid chime styles', () => {
    expect(isChimeStyle('bell')).toBe(true);
    expect(isChimeStyle('cash')).toBe(true);
    expect(isChimeStyle('ding')).toBe(true);
    expect(isChimeStyle('none')).toBe(true);
  });

  it('rejects invalid strings', () => {
    expect(isChimeStyle('beep')).toBe(false);
    expect(isChimeStyle('')).toBe(false);
    expect(isChimeStyle('BELL')).toBe(false);
  });
});

describe('isInQuietHours', () => {
  afterEach(() => { vi.restoreAllMocks(); });

  it('returns false when start equals end (no quiet window)', () => {
    expect(isInQuietHours(22, 22)).toBe(false);
    expect(isInQuietHours(0, 0)).toBe(false);
  });

  it('same-day range: inside quiet hours', () => {
    vi.spyOn(Date.prototype, 'getHours').mockReturnValue(12);
    expect(isInQuietHours(9, 17)).toBe(true);
  });

  it('same-day range: outside quiet hours', () => {
    vi.spyOn(Date.prototype, 'getHours').mockReturnValue(8);
    expect(isInQuietHours(9, 17)).toBe(false);
  });

  it('same-day range: at end boundary (exclusive)', () => {
    vi.spyOn(Date.prototype, 'getHours').mockReturnValue(17);
    expect(isInQuietHours(9, 17)).toBe(false);
  });

  it('overnight range: inside late night', () => {
    vi.spyOn(Date.prototype, 'getHours').mockReturnValue(23);
    expect(isInQuietHours(22, 8)).toBe(true);
  });

  it('overnight range: inside early morning', () => {
    vi.spyOn(Date.prototype, 'getHours').mockReturnValue(5);
    expect(isInQuietHours(22, 8)).toBe(true);
  });

  it('overnight range: outside (daytime)', () => {
    vi.spyOn(Date.prototype, 'getHours').mockReturnValue(14);
    expect(isInQuietHours(22, 8)).toBe(false);
  });

  it('overnight range: at end boundary (exclusive)', () => {
    vi.spyOn(Date.prototype, 'getHours').mockReturnValue(8);
    expect(isInQuietHours(22, 8)).toBe(false);
  });
});

describe('isQuickMuted', () => {
  it('returns false when null', () => {
    expect(isQuickMuted(null)).toBe(false);
  });

  it('returns true when mute is in the future', () => {
    expect(isQuickMuted(Date.now() + 60_000)).toBe(true);
  });

  it('returns false when mute has expired', () => {
    expect(isQuickMuted(Date.now() - 1000)).toBe(false);
  });
});
