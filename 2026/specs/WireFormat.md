# Wire Format Specification — Deliverable 0B

Byte-level specification of the on-chain binary format. Designed for minimal message size — protocol messages must be viable over low-bandwidth transports such as [Meshtastic](https://meshtastic.org/) LoRa mesh networks.

Related specs: [Architecture.md](Architecture.md) (0A), [CryptoChoices.md](CryptoChoices.md) (0C), [EconomicRules.md](EconomicRules.md) (0D).

---

## 1. Variable Byte Coding (VBC)

VBC encodes integers in 1–10 bytes. Two variants: unsigned (for sizes and counts) and signed (for type codes and signed values).

### 1.1 Unsigned VBC

Each byte carries 7 data bits (bits 0–6) and a continuation flag (bit 7). Bytes are **LSB-first**: the first byte holds the least significant 7 bits.

```
Byte layout:  [cont:7] [d6:6] [d5:5] [d4:4] [d3:3] [d2:2] [d1:1] [d0:0]
```

**Encoding** (value → bytes):
1. Emit low 7 bits as a byte. If remaining value > 0, set bit 7 (continuation).
2. Shift value right 7 bits. Repeat until value = 0.

**Decoding** (bytes → value):
1. For each byte, OR bits 0–6 into the result at the current bit offset.
2. Advance bit offset by 7. If bit 7 is clear, stop.

Maximum: 10 bytes → 70 data bits. Values up to 2⁶³−1 are valid (we reject values ≥ 2⁶³ to keep within i64 range).

**Why 10 bytes:** VBC targets 64-bit signed integers — sufficient for type codes, byte-count sizes, sequence IDs, and list counts. Unsigned VBC in 10 bytes carries 70 data bits; signed VBC carries 6 magnitude bits in the first byte plus 9×7 = 63 in continuation bytes, for 69 magnitude bits total — covering the full i64 range. Values exceeding 64 bits (share amounts, coin counts) use BigInt encoding (§4) instead, which is unbounded. The 10-byte cap also guarantees bounded reads — a decoder never consumes more than 10 bytes per VBC field.

### 1.2 Signed VBC

Signed VBC encodes a sign+magnitude pair. The mapping from signed integer to unsigned wire value is:

- `n ≥ 0` → `wire = n << 1` (bit 0 = 0)
- `n < 0` → `wire = ((-n) << 1) | 1` (bit 0 = 1)

The wire value is then encoded as unsigned VBC. In the first byte, bit 0 is the sign bit and bits 1–6 carry the 6 least significant magnitude bits. Continuation bytes carry 7 magnitude bits each.

**Range:** −(2⁶³−1) to +(2⁶³−1). Wire value 1 (negative zero) is **invalid**.

**Note:** The 2018 C++ reference (`riceyCodes.cpp`) used MSB-first unsigned VBC for type codes only and never implemented signed encoding. The 2026 format is a clean break: LSB-first with sign support.

### 1.3 VBC Test Vectors

| Signed Value | Unsigned Wire | Hex Bytes | Notes |
|---:|---:|:---|:---|
| 0 | 0 | `00` | Zero |
| 1 | 2 | `02` | Smallest positive |
| −1 | 3 | `03` | Smallest negative |
| 2 | 4 | `04` | |
| −2 | 5 | `05` | |
| 31 | 62 | `3e` | |
| −31 | 63 | `3f` | |
| 32 | 64 | `40` | |
| −32 | 65 | `41` | |
| 63 | 126 | `7e` | Max single-byte positive |
| −63 | 127 | `7f` | Max single-byte negative |
| 64 | 128 | `80 01` | First 2-byte positive |
| −64 | 129 | `81 01` | First 2-byte negative |
| −65 | 131 | `83 01` | |
| 127 | 254 | `fe 01` | |
| 128 | 256 | `80 02` | |
| −128 | 257 | `81 02` | |
| 8191 | 16382 | `fe 7f` | Max 2-byte positive |
| −8191 | 16383 | `ff 7f` | Max 2-byte negative |
| 8192 | 16384 | `80 80 01` | First 3-byte positive |
| −8192 | 16385 | `81 80 01` | First 3-byte negative |
| 1000000 | 2000000 | `80 89 7a` | 3 bytes |
| −1000000 | 2000001 | `81 89 7a` | 3 bytes |

**Unsigned VBC test vectors** (for sizes and byte counts):

| Value | Hex Bytes | Notes |
|---:|:---|:---|
| 0 | `00` | |
| 1 | `01` | |
| 127 | `7f` | Max single-byte |
| 128 | `80 01` | First 2-byte |
| 255 | `ff 01` | |
| 256 | `80 02` | |
| 16383 | `ff 7f` | Max 2-byte |
| 16384 | `80 80 01` | First 3-byte |

---

## 2. DataItem Structure

Every piece of data on-chain is a **DataItem**: a type code followed by data.

```
DataItem = TypeCode + [Size] + Data
```

- **TypeCode:** Signed VBC identifying the data type.
- **Size:** Unsigned VBC byte count of Data. Present only for variable-size items.
- **Data:** The item's payload. Format determined by TypeCode.

### 2.1 Size Categories

| Category | Size Field | Data |
|----------|-----------|------|
| **Fixed** | Absent | Exactly N bytes (N known from type code) |
| **Variable** | Unsigned VBC | Arbitrary bytes, length from Size field |
| **VBC-value** | Absent | Data is itself a VBC (signed or unsigned), self-delimiting |
| **Container** | Unsigned VBC | Contains zero or more child DataItems |

### 2.2 Byte Order

- **VBC fields:** LSB-first (as defined in §1).
- **Timestamps:** 8 bytes, big-endian.
- **Big integers:** Big-endian magnitude (see §4).
- **Hashes and keys:** Raw bytes, no byte-order conversion.

---

## 3. Type Code Registry

Type codes are signed VBC integers. The sign distinguishes code variants (not data polarity). **Separability rule: a type code's item is separable if and only if |code| has bit 5 set** (`|code| & 0x20 ≠ 0`).

### 3.1 Core Types (A1 — Inseparable, |code| 1–31)

| Code | Name | Size | Description |
|---:|:---|:---|:---|
| 1 | `ED25519_PUB` | 32 | Ed25519 public key |
| 2 | `ED25519_SIG` | 64 | Ed25519 signature |
| 3 | `SHA256` | 32 | SHA2-256 hash |
| 4 | `BLAKE3` | 32 | BLAKE3 hash |
| 5 | `TIMESTAMP` | 8 | Timestamp (see §5) |
| 6 | `AMOUNT` | variable | Share amount (big integer, see §4) |
| 7 | `SEQ_ID` | vbc-value | Sequence ID (unsigned VBC) |
| 8 | `ASSIGNMENT` | container | Assignment agreement |
| 9 | `AUTHORIZATION` | container | Signed authorization (assignment + sigs) |
| 10 | `PARTICIPANT` | container | Participant: key + amount + index |
| 11 | `BLOCK` | container | Complete block: signed block + hash |
| 12 | `BLOCK_SIGNED` | container | Block signature + pubkey + contents |
| 13 | `BLOCK_CONTENTS` | container | Pages + chain housekeeping |
| 14 | `PAGE` | container | One assignment within a block |
| 15 | `GENESIS` | container | Genesis block |
| 16 | `RECORDING_BID` | variable | Per-byte fee bid (rational, see §4.2) |
| 17 | `DEADLINE` | 8 | Recording deadline timestamp |
| 18 | `COIN_COUNT` | variable | Display coin count (big integer) |
| 19 | `FEE_RATE` | variable | Fee rate (rational) |
| 20 | `EXPIRY_PERIOD` | 8 | Expiration period (timestamp delta) |
| 21 | `CHAIN_SYMBOL` | variable | Short text label (UTF-8) |
| 22 | `PROTOCOL_VER` | vbc-value | Protocol version (unsigned VBC) |
| 23 | `SHARES_OUT` | variable | Total shares outstanding (big integer) |
| 24 | `PREV_HASH` | 32 | SHA2-256 hash of previous signed block |
| 25 | `FIRST_SEQ` | vbc-value | First sequence ID assigned in this block |
| 26 | `SEQ_COUNT` | vbc-value | Number of new sequence IDs in block |
| 27 | `LIST_SIZE` | vbc-value | Count of items in a list |
| 28 | `REFUTATION` | container | Explicit refutation of a prior agreement |
| 29 | `PAGE_INDEX` | vbc-value | Page number within block (0-based) |
| 30 | `AUTH_SIG` | container | Signature container (children vary by context, see §6) |
| −1 | `EXPIRY_MODE` | vbc-value | Expiration mode (1=hard cutoff, 2=age tax) |
| −2 | `TAX_PARAMS` | container | Age tax parameters (start age, doubling period) |

### 3.2 Separable Types (|code| 32–63)

These items can be stripped from blocks and replaced with their SHA2-256 hash without invalidating any signature.

| Code | Name | Size | Description |
|---:|:---|:---|:---|
| 32 | `NOTE` | variable | Free-form text (UTF-8) |
| 33 | `DATA_BLOB` | variable | Arbitrary byte data |
| 34 | `DESCRIPTION` | variable | Chain description (UTF-8) |
| 35 | `ICON` | variable | Chain icon (image bytes) |
| 36 | `VENDOR_PROFILE` | container | Vendor metadata (name, location, etc.) |
| 37 | `EXCHANGE_LISTING` | container | Exchange agent listing (Phase 4) |
| 38 | `CREDENTIAL_REF` | container | Credential reference: URL + content hash (Phase 5, see [ValidationAndTrust.md](ValidationAndTrust.md) §5) |
| 39 | `CREDENTIAL_URL` | variable | URL of credential document (UTF-8) |

### 3.3 Validator Types (Inseparable, |code| 64–68)

Second inseparable band. See [ValidationAndTrust.md](ValidationAndTrust.md) §4.1.

| Code | Name | Size | Description |
|---:|:---|:---|:---|
| 64 | `VALIDATOR_ATTESTATION` | container | Validator attestation (children: height, hash, anchor ref, timestamp) |
| 65 | `VALIDATED_HEIGHT` | vbc-value | Block height at validation time |
| 66 | `ROLLED_HASH` | 32 | SHA2-256 rolled hash at validated height |
| 67 | `ANCHOR_REF` | variable | External anchor reference string (UTF-8) |
| 68 | `ANCHOR_TIMESTAMP` | 8 | Anchor publication timestamp |

### 3.4 Separability Rule

To determine if a DataItem is separable, check bit 5 of the type code's absolute value:

```rust
fn is_separable(type_code: i64) -> bool {
    (type_code.unsigned_abs() & 0x20) != 0
}
```

Before signing, walk the DataItem tree and replace each separable item with a `SHA256` item (code 3) containing its SHA2-256 hash. The hash is computed over the separable item's **complete encoding** (type code + size + data).

**Note on code ranges:** The bit 5 rule creates alternating 32-wide bands: |codes| 1–31 inseparable, 32–63 separable, 64–95 inseparable, 96–127 separable, etc. All A1 type codes fall in the 1–63 range. Future type codes must be assigned with this pattern in mind.

---

## 4. Big Integer and Rational Encoding

### 4.1 Big Integer

Used for share amounts, coin counts, and shares outstanding. Encoded as a VBC byte count followed by a two's-complement big-endian representation.

```
BigInt = ByteCount(unsigned VBC) + TwosComplement(big-endian bytes)
```

**Rules:**
- **ByteCount:** Number of bytes that follow. Zero means value is 0.
- **Encoding:** Standard two's-complement, big-endian, minimum bytes. Identical to Rust `num_bigint::BigInt::to_signed_bytes_be()` / Java `BigInteger.toByteArray()`.
- **Zero:** ByteCount = 0, no data bytes.
- **Positive values** with MSB of first byte ≥ 0x80 must be prefixed with `0x00` (sign extension).
- **Negative values** are two's complement. MSB of first byte is always ≥ 0x80.
- **No redundant leading bytes:** no `0x00` prefix unless needed for sign, no `0xFF` prefix unless needed for sign.

### 4.2 Big Integer Test Vectors

| Value | ByteCount VBC | Bytes Hex | Full Encoding |
|---:|:---|:---|:---|
| 0 | `00` | *(empty)* | `00` |
| 1 | `01` | `01` | `01 01` |
| −1 | `01` | `ff` | `01 ff` |
| 127 | `01` | `7f` | `01 7f` |
| −127 | `01` | `81` | `01 81` |
| 128 | `02` | `00 80` | `02 00 80` |
| −128 | `01` | `80` | `01 80` |
| 255 | `02` | `00 ff` | `02 00 ff` |
| −255 | `02` | `ff 01` | `02 ff 01` |
| 256 | `02` | `01 00` | `02 01 00` |
| −256 | `02` | `ff 00` | `02 ff 00` |
| 2⁶⁴ | `09` | `01 00 00 00 00 00 00 00 00` | `09 01 00 00 00 00 00 00 00 00` |
| 2⁸⁶ | `0b` | `40 00 00 00 00 00 00 00 00 00 00` | `0b 40 00 00 00 00 00 00 00 00 00 00` |

*(2⁸⁶ = 77,371,252,455,336,267,181,195,264 — typical genesis share pool)*

### 4.3 Rational Fraction

Used for fee rates and tax fractions. Encoded as a nested structure:

```
Rational = TotalSize(unsigned VBC) + NumSize(unsigned VBC) + Numerator(big-endian) + Denominator(big-endian)
```

- **TotalSize:** Byte count of everything after this field (NumSize + numerator + denominator).
- **NumSize:** Byte count of the numerator magnitude.
- **Numerator:** Big-endian, sign in MSB (same as BigInt magnitude).
- **Denominator:** Two's-complement big-endian, **must be positive** (MSB < 0x80). Zero denominator is an error.
- Denominator size = TotalSize − NumSize VBC length − NumSize value.

### 4.4 Rational Test Vectors

| Value | NumSize | Num Hex | Denom Hex | Full Encoding |
|:---|:---|:---|:---|:---|
| 1/2 | `01` | `01` | `02` | `03 01 01 02` |
| −3/7 | `01` | `fd` | `07` | `03 01 fd 07` |
| 1/1000000 | `01` | `01` | `0f 42 40` | `05 01 01 0f 42 40` |

---

## 5. Timestamp Encoding

Timestamps encode UTC time as `Unix_seconds × 189,000,000` in an **8-byte big-endian signed integer** (`i64`).

The multiplier 189,000,000 = 2⁶ × 3³ × 5⁶ × 7 provides ~5.29 ns resolution and clean division by any integer 2–10.

**Signed integer rationale:** Signed `i64` supports pre-epoch dates (birthdates, historical events) and overflows only at ~year 3517 (`i64::MAX / 189,000,000 ≈ 48.8 billion` Unix seconds). This is well beyond the project's **2126 design horizon** — all implementations must correctly handle timestamps through at least 2126-12-31.

### 5.1 Timestamp Test Vectors

| Description | UTC | Unix Seconds | AO Timestamp | Hex (8 bytes BE) |
|:---|:---|---:|---:|:---|
| Epoch | 1970-01-01T00:00:00Z | 0 | 0 | `00 00 00 00 00 00 00 00` |
| 2026-01-01 | 2026-01-01T00:00:00Z | 1767225600 | 334005638400000000 | `04 a2 a0 5b c5 cf 40 00` |
| 2026-03-06 | 2026-03-06T00:00:00Z | 1772611200 | 335023516800000000 | `04 a6 3e 1d 0e 44 20 00` |

### 5.2 Monotonicity

Within a single actor (identified by signing key), timestamps must be **strictly increasing**. If the wall clock has not advanced since the last signature, bump the timestamp by 1 (~5.29 ns). The Recorder rejects any signature whose timestamp does not exceed that key's most recent recorded timestamp.

---

## 6. Block Structure (A1)

A complete block is a nested DataItem tree:

```
BLOCK (11)
├── BLOCK_SIGNED (12)
│   ├── BLOCK_CONTENTS (13)
│   │   ├── PREV_HASH (24): SHA2-256 of parent's BLOCK_SIGNED encoding
│   │   ├── FIRST_SEQ (25): first sequence ID assigned in this block
│   │   ├── SEQ_COUNT (26): number of new sequence IDs
│   │   ├── LIST_SIZE (27): number of pages
│   │   ├── SHARES_OUT (23): total shares after this block
│   │   └── PAGE (14) [repeated per assignment]
│   │       ├── PAGE_INDEX (29): 0-based index
│   │       └── AUTHORIZATION (9)
│   │           ├── ASSIGNMENT (8)
│   │           │   ├── LIST_SIZE (27): givers + receivers count
│   │           │   ├── PARTICIPANT (10) [per giver]
│   │           │   │   ├── SEQ_ID (7): source sequence ID
│   │           │   │   └── AMOUNT (6): shares given
│   │           │   ├── PARTICIPANT (10) [per receiver]
│   │           │   │   ├── ED25519_PUB (1): new public key
│   │           │   │   └── AMOUNT (6): shares received
│   │           │   ├── RECORDING_BID (16): per-byte fee
│   │           │   ├── DEADLINE (17): recording deadline
│   │           │   └── [separable items, if any]
│   │           └── AUTH_SIG (30) [per participant]
│   │               ├── ED25519_SIG (2): 64-byte signature
│   │               ├── TIMESTAMP (5): time of signing
│   │               └── PAGE_INDEX (29): participant index
│   └── AUTH_SIG (30) [block maker signature]
│       ├── ED25519_SIG (2)
│       ├── TIMESTAMP (5)
│       └── ED25519_PUB (1): block maker's public key
└── SHA256 (3): hash of BLOCK_SIGNED encoding
```

**AUTH_SIG children vary by context:** Within AUTHORIZATION, each `AUTH_SIG` contains `ED25519_SIG` + `TIMESTAMP` + `PAGE_INDEX` (identifying which participant signed). The block maker's `AUTH_SIG` contains `ED25519_SIG` + `TIMESTAMP` + `ED25519_PUB` (the block maker's public key). Implementations should dispatch on parent container type to determine expected children.

