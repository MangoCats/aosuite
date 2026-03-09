// Recorder API client — typed fetch wrapper for all recorder endpoints.

import type { DataItemJson } from '../core/dataitem.ts';

/** Thrown when a blob has been pruned per retention policy (HTTP 410 Gone). */
export class BlobPrunedError extends Error {
  public readonly hash: string;
  constructor(hash: string) {
    super(`Blob ${hash.slice(0, 12)}… was pruned per retention policy`);
    this.name = 'BlobPrunedError';
    this.hash = hash;
  }
}

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

export interface PendingRecorderChangeInfo {
  new_recorder_pubkey: string;
  new_recorder_url: string;
  pending_height: number;
}

export interface OwnerKeyInfo {
  pubkey: string;
  added_height: number;
  added_timestamp: number;
  expires_at?: number;
  status: string;
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
  // TⒶ³ fields
  recorder_pubkey?: string;
  reward_rate_num: string;
  reward_rate_den: string;
  frozen: boolean;
  pending_recorder_change?: PendingRecorderChangeInfo;
  key_rotation_rate: string;
  revocation_rate_base: string;
  owner_key_count: number;
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

export interface BlobRule {
  mime_pattern: string;
  max_blob_size?: number;
  retention_secs?: number;
  priority?: number;
}

export interface BlobPolicyResponse {
  rules: BlobRule[];
  capacity_limit?: number;
  throttle_threshold?: number;
}

export interface CaaSubmitResponse {
  caa_hash: string;
  chain_id: string;
  block_height: number;
  block_hash: string;
  first_seq: number;
  seq_count: number;
  proof_json: DataItemJson;
}

export interface CaaStatusResponse {
  caa_hash: string;
  status: string; // 'escrowed' | 'binding' | 'finalized' | 'expired'
  chain_order: number;
  deadline: number;
  block_height: number;
  has_proof: boolean;
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

