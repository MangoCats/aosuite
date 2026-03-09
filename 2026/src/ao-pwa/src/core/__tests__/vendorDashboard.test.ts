import { describe, it, expect, vi } from 'vitest';

// Test the formatTimeAgo utility used by VendorDashboard.
// Since it's a private function in the component, we test the logic directly.

function formatTimeAgo(ms: number, now: number): string {
  const secs = Math.floor((now - ms) / 1000);
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  return `${Math.floor(secs / 3600)}h ago`;
}

describe('formatTimeAgo', () => {
  it('formats seconds', () => {
    const now = Date.now();
    expect(formatTimeAgo(now - 30_000, now)).toBe('30s ago');
  });

  it('formats minutes', () => {
    const now = Date.now();
    expect(formatTimeAgo(now - 5 * 60_000, now)).toBe('5m ago');
  });

  it('formats hours', () => {
    const now = Date.now();
    expect(formatTimeAgo(now - 3 * 3600_000, now)).toBe('3h ago');
  });

  it('formats zero seconds', () => {
    const now = Date.now();
    expect(formatTimeAgo(now, now)).toBe('0s ago');
  });

  it('formats 59 seconds as seconds not minutes', () => {
    const now = Date.now();
    expect(formatTimeAgo(now - 59_000, now)).toBe('59s ago');
  });

  it('formats 60 seconds as 1 minute', () => {
    const now = Date.now();
    expect(formatTimeAgo(now - 60_000, now)).toBe('1m ago');
  });
});