### 6.1 Genesis Block

The genesis block uses type code `GENESIS` (15) instead of `BLOCK` (11) and has no `PREV_HASH`. Its contents establish the chain:

```
GENESIS (15)
├── PROTOCOL_VER (22): 1 (for A1)
├── CHAIN_SYMBOL (21): e.g., "BCG"
├── DESCRIPTION (34): chain description [separable]
├── ICON (35): chain icon [separable]
├── COIN_COUNT (18): display coins (e.g., 10,000,000,000)
├── SHARES_OUT (23): initial shares (e.g., ~2⁸⁶)
├── FEE_RATE (19): recording fee rate (rational)
├── EXPIRY_PERIOD (20): share expiration period
├── EXPIRY_MODE (-1): hard cutoff (1) or age tax (2)
├── TAX_PARAMS (-2) [if EXPIRY_MODE = 2]
│   ├── TIMESTAMP (5): TAX_START_AGE (timestamp delta)
│   └── TIMESTAMP (5): TAX_DOUBLING_PERIOD (timestamp delta)
├── PARTICIPANT (10) [issuer — receives all initial shares]
│   ├── ED25519_PUB (1): issuer's public key
│   └── AMOUNT (6): = SHARES_OUT
├── AUTH_SIG (30) [issuer's signature]
│   ├── ED25519_SIG (2)
│   └── TIMESTAMP (5)
└── SHA256 (3): hash of all preceding genesis content
```

