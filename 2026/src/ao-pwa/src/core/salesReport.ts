// Sales reporting — aggregates TxRecords into daily/weekly/monthly summaries.

import type { TxRecord } from './txHistory.ts';

export type Period = 'daily' | 'weekly' | 'monthly';

export interface SalesSummary {
  /** Period label (e.g. "2026-03-08", "2026-W10", "2026-03"). */
  label: string;
  /** Period start as Unix ms. */
  startMs: number;
  /** Total shares received in period. */
  totalShares: bigint;
  /** Number of incoming transactions. */
  txCount: number;
}

/** Group "received" transactions by period and aggregate.
 *  Only counts direction === 'received' (incoming payments). */
export function aggregateSales(records: TxRecord[], period: Period): SalesSummary[] {
  const received = records.filter(r => r.direction === 'received');
  if (received.length === 0) return [];

  const buckets = new Map<string, SalesSummary>();

  for (const r of received) {
    const key = periodKey(r.timestampMs, period);
    let bucket = buckets.get(key.label);
    if (!bucket) {
      bucket = { label: key.label, startMs: key.startMs, totalShares: 0n, txCount: 0 };
      buckets.set(key.label, bucket);
    }
    bucket.totalShares += BigInt(r.amount);
    bucket.txCount += 1;
  }

  // Sort chronologically (oldest first)
  return Array.from(buckets.values()).sort((a, b) => a.startMs - b.startMs);
}

/** Filter records to a date range [startMs, endMs). */
export function filterByDateRange(records: TxRecord[], startMs: number, endMs: number): TxRecord[] {
  return records.filter(r => r.timestampMs >= startMs && r.timestampMs < endMs);
}

/** Format shares as coin amount string (6 decimal places). */
export function sharesToCoins(shares: bigint, coinCount: bigint): string {
  if (coinCount <= 0n) return '';
  const negative = shares < 0n;
  const absShares = negative ? -shares : shares;
  const scale = 1000000n;
  const scaled = absShares * scale / coinCount;
  const intPart = scaled / scale;
  const fracPart = scaled % scale;
  return `${negative ? '-' : ''}${intPart}.${fracPart.toString().padStart(6, '0')}`;
}

/** Generate CSV for sales report. */
export function salesReportCsv(
  summaries: SalesSummary[],
  chainSymbol: string,
  coinCount: string,
  period: Period,
): string {
  const coinTotal = BigInt(coinCount || '0');
  const header = `period,${period}_total_shares,${period}_total_coins,transaction_count,avg_coins_per_tx`;
  const lines = summaries.map(s => {
    const coins = coinTotal > 0n ? sharesToCoins(s.totalShares, coinTotal) : '';
    const avgCoins = coinTotal > 0n && s.txCount > 0
      ? sharesToCoins(s.totalShares / BigInt(s.txCount), coinTotal) : '';
    return `${s.label},${s.totalShares},${coins},${s.txCount},${avgCoins}`;
  });
  return [header, ...lines].join('\n');
}

// ── Period Key Helpers ──────────────────────────────────────────────

interface PeriodKey {
  label: string;
  startMs: number;
}

function periodKey(timestampMs: number, period: Period): PeriodKey {
  const d = new Date(timestampMs);
  switch (period) {
    case 'daily':
      return dailyKey(d);
    case 'weekly':
      return weeklyKey(d);
    case 'monthly':
      return monthlyKey(d);
  }
}

function dailyKey(d: Date): PeriodKey {
  const label = d.toISOString().split('T')[0]; // YYYY-MM-DD (UTC)
  const start = Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate());
  return { label, startMs: start };
}

function weeklyKey(d: Date): PeriodKey {
  // ISO week: Monday-based, UTC.
  const day = d.getUTCDay(); // 0=Sun, 1=Mon, ...
  const mondayOffset = day === 0 ? -6 : 1 - day;
  const mondayMs = Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate() + mondayOffset);
  const monday = new Date(mondayMs);
  const isoYear = monday.getUTCFullYear();
  // ISO week number: days since Jan 4's Monday / 7 + 1
  const jan4 = new Date(Date.UTC(isoYear, 0, 4));
  const jan4Day = jan4.getUTCDay();
  const jan4MonOffset = jan4Day === 0 ? -6 : 1 - jan4Day;
  const isoWeek1MonMs = Date.UTC(isoYear, 0, 4 + jan4MonOffset);
  const weekNum = Math.floor((mondayMs - isoWeek1MonMs) / (7 * 86400000)) + 1;
  const label = `${isoYear}-W${String(weekNum).padStart(2, '0')}`;
  return { label, startMs: mondayMs };
}

function monthlyKey(d: Date): PeriodKey {
  const y = d.getUTCFullYear();
  const m = d.getUTCMonth();
  const label = `${y}-${String(m + 1).padStart(2, '0')}`;
  return { label, startMs: Date.UTC(y, m, 1) };
}
