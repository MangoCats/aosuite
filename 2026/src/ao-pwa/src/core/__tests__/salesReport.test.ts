import { describe, it, expect } from 'vitest';
import {
  aggregateSales, filterByDateRange, sharesToCoins, salesReportCsv,
  type Period,
} from '../salesReport.ts';
import type { TxRecord } from '../txHistory.ts';

function makeTx(overrides: Partial<TxRecord> = {}): TxRecord {
  return {
    blockHeight: 1,
    pageIndex: 0,
    timestampMs: Date.now(),
    direction: 'received',
    amount: '1000',
    counterparty: 'aa'.repeat(32),
    hasBlob: false,
    giverSeqIds: [0],
    receiverSeqId: 1,
    ...overrides,
  };
}

describe('aggregateSales', () => {
  it('returns empty for no records', () => {
    expect(aggregateSales([], 'daily')).toEqual([]);
  });

  it('ignores sent transactions', () => {
    const records = [makeTx({ direction: 'sent' })];
    expect(aggregateSales(records, 'daily')).toEqual([]);
  });

  it('aggregates daily totals', () => {
    const day1 = new Date('2026-03-01T10:00:00Z').getTime();
    const day1b = new Date('2026-03-01T15:00:00Z').getTime();
    const day2 = new Date('2026-03-02T12:00:00Z').getTime();

    const records = [
      makeTx({ timestampMs: day1, amount: '500' }),
      makeTx({ timestampMs: day1b, amount: '300' }),
      makeTx({ timestampMs: day2, amount: '200' }),
    ];

    const result = aggregateSales(records, 'daily');
    expect(result.length).toBe(2);
    expect(result[0].label).toBe('2026-03-01');
    expect(result[0].totalShares).toBe(800n);
    expect(result[0].txCount).toBe(2);
    expect(result[1].label).toBe('2026-03-02');
    expect(result[1].totalShares).toBe(200n);
    expect(result[1].txCount).toBe(1);
  });

  it('aggregates monthly totals', () => {
    const jan = new Date('2026-01-15T12:00:00Z').getTime();
    const feb = new Date('2026-02-20T12:00:00Z').getTime();

    const records = [
      makeTx({ timestampMs: jan, amount: '1000' }),
      makeTx({ timestampMs: feb, amount: '2000' }),
    ];

    const result = aggregateSales(records, 'monthly');
    expect(result.length).toBe(2);
    expect(result[0].label).toBe('2026-01');
    expect(result[1].label).toBe('2026-02');
  });

  it('aggregates weekly totals', () => {
    // 2026-03-02 is a Monday
    const mon = new Date('2026-03-02T12:00:00Z').getTime();
    const wed = new Date('2026-03-04T12:00:00Z').getTime();
    const nextMon = new Date('2026-03-09T12:00:00Z').getTime();

    const records = [
      makeTx({ timestampMs: mon, amount: '100' }),
      makeTx({ timestampMs: wed, amount: '200' }),
      makeTx({ timestampMs: nextMon, amount: '300' }),
    ];

    const result = aggregateSales(records, 'weekly');
    expect(result.length).toBe(2);
    expect(result[0].txCount).toBe(2);
    expect(result[0].totalShares).toBe(300n);
    expect(result[1].txCount).toBe(1);
    expect(result[1].totalShares).toBe(300n);
  });

  it('sorts chronologically', () => {
    const later = new Date('2026-06-15T12:00:00Z').getTime();
    const earlier = new Date('2026-01-10T12:00:00Z').getTime();

    const records = [
      makeTx({ timestampMs: later, amount: '100' }),
      makeTx({ timestampMs: earlier, amount: '200' }),
    ];

    const result = aggregateSales(records, 'monthly');
    expect(result[0].label).toBe('2026-01');
    expect(result[1].label).toBe('2026-06');
  });
});

describe('filterByDateRange', () => {
  it('filters inclusive start, exclusive end', () => {
    const t1 = new Date('2026-03-01').getTime();
    const t2 = new Date('2026-03-02').getTime();
    const t3 = new Date('2026-03-03').getTime();

    const records = [
      makeTx({ timestampMs: t1 }),
      makeTx({ timestampMs: t2 }),
      makeTx({ timestampMs: t3 }),
    ];

    const filtered = filterByDateRange(records, t1, t3);
    expect(filtered.length).toBe(2);
  });

  it('returns empty for no overlap', () => {
    const records = [makeTx({ timestampMs: new Date('2026-01-01').getTime() })];
    const filtered = filterByDateRange(records,
      new Date('2026-06-01').getTime(),
      new Date('2026-07-01').getTime());
    expect(filtered.length).toBe(0);
  });
});

describe('sharesToCoins', () => {
  it('computes 6-decimal coin amount', () => {
    // 1000 shares out of 1000000 coins = 0.001000 coins
    expect(sharesToCoins(1000n, 1000000n)).toBe('0.001000');
  });

  it('returns empty for zero coin count', () => {
    expect(sharesToCoins(1000n, 0n)).toBe('');
  });

  it('handles large numbers', () => {
    // 77371252455336267181195264 shares / 1000000000 coins
    const shares = 77371252455336267181195264n;
    const coins = 1000000000n;
    const result = sharesToCoins(shares, coins);
    expect(result).toContain('.');
    expect(result.split('.')[1].length).toBe(6);
  });
});

describe('salesReportCsv', () => {
  it('generates valid CSV header and rows', () => {
    const summaries = [
      { label: '2026-03-01', startMs: 0, totalShares: 1000n, txCount: 5 },
      { label: '2026-03-02', startMs: 86400000, totalShares: 2000n, txCount: 3 },
    ];

    const csv = salesReportCsv(summaries, 'BCG', '1000000', 'daily');
    const lines = csv.split('\n');
    expect(lines[0]).toBe('period,daily_total_shares,daily_total_coins,transaction_count,avg_coins_per_tx');
    expect(lines[1]).toContain('2026-03-01');
    expect(lines[1]).toContain('1000');
    expect(lines[1]).toContain('5');
    expect(lines.length).toBe(3);
  });

  it('includes average coins per transaction', () => {
    const summaries = [
      { label: '2026-03', startMs: 0, totalShares: 900n, txCount: 3 },
    ];
    const csv = salesReportCsv(summaries, 'BCG', '1000000', 'monthly');
    const row = csv.split('\n')[1];
    // avg = 300 shares / 1000000 coinCount = 0.000300 coins
    expect(row).toContain('0.000300');
  });
});