The trailing `SHA256` is computed over the complete encoding of all items within the `GENESIS` container **except** the `SHA256` item itself. The **chain ID** is this hash value — equivalently, the SHA2-256 hash of the genesis block encoding with the trailing hash item excluded.

### 6.2 Signature Construction

What gets signed by each participant:

1. Serialize the `ASSIGNMENT` container to bytes.
2. Walk the serialized tree: replace each separable DataItem with `SHA256(3)` containing the SHA2-256 hash of that item's complete encoding (type code + size + data).
3. Compute `digest = SHA2-256(substituted_bytes)`.
4. Append the 8-byte big-endian signing timestamp to the digest: `signed_data = digest || timestamp`.
5. Sign `signed_data` with Ed25519: `signature = Ed25519_Sign(private_key, signed_data)`.

The block maker signs `BLOCK_CONTENTS` (after all pages are finalized) using the same process: serialize, substitute separable items, hash, append timestamp, sign.

### 6.3 Minimal Assignment Size Estimate

A 1-giver, 1-receiver assignment with no separable items:

| Component | Bytes |
|:---|---:|
| ASSIGNMENT container overhead | ~4 |
| LIST_SIZE (2 participants) | 1 |
| Giver: SEQ_ID + AMOUNT | ~6 |
| Receiver: ED25519_PUB + AMOUNT | ~38 |
| RECORDING_BID | ~5 |
| DEADLINE | 10 |
| 2× AUTH_SIG (sig + timestamp + index) | ~150 |
| **Total** | **~214 bytes** |

