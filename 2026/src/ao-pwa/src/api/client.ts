// Recorder API client — typed fetch wrapper for all recorder endpoints.

import type { DataItemJson } from '../core/dataitem.ts';

export interface ExchangePairEntry {
  sell_symbol: string;
  buy_symbol: string;
  rate: number;
  spread?: number;
  min_trade?: number;
  max_trade?: number;
}

export interface ExchangeAgentEntry {
  name: string;
  pairs: ExchangePairEntry[];
  contact_url?: string;
  registered_at?: number;
  ttl?: number;
}

export interface VendorProfile {
  name?: string;
  description?: string;
  lat?: number;
  lon?: number;
}

export interface ChainListEntry {
  chain_id: string;
  symbol: string;
  block_height: number;
  exchange_agents?: ExchangeAgentEntry[];
  vendor_profile?: VendorProfile;
}

export interface ValidatorEndorsement {
  url: string;
  label?: string;
  validated_height: number;
  rolled_hash: string;
  status: string;
  last_checked: number;
}

export interface ChainInfo {
  chain_id: string;
  symbol: string;
  block_height: number;
  shares_out: string;
  coin_count: string;
  fee_rate_num: string;
  fee_rate_den: string;
  expiry_period: number;
  expiry_mode: number;
  next_seq_id: number;
  validators?: ValidatorEndorsement[];
}

export interface UtxoInfo {
  seq_id: number;
  pubkey: string;
  amount: string;
  block_height: number;
  block_timestamp: number;
  status: string;
}

export interface BlockInfo {
  height: number;
  hash: string;
  timestamp: number;
  shares_out: string;
  first_seq: number;
  seq_count: number;
}

export class RecorderClient {
  private readonly baseUrl: string;
  constructor(baseUrl: string) { this.baseUrl = baseUrl; }

  private async fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`, init);
    if (!res.ok) {
      const body = await res.text();
      throw new Error(`${res.status}: ${body}`);
    }
    return res.json();
  }

  /** GET /chains — list all hosted chains. */
  async listChains(): Promise<ChainListEntry[]> {
    return this.fetchJson('/chains');
  }

  /** POST /chains — create a new chain from genesis JSON. */
  async createChain(genesis: DataItemJson, blockmakerSeed?: string): Promise<ChainInfo> {
    const body: Record<string, unknown> = { genesis };
    if (blockmakerSeed) body.blockmaker_seed = blockmakerSeed;
    return this.fetchJson('/chains', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
  }

  /** GET /chain/{id}/info */
  async chainInfo(chainId: string): Promise<ChainInfo> {
    return this.fetchJson(`/chain/${chainId}/info`);
  }

  /** GET /chain/{id}/utxo/{seq_id} */
  async getUtxo(chainId: string, seqId: number): Promise<UtxoInfo> {
    return this.fetchJson(`/chain/${chainId}/utxo/${seqId}`);
  }

  /** GET /chain/{id}/blocks?from=&to= */
  async getBlocks(chainId: string, from?: number, to?: number): Promise<DataItemJson[]> {
    const params = new URLSearchParams();
    if (from !== undefined) params.set('from', String(from));
    if (to !== undefined) params.set('to', String(to));
    const qs = params.toString();
    return this.fetchJson(`/chain/${chainId}/blocks${qs ? '?' + qs : ''}`);
  }

  /** POST /chain/{id}/submit — submit an authorization JSON. */
  async submit(chainId: string, authorization: DataItemJson): Promise<BlockInfo> {
    return this.fetchJson(`/chain/${chainId}/submit`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(authorization),
    });
  }

  /** GET /chain/{id}/profile — get vendor profile. */
  async getProfile(chainId: string): Promise<VendorProfile> {
    return this.fetchJson(`/chain/${chainId}/profile`);
  }

  /** POST /chain/{id}/profile — set vendor profile. */
  async setProfile(chainId: string, profile: VendorProfile): Promise<void> {
    await this.fetchJson(`/chain/${chainId}/profile`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(profile),
    });
  }

  /** POST /chain/{id}/blob — upload blob, returns {hash: string} */
  async uploadBlob(chainId: string, data: Uint8Array): Promise<{ hash: string }> {
    const res = await fetch(`${this.baseUrl}/chain/${chainId}/blob`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/octet-stream' },
      body: data,
    });
    if (!res.ok) {
      const body = await res.text();
      throw new Error(`${res.status}: ${body}`);
    }
    return res.json();
  }

  /** GET /chain/{id}/blob/{hash} — retrieve blob content */
  async getBlob(chainId: string, hash: string): Promise<{ mime: string; data: Uint8Array }> {
    const res = await fetch(`${this.baseUrl}/chain/${chainId}/blob/${hash}`);
    if (!res.ok) {
      const body = await res.text();
      throw new Error(`${res.status}: ${body}`);
    }
    const contentType = res.headers.get('Content-Type') || 'application/octet-stream';
    const arrayBuf = await res.arrayBuffer();
    return { mime: contentType, data: new Uint8Array(arrayBuf) };
  }

  /** Subscribe to block events via SSE. Returns an EventSource. */
  subscribeBlocks(chainId: string, onBlock: (info: BlockInfo) => void): EventSource {
    const es = new EventSource(`${this.baseUrl}/chain/${chainId}/events`);
    es.addEventListener('block', (event) => {
      const info = JSON.parse((event as MessageEvent).data) as BlockInfo;
      onBlock(info);
    });
    return es;
  }
}
