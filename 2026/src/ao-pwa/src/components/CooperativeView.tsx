import { useState, useEffect, useMemo } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient } from '../api/client.ts';
import {
  buildCoopNote,
  scanBlocksForCoopRecords,
  aggregateDeliveries,
  aggregateSales,
  findAllLots,
  traceLotProvenance,
  type CoopRecord,
  type DeliveryRecord,
  type SaleRecord,
  type CostRecord,
  type AdvanceRecord,
} from '../core/cooperativeMetadata.ts';

export function CooperativeView() {
  const { recorderUrl, selectedChainId } = useStore();
  const [records, setRecords] = useState<CoopRecord[]>([]);
  const [tab, setTab] = useState<'entry' | 'ledger' | 'provenance'>('entry');
  const [loadError, setLoadError] = useState<string | null>(null);

  // Load existing cooperative records from chain blocks
  useEffect(() => {
    if (!selectedChainId) return;
    const client = new RecorderClient(recorderUrl);
    let cancelled = false;

    client.getBlocks(selectedChainId).then(blocks => {
      if (cancelled) return;
      const entries = scanBlocksForCoopRecords(blocks);
      setRecords(entries.map(e => e.record));
      setLoadError(null);
    }).catch(e => {
      if (!cancelled) setLoadError(String(e));
    });

    return () => { cancelled = true; };
  }, [recorderUrl, selectedChainId]);

  if (!selectedChainId) {
    return <div style={{ padding: 16, color: '#666' }}>Select a chain to manage cooperative records.</div>;
  }

  return (
    <div style={{ padding: 16 }}>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <TabButton label="Record Entry" active={tab === 'entry'} onClick={() => setTab('entry')} />
        <TabButton label="Ledger" active={tab === 'ledger'} onClick={() => setTab('ledger')} />
        <TabButton label="Provenance" active={tab === 'provenance'} onClick={() => setTab('provenance')} />
      </div>

      {loadError && (
        <div style={{ fontSize: 12, color: '#c00', marginBottom: 8 }}>
          Failed to load chain records: {loadError}
        </div>
      )}

      {tab === 'entry' && <RecordEntryPanel />}
      {tab === 'ledger' && <LedgerPanel records={records} />}
      {tab === 'provenance' && <ProvenancePanel records={records} />}
    </div>
  );
}

function TabButton({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: '4px 12px', fontSize: 13, cursor: 'pointer',
        fontWeight: active ? 600 : 400,
        borderBottom: active ? '2px solid #2a7' : '2px solid transparent',
        background: 'none', border: 'none', borderRadius: 0,
      }}
    >
      {label}
    </button>
  );
}

// --- Record entry panel ---

