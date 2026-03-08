// WebSocket relay client for paired-device push sync.
// Spec: specs/WalletSync.md §4.2–4.4
//
// Connects to a relay server, sends/receives encrypted sync messages,
// and maintains heartbeat. All messages are end-to-end encrypted with
// the group relay key — the relay sees only opaque ciphertext.

import { encryptForRelay, decryptFromRelay, deriveWalletId } from './pairing.ts';
import { getDeviceId, getUnspentKeys } from './walletDb.ts';
import type { SyncPayload } from './walletSync.ts';
import { importSyncPayload } from './walletSync.ts';

// ── Relay Message Types ──────────────────────────────────────────────

interface RelayMessage {
  from: string;     // device_id
  seq: number;
  payload: string;  // base64 encrypted blob
}

type SyncMessageType = 'key_acquired' | 'key_spent' | 'heartbeat';

interface SyncMessage {
  type: SyncMessageType;
  deviceId: string;
  timestamp: string;
  // For key_acquired/key_spent, wraps a SyncPayload
  syncPayload?: SyncPayload;
  // For heartbeat
  keyCount?: number;
}

const VALID_SYNC_TYPES = new Set<string>(['key_acquired', 'key_spent', 'heartbeat']);

/** Validate a parsed wire-format object as a SyncMessage (accepts snake_case). */
function validateSyncMessage(obj: unknown): obj is SyncMessage {
  if (typeof obj !== 'object' || obj === null) return false;
  const o = obj as Record<string, unknown>;
  // Accept both snake_case (spec) and camelCase (legacy)
  const type = o.type;
  const deviceId = o.device_id ?? o.deviceId;
  const timestamp = o.timestamp;
  return (
    typeof type === 'string' && VALID_SYNC_TYPES.has(type) &&
    typeof deviceId === 'string' &&
    typeof timestamp === 'string'
  );
}

/** Convert wire-format (snake_case) sync message to internal SyncMessage. */
function fromWireSyncMessage(obj: Record<string, unknown>): SyncMessage {
  return {
    type: obj.type as SyncMessageType,
    deviceId: (obj.device_id ?? obj.deviceId) as string,
    timestamp: obj.timestamp as string,
    syncPayload: (obj.sync_payload ?? obj.syncPayload) as SyncPayload | undefined,
    keyCount: (obj.key_count ?? obj.keyCount) as number | undefined,
  };
}

/** Convert internal SyncMessage to spec-compliant snake_case wire format. */
function toWireSyncMessage(msg: SyncMessage): Record<string, unknown> {
  const wire: Record<string, unknown> = {
    type: msg.type,
    device_id: msg.deviceId,
    timestamp: msg.timestamp,
  };
  if (msg.syncPayload !== undefined) wire.sync_payload = msg.syncPayload;
  if (msg.keyCount !== undefined) wire.key_count = msg.keyCount;
  return wire;
}

// Max seq before wrapping (well within safe integer range)
const MAX_SEQ = 2 ** 48;

// ── Relay Client ─────────────────────────────────────────────────────

export interface RelayClientOptions {
  relayUrl: string;
  relayKeyHex: string;
  /** Wallet passphrase for encrypting/decrypting seeds in sync payloads. */
  getPassphrase?: () => string | null;
  onKeysReceived?: (imported: number, spentMarked: number, fromDevice: string) => void;
  onHeartbeat?: (deviceId: string, keyCount: number) => void;
  onUnknownSpend?: (deviceId: string) => void;
  onConnectionChange?: (connected: boolean) => void;
}

export class RelayClient {
  private ws: WebSocket | null = null;
  private seq = 0;
  private seenSeqs = new Map<string, number>(); // deviceId → highest seen seq
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectDelay = 1000;
  private closed = false;
  private walletId: string | null = null;

  constructor(private readonly opts: RelayClientOptions) {}

  /** Connect to the relay. Automatically reconnects on disconnect. */
  async connect(): Promise<void> {
    // Clean up any existing connection before reconnecting
    if (this.ws) {
      this.ws.onopen = null;
      this.ws.onmessage = null;
      this.ws.onclose = null;
      this.ws.onerror = null;
      this.ws.close();
      this.ws = null;
    }

    this.closed = false;
    this.walletId = await deriveWalletId(this.opts.relayKeyHex);

    const url = `${this.opts.relayUrl}/ws/${this.walletId}`;
    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this.reconnectDelay = 1000; // reset backoff
      this.opts.onConnectionChange?.(true);
      this.startHeartbeat();
    };

