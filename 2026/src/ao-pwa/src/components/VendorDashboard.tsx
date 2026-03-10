import { useState, useEffect, useRef, useMemo } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient, ssePool } from '../api/client.ts';
import type { ChainListEntry, BlockInfo } from '../api/client.ts';
import { getKeys } from '../core/walletDb.ts';

interface ChainCard {
  chainId: string;
  symbol: string;
  businessName: string;
  sessionTxCount: number;
  lastTxTime: number | null;
  sseConnected: boolean;
  blockHeight: number;
}

/** Multi-chain vendor dashboard — shows cards for all chains the vendor manages. */
export function VendorDashboard() {
  const { recorderUrl, chains, selectChain } = useStore();
  const [cards, setCards] = useState<ChainCard[]>([]);
  const [loading, setLoading] = useState(true);
  const unsubRefs = useRef<Map<string, () => void>>(new Map());

  // Discover owned chains: chains where we have keys in walletDb
  useEffect(() => {
    let cancelled = false;
    async function loadOwnedChains() {
      const allKeys = await getKeys();
      const ownedIds = new Set(allKeys.map(k => k.chainId));
      if (cancelled) return;

      // Filter chain list to owned chains
      const ownedChains = chains.filter(c => ownedIds.has(c.chain_id));

      // Fetch profiles for each owned chain (in parallel)
      const client = new RecorderClient(recorderUrl);
      const profileResults = await Promise.allSettled(
        ownedChains.map(async (chain) => {
          let name = chain.vendor_profile?.name ?? '';
          if (!name) {
            const p = await client.getProfile(chain.chain_id);
            name = p.name ?? '';
          }
          return { chain, name };
        })
      );
      const newCards: ChainCard[] = profileResults.map((result, i) => {
        const chain = ownedChains[i];
        const name = result.status === 'fulfilled' ? result.value.name : '';
        return {
          chainId: chain.chain_id,
          symbol: chain.symbol,
          businessName: name,
          sessionTxCount: 0,
          lastTxTime: null,
          sseConnected: false,
          blockHeight: chain.block_height,
        };
      });
      if (!cancelled) {
        setCards(newCards);
        setLoading(false);
      }
    }
    loadOwnedChains();
    return () => { cancelled = true; };
  }, [recorderUrl, chains]);

  // Stable chain ID list for SSE effect deps
  const cardChainIds = useMemo(
    () => cards.map(c => c.chainId),
    [cards.map(c => c.chainId).join(',')]
  );

  // SSE subscriptions for each card via shared pool
  useEffect(() => {
    const currentUnsubs = unsubRefs.current;

    // Close stale connections
    for (const [id, unsub] of currentUnsubs) {
      if (!cardChainIds.includes(id)) {
        unsub();
        currentUnsubs.delete(id);
      }
    }

    // Open new connections
    for (const chainId of cardChainIds) {
      if (currentUnsubs.has(chainId)) continue;
      const cid = chainId; // capture for closures
      const unsub = ssePool.subscribe(
        recorderUrl,
        cid,
        (block: BlockInfo) => {
          setCards(prev => prev.map(c => {
            if (c.chainId !== cid) return c;
            return {
              ...c,
              sessionTxCount: block.seq_count > 0
                ? c.sessionTxCount + block.seq_count
                : c.sessionTxCount,
              lastTxTime: block.seq_count > 0 ? Date.now() : c.lastTxTime,
              blockHeight: block.height,
            };
          }));
        },
        (connected: boolean) => {
          setCards(prev => prev.map(c =>
            c.chainId === cid ? { ...c, sseConnected: connected } : c
          ));
        },
      );
      currentUnsubs.set(chainId, unsub);
    }

    return () => {
      for (const [, unsub] of currentUnsubs) unsub();
      currentUnsubs.clear();
    };
  }, [recorderUrl, cardChainIds]);

  if (loading) {
    return <div style={{ padding: 16, color: '#666', fontSize: 13 }}>Loading vendor chains...</div>;
  }

  if (cards.length === 0) {
    return (
      <div style={{ padding: 16, color: '#666', fontSize: 13 }}>
        No vendor chains found. Create a chain first, or add keys for an existing chain.
      </div>
    );
  }

  // Combined revenue
  const totalRevenue = cards.reduce((sum, c) => sum + c.sessionTxCount, 0);

  return (
    <div style={{ padding: 16 }}>
      <div style={{ marginBottom: 12 }}>
        <span style={{ fontSize: 15, fontWeight: 600 }}>Vendor Dashboard</span>
        <span style={{ marginLeft: 12, fontSize: 13, color: '#666' }}>
          {cards.length} chain{cards.length !== 1 ? 's' : ''} — {totalRevenue} tx this session
        </span>
      </div>

      <div style={{
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fill, minmax(240px, 1fr))',
        gap: 12,
      }}>
        {cards.map(card => (
          <div
            key={card.chainId}
            onClick={() => selectChain(card.chainId)}
            style={{
              padding: 12,
              background: '#f9f9f9',
              borderRadius: 6,
              border: '1px solid #ddd',
              cursor: 'pointer',
            }}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ fontSize: 16, fontWeight: 600 }}>{card.symbol}</span>
              <span style={{
                fontSize: 10,
                color: card.sseConnected ? '#090' : '#c00',
              }}>
                {card.sseConnected ? 'LIVE' : 'OFF'}
              </span>
            </div>
            {card.businessName && (
              <div style={{ fontSize: 12, color: '#666', marginTop: 2 }}>{card.businessName}</div>
            )}
            <div style={{ fontSize: 13, marginTop: 8 }}>
              <div>Block: {card.blockHeight}</div>
              <div>Session: {card.sessionTxCount} tx</div>
              {card.lastTxTime && (
                <div style={{ fontSize: 11, color: '#999' }}>
                  Last: {formatTimeAgo(card.lastTxTime)}
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function formatTimeAgo(ms: number): string {
  const secs = Math.floor((Date.now() - ms) / 1000);
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  return `${Math.floor(secs / 3600)}h ago`;
}
