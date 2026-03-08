# Multi-Device Wallet Sync — Specification

Specifies how users access the same coin-keys from multiple devices (phone, desktop, shared household wallet). Three layered approaches: local QR transfer (default), paired-device push relay (shared access), and optional cloud vault (backup).

Related specs: [CryptoChoices.md](CryptoChoices.md) (§4–5), [Architecture.md](Architecture.md) (§4).

---

## 1. Problem Statement

AO uses single-use keys: each transaction generates fresh receiver and change keys. This creates three multi-device challenges:

1. **Stale balances.** A device holding a key doesn't know if another device (or attacker) already spent it — until it queries the recorder.
2. **Key proliferation.** New keys created on one device are unknown to other devices.
3. **Spend attribution.** When a device discovers its key was spent, it can't distinguish "my other phone spent it" from "an attacker stole my seed."

### 1.1 Design Principles

- **Recorder is the authority** for UTXO status. Devices always validate held keys against the recorder before proposing transactions.
- **Private keys never transit unencrypted networks.** All seed transfer is either air-gapped (QR/NFC) or end-to-end encrypted (relay).
- **No custodial elements.** No third party ever holds plaintext seeds.
- **Layered adoption.** Approach 3 (QR) works with zero infrastructure. Approach 2 (relay) adds real-time sync. Approach 1 (cloud vault) is optional backup.

---

## 2. UTXO Validation on Connect (All Approaches)

Every device, on connecting to a recorder, MUST validate all held keys before displaying balances.

### 2.1 Validation Flow

```
For each held key K on chain C:
  1. Query GET /chain/{C}/utxo/{K.seq_id}
  2. If response is "unspent" → mark K as valid, display balance
  3. If response is "spent" → mark K as spent locally
     a. If spend was initiated by THIS device → normal (already known)
     b. If spend was initiated by a PAIRED device → show "Spent by [device_label]"
     c. If spend source is UNKNOWN → alert: "Key spent by unknown device — possible compromise"
  4. If response is 404 (no such UTXO) → key may have expired or never been recorded
```

### 2.2 SSE/WebSocket Monitoring

Devices SHOULD subscribe to `GET /chain/{id}/events` (SSE) or `/chain/{id}/ws` (WebSocket) for chains where they hold keys. When a block event arrives containing a spend of a held key, the device updates immediately — no polling needed.

### 2.3 Spend Attribution

To distinguish "my other device spent this" from "attacker spent this," the spending device includes a `device_id` in its local sync state (Approach 2/3). The `device_id` is a random 16-byte identifier generated once per device, stored locally, and shared only with paired devices.

---

## 3. Approach 3: QR/NFC Key Transfer (Default)

Zero infrastructure. Seeds transfer between devices via air-gapped local channel.

### 3.1 Key Sync Payload

When a device acquires new keys (from a transaction), it builds a sync payload:

```json
{
  "v": 1,
  "type": "key_sync",
  "device_id": "<hex 32 chars>",
  "keys": [
    {
      "chain_id": "<hex 64 chars>",
      "public_key": "<hex 64 chars>",
      "encrypted_seed": "<hex 176 chars>",
      "seq_id": 42,
      "amount": "1000000",
      "acquired_at": "<ISO 8601>"
    }
  ],
  "spent": [
    {
      "chain_id": "<hex 64 chars>",
      "public_key": "<hex 64 chars>",
      "seq_id": 17,
      "spent_at": "<ISO 8601>"
    }
  ]
}
```

The `encrypted_seed` uses the same format as CryptoChoices.md §4.4: Argon2id-derived key from the user's wallet passphrase, XChaCha20-Poly1305 encrypted, `salt || nonce || ciphertext` hex-encoded (176 chars = 88 bytes × 2).

### 3.2 QR Transfer Flow

1. **Source device** shows "Sync" badge with count of unsynced keys.
2. User taps "Sync" → device builds sync payload, encrypts with wallet passphrase, encodes as QR.
3. **Target device** scans QR → prompts for wallet passphrase → decrypts seeds → imports keys.
4. Source device marks keys as "synced."

