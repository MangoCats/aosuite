import type { ChainHolding } from './api';

/**
 * Convert a share amount to a human-readable coin equivalent.
 * coins = shares × coinCount / totalShares
 */
export function sharesToCoins(shares: string, totalShares: string, coinCount: string): string {
  if (!totalShares || !coinCount || totalShares === '0') return '-';
  try {
    const s = BigInt(shares);
    const ts = BigInt(totalShares);
    const cc = BigInt(coinCount);
    if (ts === 0n) return '-';
    const coins = s * cc / ts;
    return coins.toLocaleString();
  } catch {
    return '-';
  }
}

/** Format a ChainHolding's shares as a coin equivalent string with symbol. */
export function holdingToCoins(h: ChainHolding): string {
  const coins = sharesToCoins(h.shares, h.total_shares, h.coin_count);
  return coins === '-' ? '-' : `${coins} ${h.symbol}`;
}