function RecordEntryPanel() {
  const [recordType, setRecordType] = useState<'delivery' | 'sale' | 'cost' | 'advance'>('delivery');
  const [status, setStatus] = useState('');

  const [crop, setCrop] = useState('');
  const [weightKg, setWeightKg] = useState('');
  const [grade, setGrade] = useState('');
  const [lot, setLot] = useState('');
  const [location, setLocation] = useState('');
  const [pricePerKg, setPricePerKg] = useState('');
  const [buyer, setBuyer] = useState('');
  const [market, setMarket] = useState('');
  const [category, setCategory] = useState('');
  const [description, setDescription] = useState('');
  const [total, setTotal] = useState('');
  const [split, setSplit] = useState('');
  const [season, setSeason] = useState('');
  const [purpose, setPurpose] = useState('');

  function buildRecord(): CoopRecord | null {
    switch (recordType) {
      case 'delivery': {
        if (!crop) return null;
        return {
          type: 'delivery', crop,
          weight_kg: weightKg ? parseFloat(weightKg) : undefined,
          grade: grade || undefined, lot: lot || undefined, location: location || undefined,
        };
      }
      case 'sale': {
        if (!crop) return null;
        return {
          type: 'sale', crop,
          weight_kg: weightKg ? parseFloat(weightKg) : undefined,
          price_per_kg: pricePerKg ? parseFloat(pricePerKg) : undefined,
          buyer: buyer || undefined, market: market || undefined, lot: lot || undefined,
        };
      }
      case 'cost': {
        if (!category) return null;
        return {
          type: 'cost', category,
          description: description || undefined,
          total: total ? parseFloat(total) : undefined,
          split: split ? parseInt(split, 10) : undefined,
        };
      }
      case 'advance':
        return { type: 'advance', season: season || undefined, purpose: purpose || undefined };
    }
  }

  // Compute preview without side effects (C2 fix)
  const previewRecord = useMemo(
    () => buildRecord(),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [recordType, crop, weightKg, grade, lot, location, pricePerKg, buyer, market, category, description, total, split, season, purpose],
  );

  function handleAttachNote() {
    const record = buildRecord();
    if (!record) {
      setStatus('Required fields are missing');
      return;
    }
    const noteText = buildCoopNote(record);
    useStore.getState().setPendingCoopNote(noteText);
    setStatus('Note prepared — switch to Consumer view to attach it to a transfer');
    clearForm();
  }

  function clearForm() {
    setCrop(''); setWeightKg(''); setGrade(''); setLot(''); setLocation('');
    setPricePerKg(''); setBuyer(''); setMarket('');
    setCategory(''); setDescription(''); setTotal(''); setSplit('');
    setSeason(''); setPurpose('');
  }

  const inputStyle = { padding: '4px 6px', fontSize: 13 };

  return (
    <div style={{ background: '#f9f9f9', padding: 12, borderRadius: 4 }}>
      <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 8 }}>New Record</div>

      <div style={{ marginBottom: 8 }}>
        <select value={recordType} onChange={e => setRecordType(e.target.value as any)} style={{ fontSize: 13 }}>
          <option value="delivery">Delivery</option>
          <option value="sale">Sale</option>
          <option value="cost">Cost Allocation</option>
          <option value="advance">Advance/Credit</option>
        </select>
      </div>

      <div style={{ display: 'grid', gap: 6, maxWidth: 400 }}>
        {(recordType === 'delivery' || recordType === 'sale') && (
          <>
            <input value={crop} onChange={e => setCrop(e.target.value)} placeholder="Crop *" style={inputStyle} />
            <input value={weightKg} onChange={e => setWeightKg(e.target.value)} placeholder="Weight (kg)" type="number" style={inputStyle} />
          </>
        )}
        {recordType === 'delivery' && (
          <>
            <input value={grade} onChange={e => setGrade(e.target.value)} placeholder="Grade (e.g. A, B)" style={inputStyle} />
            <input value={lot} onChange={e => setLot(e.target.value)} placeholder="Lot ID (e.g. 2026-W10-012)" style={inputStyle} />
            <input value={location} onChange={e => setLocation(e.target.value)} placeholder="Collection point" style={inputStyle} />
          </>
        )}
        {recordType === 'sale' && (
          <>
            <input value={pricePerKg} onChange={e => setPricePerKg(e.target.value)} placeholder="Price per kg" type="number" style={inputStyle} />
            <input value={buyer} onChange={e => setBuyer(e.target.value)} placeholder="Buyer name" style={inputStyle} />
            <input value={market} onChange={e => setMarket(e.target.value)} placeholder="Market / destination" style={inputStyle} />
            <input value={lot} onChange={e => setLot(e.target.value)} placeholder="Lot ID (for provenance)" style={inputStyle} />
          </>
        )}
        {recordType === 'cost' && (
          <>
            <input value={category} onChange={e => setCategory(e.target.value)} placeholder="Category * (e.g. transport)" style={inputStyle} />
            <input value={description} onChange={e => setDescription(e.target.value)} placeholder="Description" style={inputStyle} />
            <input value={total} onChange={e => setTotal(e.target.value)} placeholder="Total cost" type="number" style={inputStyle} />
            <input value={split} onChange={e => setSplit(e.target.value)} placeholder="Split among (members)" type="number" style={inputStyle} />
          </>
        )}
        {recordType === 'advance' && (
          <>
            <input value={season} onChange={e => setSeason(e.target.value)} placeholder="Season (e.g. 2026-long-rains)" style={inputStyle} />
            <input value={purpose} onChange={e => setPurpose(e.target.value)} placeholder="Purpose (e.g. seed+fertilizer)" style={inputStyle} />
          </>
        )}

        <button onClick={handleAttachNote} style={{ fontSize: 13, padding: '6px 12px' }}>
          Prepare {recordType.charAt(0).toUpperCase() + recordType.slice(1)} Note
        </button>
      </div>

      {status && (
        <div style={{
          marginTop: 8, fontSize: 12, padding: '4px 8px', borderRadius: 4,
          background: status.startsWith('Error') || status.startsWith('Required') ? '#fee' : '#f0f7f0',
          color: status.startsWith('Error') || status.startsWith('Required') ? '#c00' : '#2a7',
        }}>
          {status}
        </div>
      )}

      <NotePreview record={previewRecord} />
      <PendingNoteIndicator />
    </div>
  );
}

