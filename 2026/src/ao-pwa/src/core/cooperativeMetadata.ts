// Cooperative metadata parser and builder.
// Encodes/decodes structured key:value NOTE content per CooperativeMetadata.md spec.

import * as tc from './typecodes.ts';
import { bytesItem, type DataItem, type DataItemJson } from './dataitem.ts';
import { hexToBytes } from './hex.ts';

// --- Record types ---

export type CoopRecordType = 'delivery' | 'sale' | 'cost' | 'advance';

export interface DeliveryRecord {
  type: 'delivery';
  crop: string;
  weight_kg?: number;
  grade?: string;
  lot?: string;
  location?: string;
  [key: string]: string | number | undefined;
}

export interface SaleRecord {
  type: 'sale';
  crop: string;
  weight_kg?: number;
  price_per_kg?: number;
  buyer?: string;
  market?: string;
  lot?: string;
  [key: string]: string | number | undefined;
}

export interface CostRecord {
  type: 'cost';
  category: string;
  description?: string;
  total?: number;
  split?: number;
  [key: string]: string | number | undefined;
}

export interface AdvanceRecord {
  type: 'advance';
  season?: string;
  purpose?: string;
  [key: string]: string | number | undefined;
}

export type CoopRecord = DeliveryRecord | SaleRecord | CostRecord | AdvanceRecord;

// --- Parsing ---

/** Safe parseFloat that returns undefined for NaN. */
function safeFloat(s: string): number | undefined {
  const n = parseFloat(s);
  return Number.isFinite(n) ? n : undefined;
}

/** Safe parseInt that returns undefined for NaN. */
function safeInt(s: string): number | undefined {
  const n = parseInt(s, 10);
  return Number.isFinite(n) ? n : undefined;
}

/** Parse a key:value NOTE string into a structured record. Returns null if no type field. */
export function parseCoopNote(text: string): CoopRecord | null {
  const fields: Record<string, string> = {};
  for (const line of text.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    const colonIdx = trimmed.indexOf(':');
    if (colonIdx < 1) continue;
    const key = trimmed.slice(0, colonIdx).trim().toLowerCase();
    const value = trimmed.slice(colonIdx + 1).trim();
    fields[key] = value;
  }

  const recordType = fields['type'];
  if (!recordType) return null;

  switch (recordType) {
    case 'delivery':
      return {
        type: 'delivery',
        crop: fields['crop'] ?? '',
        weight_kg: fields['weight_kg'] ? safeFloat(fields['weight_kg']) : undefined,
        grade: fields['grade'],
        lot: fields['lot'],
        location: fields['location'],
      };
    case 'sale':
      return {
        type: 'sale',
        crop: fields['crop'] ?? '',
        weight_kg: fields['weight_kg'] ? safeFloat(fields['weight_kg']) : undefined,
        price_per_kg: fields['price_per_kg'] ? safeFloat(fields['price_per_kg']) : undefined,
        buyer: fields['buyer'],
        market: fields['market'],
        lot: fields['lot'],
      };
    case 'cost':
      return {
        type: 'cost',
        category: fields['category'] ?? '',
        description: fields['description'],
        total: fields['total'] ? safeFloat(fields['total']) : undefined,
        split: fields['split'] ? safeInt(fields['split']) : undefined,
      };
    case 'advance':
      return {
        type: 'advance',
        season: fields['season'],
        purpose: fields['purpose'],
      };
    default:
      return null;
  }
}

/** Build a NOTE content string from a CoopRecord. */
export function buildCoopNote(record: CoopRecord): string {
  const lines: string[] = [];
  lines.push(`type:${record.type}`);

  for (const [key, value] of Object.entries(record)) {
    if (key === 'type') continue;
    if (value === undefined || value === '') continue;
    // Sanitize: replace newlines with spaces to prevent injection
    const safe = String(value).replace(/[\n\r]/g, ' ');
    lines.push(`${key}:${safe}`);
  }

  return lines.join('\n');
}

/** Build a NOTE DataItem from a CoopRecord. */
export function buildCoopNoteItem(record: CoopRecord): DataItem {
  const text = buildCoopNote(record);
  return bytesItem(tc.NOTE, new TextEncoder().encode(text));
}

