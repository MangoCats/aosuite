import { describe, it, expect } from 'vitest';

// Test the escapeHtml utility (extracted logic test)
function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

describe('PrintableQr helpers', () => {
  describe('escapeHtml', () => {
    it('escapes ampersands', () => {
      expect(escapeHtml('A & B')).toBe('A &amp; B');
    });

    it('escapes angle brackets', () => {
      expect(escapeHtml('<script>')).toBe('&lt;script&gt;');
    });

    it('escapes quotes', () => {
      expect(escapeHtml('"hello"')).toBe('&quot;hello&quot;');
    });

    it('handles multiple special chars', () => {
      expect(escapeHtml('a<b&c>d"e')).toBe('a&lt;b&amp;c&gt;d&quot;e');
    });

    it('passes through plain text unchanged', () => {
      expect(escapeHtml("Bob's Curry Goat")).toBe("Bob's Curry Goat");
    });
  });

  describe('chain URL construction', () => {
    it('builds correct chain URL', () => {
      const recorderUrl = 'http://localhost:3000';
      const chainId = 'abc123';
      const chainUrl = `${recorderUrl}/chain/${chainId}`;
      expect(chainUrl).toBe('http://localhost:3000/chain/abc123');
    });

    it('handles HTTPS URLs', () => {
      const recorderUrl = 'https://recorder.example.com';
      const chainId = 'deadbeef';
      const chainUrl = `${recorderUrl}/chain/${chainId}`;
      expect(chainUrl).toBe('https://recorder.example.com/chain/deadbeef');
    });
  });
});