function NotePreview({ record }: { record: CoopRecord | null }) {
  if (!record) return null;
  const text = buildCoopNote(record);
  return (
    <div style={{ marginTop: 8 }}>
      <div style={{ fontSize: 11, color: '#999' }}>NOTE preview:</div>
      <pre style={{ fontSize: 11, background: '#eee', padding: 6, borderRadius: 3, whiteSpace: 'pre-wrap' }}>{text}</pre>
    </div>
  );
}

function PendingNoteIndicator() {
  const { pendingCoopNote, setPendingCoopNote } = useStore();
  if (!pendingCoopNote) return null;
  return (
    <div style={{ marginTop: 8, padding: 8, background: '#f0f7f0', borderRadius: 4, fontSize: 12 }}>
      <div style={{ fontWeight: 500, color: '#2a7' }}>Pending note (will attach to next transfer):</div>
      <pre style={{ fontSize: 11, margin: '4px 0', whiteSpace: 'pre-wrap' }}>{pendingCoopNote}</pre>
      <button onClick={() => setPendingCoopNote(null)} style={{ fontSize: 11 }}>Clear</button>
    </div>
  );
}

// --- Ledger panel ---

function LedgerPanel({ records }: { records: CoopRecord[] }) {
  const deliveries = records.filter((r): r is DeliveryRecord => r.type === 'delivery');
  const sales = records.filter((r): r is SaleRecord => r.type === 'sale');
  const costs = records.filter((r): r is CostRecord => r.type === 'cost');
  const advances = records.filter((r): r is AdvanceRecord => r.type === 'advance');

  const deliveryLedger = aggregateDeliveries(deliveries);
  const saleSummary = aggregateSales(sales);

  return (
    <div>
      <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>Delivery Ledger ({deliveries.length} records)</h3>
      {deliveryLedger.length === 0 ? (
        <div style={{ fontSize: 12, color: '#999' }}>No deliveries recorded yet.</div>
      ) : (
        <table style={{ borderCollapse: 'collapse', fontSize: 13, marginBottom: 16 }}>
          <thead><tr style={{ borderBottom: '1px solid #ccc' }}><Th>Crop</Th><Th>Total kg</Th><Th>Deliveries</Th><Th>Grades</Th></tr></thead>
          <tbody>
            {deliveryLedger.map(l => (
              <tr key={l.crop}>
                <Td>{l.crop}</Td><Td>{l.totalKg.toFixed(1)}</Td><Td>{l.deliveryCount}</Td>
                <Td>{Object.entries(l.grades).map(([g, n]) => `${g}:${n}`).join(', ') || '—'}</Td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>Sales Summary ({sales.length} records)</h3>
      {saleSummary.length === 0 ? (
        <div style={{ fontSize: 12, color: '#999' }}>No sales recorded yet.</div>
      ) : (
        <table style={{ borderCollapse: 'collapse', fontSize: 13, marginBottom: 16 }}>
          <thead><tr style={{ borderBottom: '1px solid #ccc' }}><Th>Crop</Th><Th>Total kg</Th><Th>Revenue</Th><Th>Sales</Th></tr></thead>
          <tbody>
            {saleSummary.map(s => (
              <tr key={s.crop}>
                <Td>{s.crop}</Td><Td>{s.totalKg.toFixed(1)}</Td><Td>{s.totalRevenue.toLocaleString()}</Td><Td>{s.saleCount}</Td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {costs.length > 0 && (
        <>
          <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>Cost Allocations ({costs.length})</h3>
          <table style={{ borderCollapse: 'collapse', fontSize: 13, marginBottom: 16 }}>
            <thead><tr style={{ borderBottom: '1px solid #ccc' }}><Th>Category</Th><Th>Total</Th><Th>Split</Th><Th>Per Member</Th></tr></thead>
            <tbody>
              {costs.map((c, i) => (
                <tr key={i}>
                  <Td>{c.category}</Td><Td>{c.total?.toLocaleString() ?? '—'}</Td><Td>{c.split ?? '—'}</Td>
                  <Td>{c.total && c.split ? (c.total / c.split).toFixed(0) : '—'}</Td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {advances.length > 0 && (
        <>
          <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>Advances ({advances.length})</h3>
          <table style={{ borderCollapse: 'collapse', fontSize: 13 }}>
            <thead><tr style={{ borderBottom: '1px solid #ccc' }}><Th>Season</Th><Th>Purpose</Th></tr></thead>
            <tbody>
              {advances.map((a, i) => (
                <tr key={i}><Td>{a.season ?? '—'}</Td><Td>{a.purpose ?? '—'}</Td></tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}

// --- Provenance panel ---

function ProvenancePanel({ records }: { records: CoopRecord[] }) {
  const lots = findAllLots(records);
  const [selectedLot, setSelectedLot] = useState<string | null>(null);

  if (lots.length === 0) {
    return <div style={{ fontSize: 12, color: '#999' }}>No lot identifiers found. Record deliveries or sales with lot IDs to trace provenance.</div>;
  }

  const chain = selectedLot ? traceLotProvenance(records, selectedLot) : null;

  return (
    <div>
      <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 8 }}>Lot Provenance</div>
      <select value={selectedLot ?? ''} onChange={e => setSelectedLot(e.target.value || null)} style={{ fontSize: 13, marginBottom: 12 }}>
        <option value="">Select a lot...</option>
        {lots.map(l => <option key={l} value={l}>{l}</option>)}
      </select>

      {chain && (
        <div>
          <h4 style={{ fontSize: 13, margin: '8px 0 4px', color: '#2a7' }}>Lot: {chain.lot}</h4>

          {chain.deliveries.length > 0 && (
            <div style={{ marginBottom: 8 }}>
              <div style={{ fontSize: 12, fontWeight: 500, color: '#444' }}>Deliveries ({chain.deliveries.length})</div>
              {chain.deliveries.map((d, i) => (
                <div key={i} style={{ fontSize: 12, fontFamily: 'monospace', padding: '2px 0' }}>
                  {d.crop} — {d.weight_kg ?? '?'} kg, grade {d.grade ?? '?'}
                  {d.location && ` @ ${d.location}`}
                </div>
              ))}
            </div>
          )}

          {chain.sales.length > 0 && (
            <div>
              <div style={{ fontSize: 12, fontWeight: 500, color: '#444' }}>Sales ({chain.sales.length})</div>
              {chain.sales.map((s, i) => (
                <div key={i} style={{ fontSize: 12, fontFamily: 'monospace', padding: '2px 0' }}>
                  {s.crop} — {s.weight_kg ?? '?'} kg @ {s.price_per_kg ?? '?'}/kg
                  {s.buyer && ` → ${s.buyer}`}
                </div>
              ))}
            </div>
          )}

          {chain.deliveries.length > 0 && chain.sales.length === 0 && (
            <div style={{ fontSize: 12, color: '#999', fontStyle: 'italic' }}>
              No sales recorded for this lot yet.
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// --- Table helpers ---

function Th({ children }: { children: React.ReactNode }) {
  return <th style={{ padding: '4px 12px 4px 0', textAlign: 'left', fontWeight: 500, color: '#444' }}>{children}</th>;
}

function Td({ children }: { children: React.ReactNode }) {
  return <td style={{ padding: '4px 12px 4px 0' }}>{children}</td>;
}