    this.ws.onmessage = async (event) => {
      try {
        const data = typeof event.data === 'string' ? event.data : '';
        const msg: RelayMessage = JSON.parse(data);
        if (typeof msg.from !== 'string' || typeof msg.seq !== 'number' || typeof msg.payload !== 'string') {
          return; // malformed relay message
        }
        await this.handleMessage(msg);
      } catch (e) {
        console.warn('[ao-relay] message handling error:', e);
      }
    };

    this.ws.onclose = () => {
      this.opts.onConnectionChange?.(false);
      this.stopHeartbeat();
      if (!this.closed) {
        this.scheduleReconnect();
      }
    };

    this.ws.onerror = () => {
      // onclose will fire after onerror
    };
  }

  /** Disconnect from the relay. */
  disconnect(): void {
    this.closed = true;
    this.stopHeartbeat();
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  /** Send a sync payload (key_acquired or key_spent) through the relay. */
  async sendSync(payload: SyncPayload): Promise<void> {
    const deviceId = await getDeviceId();
    const syncMsg: SyncMessage = {
      type: payload.keys.length > 0 ? 'key_acquired' : 'key_spent',
      deviceId,
      timestamp: new Date().toISOString(),
      syncPayload: payload,
    };
    await this.sendEncrypted(syncMsg);
  }

  // ── Internal ───────────────────────────────────────────────────────

  private async handleMessage(msg: RelayMessage): Promise<void> {
    // Replay protection: ignore already-seen or old sequences
    const lastSeen = this.seenSeqs.get(msg.from) ?? -1;
    if (msg.seq <= lastSeen) return;
    this.seenSeqs.set(msg.from, msg.seq);

    // Ignore our own messages
    const myDeviceId = await getDeviceId();
    if (msg.from === myDeviceId) return;

    // Decrypt
    let plaintext: string;
    try {
      plaintext = await decryptFromRelay(this.opts.relayKeyHex, msg.payload);
    } catch {
      return; // Decryption failure — wrong key or tampered
    }

    // Parse and validate the decrypted sync message
    let parsed: unknown;
    try {
      parsed = JSON.parse(plaintext);
    } catch {
      console.warn('[ao-relay] invalid JSON in decrypted message');
      return;
    }
    if (!validateSyncMessage(parsed)) {
      console.warn('[ao-relay] invalid sync message structure');
      return;
    }
    const syncMsg = fromWireSyncMessage(parsed as Record<string, unknown>);

    if (syncMsg.type === 'heartbeat') {
      this.opts.onHeartbeat?.(syncMsg.deviceId, syncMsg.keyCount ?? 0);
      return;
    }

    // Import keys/spends from the sync payload
    if (syncMsg.syncPayload) {
      const passphrase = this.opts.getPassphrase?.() ?? null;
      const result = await importSyncPayload(syncMsg.syncPayload, passphrase);
      if (result.imported > 0 || result.spentMarked > 0) {
        this.opts.onKeysReceived?.(result.imported, result.spentMarked, syncMsg.deviceId);
      }
    }
  }

  private async sendEncrypted(syncMsg: SyncMessage): Promise<void> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;

    const deviceId = await getDeviceId();
    const plaintext = JSON.stringify(toWireSyncMessage(syncMsg));
    const encrypted = await encryptForRelay(this.opts.relayKeyHex, plaintext);

    // Wrap seq at MAX_SEQ to stay within safe integer range
    if (this.seq >= MAX_SEQ) this.seq = 0;

    const relayMsg: RelayMessage = {
      from: deviceId,
      seq: this.seq++,
      payload: encrypted,
    };

    this.ws.send(JSON.stringify(relayMsg));
  }

  private startHeartbeat(): void {
    this.stopHeartbeat();
    // Send heartbeat every 5 minutes.
    // sendEncrypted already checks readyState, so the immediate call is safe.
    const sendHeartbeat = async () => {
      const deviceId = await getDeviceId();
      const msg: SyncMessage = {
        type: 'heartbeat',
        deviceId,
        timestamp: new Date().toISOString(),
        keyCount: (await getUnspentKeys()).length,
      };
      await this.sendEncrypted(msg);
    };
    sendHeartbeat();
    this.heartbeatTimer = setInterval(sendHeartbeat, 5 * 60 * 1000);
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  private scheduleReconnect(): void {
    this.reconnectTimer = setTimeout(() => {
      this.connect();
    }, this.reconnectDelay);
    // Exponential backoff: 1s, 2s, 4s, 8s, ... max 30s
    this.reconnectDelay = Math.min(this.reconnectDelay * 2, 30_000);
  }
}