For payloads exceeding a single QR capacity (~2.9 KB binary at M error correction), use **animated QR** (sequence of frames, each containing a numbered fragment). The scanner reassembles fragments before decryption.

### 3.3 NFC Transfer (Optional Enhancement)

Same payload, transferred via NFC NDEF record instead of QR. Suitable for phone-to-phone tap. NFC payload is encrypted identically to QR.

### 3.4 Batch Sync

A "Full Wallet Export" option generates the complete wallet state (all unspent keys across all chains) as a single sync payload. Used for initial device setup or disaster recovery.

### 3.5 Limitations

- Requires physical proximity of devices.
- Delay between acquisition and sync depends on when user scans.
- Does not scale well beyond 2–3 devices.

---

## 4. Approach 2: Paired-Device Push Relay

Real-time sync between paired devices via end-to-end encrypted relay. Designed for shared wallets (e.g., household members accessing the same coins from separate phones and desktops).

### 4.1 Device Pairing

Pairing establishes a shared secret between two devices using X25519 Diffie-Hellman, exchanged via QR scan (one-time ceremony).

#### Pairing Flow

1. **Device A** generates ephemeral X25519 keypair: `(a_priv, a_pub)`.
2. Device A displays QR containing: `{"v":1, "type":"pair", "pub":"<a_pub hex>", "device_id":"<hex>", "label":"Alice's Phone"}`.
3. **Device B** scans QR, generates its own X25519 keypair: `(b_priv, b_pub)`.
4. Device B computes shared secret: `shared = X25519(b_priv, a_pub)`.
5. Device B derives symmetric key: `relay_key = HKDF-SHA256(shared, salt="ao-wallet-sync-v1", info="relay", 32 bytes)`.
6. Device B displays confirmation QR: `{"v":1, "type":"pair_ack", "pub":"<b_pub hex>", "device_id":"<hex>", "label":"Bob's Desktop"}`.
7. **Device A** scans confirmation QR, computes same `shared` and `relay_key`.
8. Both devices store: `{peer_device_id, peer_label, relay_key}` in encrypted local storage.

The X25519 keypairs are ephemeral — discarded after `relay_key` is derived. Only the symmetric `relay_key` persists.

#### Multi-Device Groups

For N devices, each pair must complete the pairing ceremony independently (N-1 pairings from a "primary" device). Each pair has its own `relay_key`. The primary device relays sync messages to all peers.

Alternatively, a **group key** can be established: the first device generates a random 32-byte `group_key`, encrypts it with each pair's `relay_key`, and distributes. All subsequent sync messages use the `group_key`. This reduces per-message encryption from N-1 to 1.

### 4.2 Relay Server

A lightweight message relay that forwards opaque encrypted blobs between paired devices. The relay never sees plaintext.

#### Relay Protocol

- **Transport:** WebSocket (`wss://relay.example/ws/{wallet_id}`).
- **`wallet_id`:** `BLAKE3(group_key)` truncated to 16 bytes, hex-encoded. Identifies the sync group without revealing the key.
- **Message format:**

```json
{
  "from": "<device_id hex>",
  "seq": 42,
  "payload": "<base64 of XChaCha20-Poly1305(group_key, nonce, sync_message_json)>"
}
```

- **Relay behavior:**
  - Accepts WebSocket connections authenticated by `wallet_id`.
  - Forwards messages to all other connected clients with the same `wallet_id`.
  - Retains undelivered messages for up to 72 hours (configurable) for offline devices.
  - Maximum message size: 64 KB.
  - Maximum retained messages per `wallet_id`: 1000.
  - No persistence guarantee — relay is best-effort. Devices fall back to recorder UTXO validation (§2) for consistency.

#### Self-Hosting

The relay is a minimal WebSocket forwarder (< 500 lines). Users can self-host. The PWA allows configuring a custom relay URL in Settings.

#### MQTT Alternative

For deployments already running MQTT (Phase 4), the relay can piggyback on the existing broker:
- Topic: `ao/sync/{wallet_id}`
- QoS 1 (at least once delivery)
- Retained messages for offline devices
- Same encrypted payload format

### 4.3 Sync Messages

Three message types flow through the relay:

#### KEY_ACQUIRED