Well under the 256-byte target from [Architecture.md](Architecture.md) §6 principle #11.

---

## 7. JSON Representation

For debugging and web clients, all DataItems have a canonical JSON form:

```json
{
  "type": "ASSIGNMENT",
  "code": 8,
  "items": [
    { "type": "LIST_SIZE", "code": 27, "value": 2 },
    { "type": "PARTICIPANT", "code": 10, "items": [
      { "type": "SEQ_ID", "code": 7, "value": 42 },
      { "type": "AMOUNT", "code": 6, "value": "77371252455336267181195264" }
    ]},
    ...
  ]
}
```

Rules:
- Type name and numeric code are both present.
- Fixed-size items use `"value"` (hex string for bytes, decimal string for big integers).
- Containers use `"items"` array preserving child order.
- VBC-value items use `"value"` (decimal integer).
- JSON ↔ binary round-trip must produce identical bytes.

---

## 8. Divergence from 2018 C++ Code

| Aspect | 2018 C++ | 2026 Spec |
|:---|:---|:---|
| VBC byte order | MSB-first (prepend) | LSB-first (append) |
| VBC sign encoding | Not implemented | Bit 0 of wire value |
| Separability bit | Bit 6 of code (0x40), under reconsideration | Bit 5 of |code| (0x20) |
| Crypto type codes | ECDSA (1,−1), RSA (61,62), SHA3 (−5) | Ed25519 (1,2), SHA2-256 (3), BLAKE3 (4) |
| Type code numbering | Sparse (1–5221) | Compact (1–36, more as needed) |

The 2018 C++ code is reference material only. No binary compatibility is maintained.
