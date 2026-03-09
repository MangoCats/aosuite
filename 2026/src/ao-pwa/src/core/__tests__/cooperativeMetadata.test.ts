import { describe, it, expect } from 'vitest';
import {
  parseCoopNote,
  buildCoopNote,
  buildCoopNoteItem,
  aggregateDeliveries,
  aggregateSales,
  traceLotProvenance,
  findAllLots,
  scanBlocksForCoopRecords,
  type DeliveryRecord,
  type SaleRecord,
  type CostRecord,
  type CoopRecord,
} from '../cooperativeMetadata.ts';
import * as tc from '../typecodes.ts';

describe('parseCoopNote', () => {
  it('parses a delivery record', () => {
    const text = `type:delivery
crop:tomatoes
weight_kg:180.5
grade:A
lot:2026-W10-012
location:Riuki Collection Point`;
    const record = parseCoopNote(text);
    expect(record).not.toBeNull();
    expect(record!.type).toBe('delivery');
    const d = record as DeliveryRecord;
    expect(d.crop).toBe('tomatoes');
    expect(d.weight_kg).toBeCloseTo(180.5);
    expect(d.grade).toBe('A');
    expect(d.lot).toBe('2026-W10-012');
    expect(d.location).toBe('Riuki Collection Point');
  });

  it('parses a sale record', () => {
    const text = `type:sale
crop:tomatoes
weight_kg:500
price_per_kg:45.00
buyer:Nairobi Fresh Market
market:Wakulima`;
    const record = parseCoopNote(text) as SaleRecord;
    expect(record.type).toBe('sale');
    expect(record.crop).toBe('tomatoes');
    expect(record.weight_kg).toBe(500);
    expect(record.price_per_kg).toBe(45);
    expect(record.buyer).toBe('Nairobi Fresh Market');
    expect(record.market).toBe('Wakulima');
  });

  it('parses a cost record', () => {
    const text = `type:cost
category:transport
description:Truck to Wakulima market
total:15000
split:47`;
    const record = parseCoopNote(text) as CostRecord;
    expect(record.type).toBe('cost');
    expect(record.category).toBe('transport');
    expect(record.total).toBe(15000);
    expect(record.split).toBe(47);
  });

  it('parses an advance record', () => {
    const text = `type:advance\nseason:2026-long-rains\npurpose:seed+fertilizer`;
    const record = parseCoopNote(text);
    expect(record).not.toBeNull();
    expect(record!.type).toBe('advance');
    expect((record as any).season).toBe('2026-long-rains');
    expect((record as any).purpose).toBe('seed+fertilizer');
  });

  it('ignores comments and blank lines', () => {
    const text = `# Delivery at Riuki
type:delivery
crop:mangoes

# Fresh batch
weight_kg:50`;
    const record = parseCoopNote(text) as DeliveryRecord;
    expect(record.crop).toBe('mangoes');
    expect(record.weight_kg).toBe(50);
  });

  it('returns null for no type field', () => {
    expect(parseCoopNote('crop:tomatoes\nweight_kg:100')).toBeNull();
  });

  it('returns null for unknown type', () => {
    expect(parseCoopNote('type:unknown\nfoo:bar')).toBeNull();
  });

  it('handles missing optional fields', () => {
    const record = parseCoopNote('type:delivery\ncrop:beans') as DeliveryRecord;
    expect(record.crop).toBe('beans');
    expect(record.weight_kg).toBeUndefined();
    expect(record.grade).toBeUndefined();
    expect(record.lot).toBeUndefined();
  });
});

describe('buildCoopNote', () => {
  it('builds a delivery note string', () => {
    const record: DeliveryRecord = {
      type: 'delivery',
      crop: 'tomatoes',
      weight_kg: 180,
      grade: 'A',
      lot: '2026-W10-012',
    };
    const text = buildCoopNote(record);
    expect(text).toContain('type:delivery');
    expect(text).toContain('crop:tomatoes');
    expect(text).toContain('weight_kg:180');
    expect(text).toContain('grade:A');
    expect(text).toContain('lot:2026-W10-012');
  });

  it('omits undefined and empty fields', () => {
    const record: DeliveryRecord = {
      type: 'delivery',
      crop: 'beans',
    };
    const text = buildCoopNote(record);
    expect(text).toBe('type:delivery\ncrop:beans');
    expect(text).not.toContain('weight_kg');
  });

  it('roundtrips through parse', () => {
    const original: DeliveryRecord = {
      type: 'delivery',
      crop: 'coffee',
      weight_kg: 25.5,
      grade: 'AA',
      lot: '2026-W03-001',
      location: 'Nyeri Mill',
    };
    const text = buildCoopNote(original);
    const parsed = parseCoopNote(text) as DeliveryRecord;
    expect(parsed.type).toBe('delivery');
    expect(parsed.crop).toBe('coffee');
    expect(parsed.weight_kg).toBeCloseTo(25.5);
    expect(parsed.grade).toBe('AA');
    expect(parsed.lot).toBe('2026-W03-001');
    expect(parsed.location).toBe('Nyeri Mill');
  });
});