// --- Scanning blocks for cooperative records ---

export interface CoopBlockEntry {
  record: CoopRecord;
}

/**
 * Extract cooperative records from blocks returned by RecorderClient.getBlocks().
 * Each block is a DataItemJson (container with .items children).
 */
export function scanBlocksForCoopRecords(blocks: DataItemJson[]): CoopBlockEntry[] {
  const entries: CoopBlockEntry[] = [];
  for (const block of blocks) {
    findNotesInItem(block, entries);
  }
  return entries;
}

/** Recursive walk through DataItemJson to find NOTE items. */
function findNotesInItem(item: DataItemJson, out: CoopBlockEntry[]): void {
  // NOTE type code = 32
  if (item.code === 32 && typeof item.value === 'string') {
    try {
      const bytes = hexToBytes(item.value);
      const text = new TextDecoder().decode(bytes);
      const record = parseCoopNote(text);
      if (record) {
        out.push({ record });
      }
    } catch { /* skip malformed */ }
  }

  // Recurse into containers
  if (Array.isArray(item.items)) {
    for (const child of item.items) {
      findNotesInItem(child, out);
    }
  }
}

// --- Aggregation ---

export interface DeliveryLedger {
  crop: string;
  totalKg: number;
  deliveryCount: number;
  grades: Record<string, number>; // grade → count
}

export interface SaleSummary {
  crop: string;
  totalKg: number;
  totalRevenue: number;
  saleCount: number;
}

/** Aggregate delivery records into per-crop ledger. */
export function aggregateDeliveries(records: DeliveryRecord[]): DeliveryLedger[] {
  const byCrop = new Map<string, DeliveryLedger>();

  for (const r of records) {
    const key = r.crop.toLowerCase();
    let ledger = byCrop.get(key);
    if (!ledger) {
      ledger = { crop: r.crop, totalKg: 0, deliveryCount: 0, grades: {} };
      byCrop.set(key, ledger);
    }
    ledger.deliveryCount++;
    if (r.weight_kg != null && Number.isFinite(r.weight_kg)) {
      ledger.totalKg += r.weight_kg;
    }
    if (r.grade) {
      ledger.grades[r.grade] = (ledger.grades[r.grade] || 0) + 1;
    }
  }

  return Array.from(byCrop.values());
}

/** Aggregate sale records into per-crop summary. */
export function aggregateSales(records: SaleRecord[]): SaleSummary[] {
  const byCrop = new Map<string, SaleSummary>();

  for (const r of records) {
    const key = r.crop.toLowerCase();
    let summary = byCrop.get(key);
    if (!summary) {
      summary = { crop: r.crop, totalKg: 0, totalRevenue: 0, saleCount: 0 };
      byCrop.set(key, summary);
    }
    summary.saleCount++;
    const wt = r.weight_kg;
    const px = r.price_per_kg;
    if (wt != null && Number.isFinite(wt)) summary.totalKg += wt;
    if (wt != null && px != null && Number.isFinite(wt) && Number.isFinite(px)) {
      summary.totalRevenue += wt * px;
    }
  }

  return Array.from(byCrop.values());
}

// --- Provenance ---

export interface ProvenanceChain {
  lot: string;
  deliveries: DeliveryRecord[];
  sales: SaleRecord[];
}

/** Trace provenance for a specific lot across all records. */
export function traceLotProvenance(records: CoopRecord[], lot: string): ProvenanceChain {
  const chain: ProvenanceChain = { lot, deliveries: [], sales: [] };

  for (const r of records) {
    if (r.type === 'delivery' && r.lot === lot) {
      chain.deliveries.push(r);
    }
    if (r.type === 'sale' && r.lot === lot) {
      chain.sales.push(r);
    }
  }

  return chain;
}

/** Find all unique lot identifiers across delivery and sale records. */
export function findAllLots(records: CoopRecord[]): string[] {
  const lots = new Set<string>();
  for (const r of records) {
    if ((r.type === 'delivery' || r.type === 'sale') && r.lot) lots.add(r.lot);
  }
  return Array.from(lots).sort();
}