  /** GET /chain/{id}/owner-keys — list all owner keys with status. */
  async getOwnerKeys(chainId: string): Promise<OwnerKeyInfo[]> {
    return this.fetchJson(`/chain/${chainId}/owner-keys`);
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

  /** GET /chain/{id}/blob/{hash} — retrieve blob content.
   *  Throws BlobPrunedError (410) if the blob was pruned per retention policy. */
  async getBlob(chainId: string, hash: string): Promise<{ mime: string; data: Uint8Array }> {
    const res = await fetch(`${this.baseUrl}/chain/${chainId}/blob/${hash}`);
    if (res.status === 410) {
      throw new BlobPrunedError(hash);
    }
    if (!res.ok) {
      const body = await res.text();
      throw new Error(`${res.status}: ${body}`);
    }
    const contentType = res.headers.get('Content-Type') || 'application/octet-stream';
    const arrayBuf = await res.arrayBuffer();
    return { mime: contentType, data: new Uint8Array(arrayBuf) };
  }

  /** GET /chain/{id}/blob-policy — get chain's blob retention policy from genesis. */
  async getBlobPolicy(chainId: string): Promise<BlobPolicyResponse | null> {
    const res = await fetch(`${this.baseUrl}/chain/${chainId}/blob-policy`);
    if (!res.ok) return null;
    const data = await res.json();
    // Server returns null JSON value if no policy exists
    if (!data || !data.items) return null;
    return parseBlobPolicyJson(data);
  }

  /** POST /chain/{id}/refute — submit a refutation for an agreement hash. */
  async refute(chainId: string, agreementHash: string): Promise<void> {
    await this.fetchJson(`/chain/${chainId}/refute`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ agreement_hash: agreementHash }),
    });
  }

  // ── CAA Escrow Endpoints ───────────────────────────────────────────

  /** POST /chain/{id}/caa/submit — submit CAA DataItem JSON for escrow recording. */
  async caaSubmit(chainId: string, caaJson: DataItemJson): Promise<CaaSubmitResponse> {
    return this.fetchJson(`/chain/${chainId}/caa/submit`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(caaJson),
    });
  }

  /** POST /chain/{id}/caa/bind — submit binding proofs to finalize escrow. */
  async caaBind(chainId: string, caaHash: string, proofs: DataItemJson[]): Promise<CaaStatusResponse> {
    return this.fetchJson(`/chain/${chainId}/caa/bind`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ caa_hash: caaHash, proofs }),
    });
  }

  /** GET /chain/{id}/caa/{caaHash} — query CAA escrow status. */
  async caaStatus(chainId: string, caaHash: string): Promise<CaaStatusResponse> {
    return this.fetchJson(`/chain/${chainId}/caa/${caaHash}`);
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

// ── Blob Policy JSON Parser ───────────────────────────────────────

import { hexToBytes } from '../core/hex.ts';
import * as tc from '../core/typecodes.ts';

/** Parse a BLOB_POLICY DataItemJson into a typed BlobPolicyResponse. */
function parseBlobPolicyJson(json: DataItemJson): BlobPolicyResponse {
  const rules: BlobRule[] = [];
  let capacity_limit: number | undefined;
  let throttle_threshold: number | undefined;

  for (const child of json.items ?? []) {
    if (child.code === Number(tc.BLOB_RULE)) {
      const rule = parseBlobRuleJson(child);
      if (rule) rules.push(rule);
    } else if (child.code === Number(tc.CAPACITY_LIMIT) && child.value != null) {
      capacity_limit = decodeBigintHex(String(child.value));
    } else if (child.code === Number(tc.THROTTLE_THRESHOLD) && child.value != null) {
      throttle_threshold = decodeBigintHex(String(child.value));
    }
  }

  return { rules, capacity_limit, throttle_threshold };
}

function parseBlobRuleJson(json: DataItemJson): BlobRule | null {
  let mime_pattern = '';
  let max_blob_size: number | undefined;
  let retention_secs: number | undefined;
  let priority: number | undefined;

  for (const child of json.items ?? []) {
    if (child.code === Number(tc.MIME_PATTERN) && child.value != null) {
      const bytes = hexToBytes(String(child.value));
      mime_pattern = new TextDecoder().decode(bytes);
    } else if (child.code === Number(tc.MAX_BLOB_SIZE) && child.value != null) {
      max_blob_size = decodeBigintHex(String(child.value));
    } else if (child.code === Number(tc.RETENTION_SECS) && child.value != null) {
      // RETENTION_SECS is Fixed(8), stored as 8-byte big-endian timestamp
      // Convert from AO timestamp (Unix seconds × 189_000_000) back to seconds
      const hex = String(child.value);
      const bytes = hexToBytes(hex);
      if (bytes.length === 8) {
        const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
        const raw = view.getBigInt64(0);
        retention_secs = Number(raw / 189_000_000n);
      }
    } else if (child.code === Number(tc.PRIORITY) && child.value != null) {
      priority = Number(child.value);
    }
  }

  if (!mime_pattern) return null;
  return { mime_pattern, max_blob_size, retention_secs, priority };
}

/** Decode a hex-encoded bigint value to a JS number. */
function decodeBigintHex(hex: string): number | undefined {
  try {
    const bytes = hexToBytes(hex);
    // AO bigint encoding: sign in bit 0, value in remaining bits, LSB-first VBC
    // For simple positive values, just parse the encoded VBC
    if (bytes.length === 0) return undefined;
    let value = 0n;
    let shift = 0n;
    for (let i = 0; i < bytes.length; i++) {
      const b = bytes[i];
      if (i === 0) {
        // First byte: bit 0 = sign (0=positive), bits 1-6 = value, bit 7 = continuation
        const magnitude = (b >> 1) & 0x3f;
        value = BigInt(magnitude);
        shift = 6n;
      } else {
        // Subsequent bytes: bits 0-6 = value, bit 7 = continuation
        const magnitude = b & 0x7f;
        value |= BigInt(magnitude) << shift;
        shift += 7n;
      }
      if ((b & 0x80) === 0) break;
    }
    return Number(value);
  } catch {
    return undefined;
  }
}