describe('buildCoopNoteItem', () => {
  it('creates a NOTE DataItem with correct type code', () => {
    const item = buildCoopNoteItem({ type: 'delivery', crop: 'tomatoes' });
    expect(item.typeCode).toBe(tc.NOTE);
    expect(item.value.kind).toBe('bytes');
    if (item.value.kind === 'bytes') {
      const text = new TextDecoder().decode(item.value.data);
      expect(text).toContain('type:delivery');
      expect(text).toContain('crop:tomatoes');
    }
  });
});

describe('aggregateDeliveries', () => {
  it('aggregates by crop', () => {
    const records: DeliveryRecord[] = [
      { type: 'delivery', crop: 'tomatoes', weight_kg: 100, grade: 'A' },
      { type: 'delivery', crop: 'tomatoes', weight_kg: 80, grade: 'B' },
      { type: 'delivery', crop: 'beans', weight_kg: 50, grade: 'A' },
    ];
    const ledgers = aggregateDeliveries(records);
    expect(ledgers).toHaveLength(2);
    const tomatoes = ledgers.find(l => l.crop === 'tomatoes')!;
    expect(tomatoes.totalKg).toBe(180);
    expect(tomatoes.deliveryCount).toBe(2);
    expect(tomatoes.grades['A']).toBe(1);
    expect(tomatoes.grades['B']).toBe(1);
  });

  it('handles empty input', () => {
    expect(aggregateDeliveries([])).toEqual([]);
  });
});

describe('aggregateSales', () => {
  it('computes total revenue', () => {
    const records: SaleRecord[] = [
      { type: 'sale', crop: 'tomatoes', weight_kg: 200, price_per_kg: 45 },
      { type: 'sale', crop: 'tomatoes', weight_kg: 300, price_per_kg: 40 },
    ];
    const summaries = aggregateSales(records);
    expect(summaries).toHaveLength(1);
    expect(summaries[0].totalKg).toBe(500);
    expect(summaries[0].totalRevenue).toBe(200 * 45 + 300 * 40);
    expect(summaries[0].saleCount).toBe(2);
  });
});

describe('provenance', () => {
  const records: CoopRecord[] = [
    { type: 'delivery', crop: 'tomatoes', weight_kg: 100, lot: 'LOT-001', grade: 'A' },
    { type: 'delivery', crop: 'tomatoes', weight_kg: 80, lot: 'LOT-002', grade: 'B' },
    { type: 'sale', crop: 'tomatoes', weight_kg: 100, price_per_kg: 45, lot: 'LOT-001' },
    { type: 'delivery', crop: 'beans', weight_kg: 50, lot: 'LOT-001', grade: 'A' },
  ];

  it('findAllLots returns lots from deliveries and sales', () => {
    const lots = findAllLots(records);
    expect(lots).toEqual(['LOT-001', 'LOT-002']);
  });

  it('traceLotProvenance finds matching deliveries and sales', () => {
    const chain = traceLotProvenance(records, 'LOT-001');
    expect(chain.lot).toBe('LOT-001');
    expect(chain.deliveries).toHaveLength(2); // tomatoes + beans
    expect(chain.deliveries[0].crop).toBe('tomatoes');
    expect(chain.deliveries[1].crop).toBe('beans');
    expect(chain.sales).toHaveLength(1); // tomatoes sale with LOT-001
    expect(chain.sales[0].crop).toBe('tomatoes');
  });

  it('traceLotProvenance returns empty for unknown lot', () => {
    const chain = traceLotProvenance(records, 'LOT-999');
    expect(chain.deliveries).toHaveLength(0);
    expect(chain.sales).toHaveLength(0);
  });
});

describe('scanBlocksForCoopRecords', () => {
  it('extracts records from block items', () => {
    // Encode "type:delivery\ncrop:tomatoes" as hex
    const text = 'type:delivery\ncrop:tomatoes';
    const hex = Array.from(new TextEncoder().encode(text))
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');

    // DataItemJson uses { type: string, code: number, value?, items? }
    const blocks = [
      {
        type: 'BLOCK', code: 2,
        items: [
          {
            type: 'ASSIGNMENT', code: 1,
            items: [
              { type: 'NOTE', code: 32, value: hex },
              { type: 'AMOUNT', code: 10, value: 'aabb' },
            ],
          },
        ],
      },
    ];

    const entries = scanBlocksForCoopRecords(blocks);
    expect(entries).toHaveLength(1);
    expect(entries[0].record.type).toBe('delivery');
    expect((entries[0].record as DeliveryRecord).crop).toBe('tomatoes');
  });

  it('skips non-coop NOTE items', () => {
    const text = 'just a regular note';
    const hex = Array.from(new TextEncoder().encode(text))
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');

    const blocks = [{ type: 'BLOCK', code: 2, items: [{ type: 'NOTE', code: 32, value: hex }] }];
    const entries = scanBlocksForCoopRecords(blocks);
    expect(entries).toHaveLength(0);
  });

  it('finds sale records with lot for provenance', () => {
    const text = 'type:sale\ncrop:tomatoes\nlot:LOT-001\nweight_kg:100';
    const hex = Array.from(new TextEncoder().encode(text))
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');

    const blocks = [{ type: 'BLOCK', code: 2, items: [{ type: 'NOTE', code: 32, value: hex }] }];
    const entries = scanBlocksForCoopRecords(blocks);
    expect(entries).toHaveLength(1);
    expect(entries[0].record.type).toBe('sale');
    expect((entries[0].record as SaleRecord).lot).toBe('LOT-001');
  });
});