Sent immediately when a device creates new keys during a transaction.

```json
{
  "type": "key_acquired",
  "device_id": "<hex>",
  "timestamp": "<ISO 8601>",
  "keys": [
    {
      "chain_id": "<hex 64>",
      "public_key": "<hex 64>",
      "encrypted_seed": "<hex 176>",
      "seq_id": 42,
      "amount": "1000000"
    }
  ]
}
```

Seeds are encrypted with the wallet passphrase (CryptoChoices.md §4), then the entire message is encrypted with the `group_key` for relay transport. Double encryption: inner layer protects the seed at rest, outer layer protects in transit.

#### KEY_SPENT

Sent immediately when a device spends a key.

```json
{
  "type": "key_spent",
  "device_id": "<hex>",
  "timestamp": "<ISO 8601>",
  "spent": [
    {
      "chain_id": "<hex 64>",
      "public_key": "<hex 64>",
      "seq_id": 17,
      "block_height": 42
    }
  ]
}
```

#### HEARTBEAT

Sent every 5 minutes (configurable). Confirms the device is online and its sync state is current.

```json
{
  "type": "heartbeat",
  "device_id": "<hex>",
  "timestamp": "<ISO 8601>",
  "key_count": 12,
  "latest_seq": 45
}
```

### 4.4 Conflict Resolution

- **UTXO status conflicts:** The recorder is authoritative. If the relay says "unspent" but the recorder says "spent," the recorder wins. Devices re-validate against the recorder on every connect (§2).
- **Duplicate KEY_ACQUIRED:** Idempotent — if a device already has the key, it ignores the duplicate.
- **Missing messages:** If a device was offline and missed relay messages, it discovers the true state by querying the recorder's UTXO endpoints and SSE stream on reconnect.

### 4.5 Security Properties

- **Relay learns nothing:** All payloads are encrypted with the `group_key`. The relay sees only `wallet_id` (a hash) and opaque ciphertext.
- **Compromised relay:** Cannot forge messages (AEAD authentication). Cannot read seeds (double-encrypted). Can only deny service (withhold messages) — mitigated by recorder validation fallback.
- **Lost device:** Unpair by rotating the `group_key` on remaining devices. Lost device cannot sync further. Seeds on lost device are protected by wallet passphrase encryption.
- **Replay attacks:** Sequence numbers (`seq`) per device, tracked by peers. Replayed messages with already-seen `seq` are ignored.

---

## 5. Approach 1: Encrypted Cloud Vault (Optional)

Optional cloud backup for users who prefer passphrase-based recovery over QR ceremonies. Higher convenience, higher trust surface.

### 5.1 Vault Structure

The vault is a single encrypted blob stored at a cloud endpoint (S3-compatible, WebDAV, or purpose-built service).

```
vault_key  = Argon2id(vault_passphrase, vault_salt, memory=64MB, iterations=3, parallelism=1, output=32 bytes)
vault_blob = XChaCha20-Poly1305_Encrypt(vault_key, nonce, wallet_state_json)
stored     = vault_salt(16) || nonce(24) || vault_blob
```

The `wallet_state_json` is the same format as the Approach 3 full wallet export (§3.4).

### 5.2 Sync Flow

1. On key acquisition or spend, device re-encrypts the full wallet state and uploads.
2. Other devices poll the vault endpoint (or receive a push notification) and download.
3. On download, device decrypts with `vault_passphrase`, merges with local state, validates against recorder.

### 5.3 Merge Strategy

- **New keys** (in vault but not local): Import.
- **Spent keys** (local says unspent, vault says spent): Accept spend, validate against recorder.
- **Conflicting state:** Recorder is authoritative. Query UTXO endpoints to resolve.

### 5.4 Risk Considerations

- **Vault passphrase = single point of compromise.** If leaked, all seeds across all chains are exposed.
- **Cloud availability.** If the vault service is down, devices fall back to Approach 3 (QR) or Approach 2 (relay).
- **Not recommended as sole backup.** Users should maintain QR-based or file-based backups alongside the vault.

### 5.5 Implementation Priority

Approach 1 is **deferred** — documented here for completeness. Approaches 2 and 3 cover the primary use cases without introducing cloud dependencies.

---

## 6. Wallet State Model

All approaches share a common local wallet state structure.

### 6.1 WalletState

```typescript
interface WalletState {
  version: 1;
  device_id: string;          // Random 16 bytes, hex-encoded, generated once
  device_label: string;       // User-chosen name ("Alice's Phone")
  keys: KeyEntry[];
  peers: PeerDevice[];        // Paired devices (Approach 2)
  relay_url?: string;         // Custom relay URL (Approach 2)
  vault_url?: string;         // Cloud vault URL (Approach 1)
}

interface KeyEntry {
  chain_id: string;           // Hex 64 chars
  public_key: string;         // Hex 64 chars
  encrypted_seed: string;     // Hex 176 chars (salt || nonce || ciphertext)
  seq_id: number | null;      // Assigned by recorder after recording; null if pre-recording
  amount: string | null;      // Share amount as decimal string; null if unknown
  status: 'unspent' | 'spent' | 'expired' | 'unconfirmed';
  acquired_at: string;        // ISO 8601
  acquired_by: string;        // device_id that created this key
  synced: boolean;            // Has this key been synced to all peers?
  spent_by?: string;          // device_id that spent this key (if known)
  spent_at?: string;          // ISO 8601
}

interface PeerDevice {
  device_id: string;
  label: string;
  relay_key: string;          // Hex 64 chars (shared symmetric key)
  paired_at: string;          // ISO 8601
  last_seen?: string;         // ISO 8601 (from heartbeat)
}
```

### 6.2 Storage

- **Browser:** IndexedDB database `ao-wallet`, object stores `keys`, `peers`, `config`.
- **CLI:** Encrypted JSON file at `~/.ao/wallet.json` (same Argon2id + XChaCha20-Poly1305 as §4.1).
- **Migration from current MVP:** Current `localStorage` wallet (`ao_wallet_seed`, `ao_wallet_pubkey`, `ao_wallet_label`) is imported into the new IndexedDB store on first load, then `localStorage` entries are removed.

---

## 7. User Alerts

### 7.1 Unknown Spend Alert

When UTXO validation (§2) discovers a key was spent and the `spent_by` device is not in the peer list:

> **Warning: Coins spent by unknown device**
>
> {amount} coins on chain {symbol} (seq #{seq_id}) were spent at {time}.
> This spend was not made by any of your paired devices.
>
> If you did not authorize this, your key may be compromised.
> Consider transferring remaining coins to fresh keys immediately.

### 7.2 Sync Pending Badge

The header shows a badge when unsynced keys exist:

- **QR mode (Approach 3):** "Sync: 3 keys" — tapping opens the QR sync screen.
- **Relay mode (Approach 2):** Badge appears only when the relay is unreachable and keys are queued.

### 7.3 Peer Device Status

In Settings, paired devices show connection status:

- **Online** (heartbeat received within 10 minutes)
- **Offline** (no recent heartbeat)
- **Unpaired** (removed from peer list)

---

## 8. Compatibility

### 8.1 Backward Compatibility

- Devices that don't support sync continue to work as today — single-device wallets.
- The sync payload format is versioned (`"v": 1`). Future versions can extend without breaking.
- Sync is opt-in. No behavioral changes for users who don't pair devices.

### 8.2 Cross-Platform

- **Phone ↔ Phone:** QR scan (Approach 3) or relay (Approach 2).
- **Phone ↔ Desktop:** QR scan via webcam, or relay.
- **Desktop ↔ Desktop:** Relay (no camera for QR). File-based sync payload as fallback.
- **CLI ↔ PWA:** CLI exports sync payload as JSON file. PWA imports via file picker.

---

## 9. Non-Goals

- **Seed phrase / mnemonic recovery.** Incompatible with independently-random single-use keys (CryptoChoices.md §5.3).
- **Hardware wallet integration.** Out of scope for this specification.
- **Automatic cloud backup without user opt-in.** Users must explicitly enable Approach 1.
- **Multi-recorder wallet discovery.** Wallet state tracks which recorder holds each chain. Discovery of new chains remains via QR scan (N3).
