# Economic Rules Specification — Deliverable 0D

Deterministic arithmetic rules that all nodes must compute identically. Every formula uses arbitrary-precision integers with explicit rounding. Division is always the last operation.

Related specs: [Architecture.md](Architecture.md) (0A), [WireFormat.md](WireFormat.md) (0B), [CryptoChoices.md](CryptoChoices.md) (0C).

---

## 1. Genesis Parameters

A chain starts with a genesis block declaring these economic parameters:

| Parameter | Type | Typical Value | Description |
|-----------|------|---------------|-------------|
| `SHARES_OUT` | big integer | ~2⁸⁶ | Initial shares outstanding |
| `COIN_COUNT` | big integer | ~2⁸³ | Fixed display coin count (never changes) |
| `FEE_RATE` | rational | 1/1000000 | Recording fee per byte per coin |
| `EXPIRY_PERIOD` | timestamp delta | 1 year | Share expiration period |
| `EXPIRY_MODE` | integer | 1 or 2 | 1 = hard cutoff, 2 = age tax |
| `TAX_PARAMS` | (if mode 2) | start_age, doubling_period | Age tax curve parameters |

All parameters are immutable after genesis. To change parameters, create a new chain and migrate shares via exchange.

### 1.1 Why Large Share Pools

Starting with ~2⁸⁶ shares ensures sub-nanocoin precision for recording fees without fractional shares. As shares are retired over years, the pool shrinks but remains large enough for fine-grained arithmetic. The coin count (~2⁸³) is purely for display — users see "12.50 BCG" rather than "96714065652...1024 shares."

---

## 2. Coin Display

Coins are a human-readable view of a share balance. The conversion is:

```
user_coins = (user_shares × COIN_COUNT) / SHARES_OUT
```

- All values are arbitrary-precision integers.
- **Truncate** (round toward zero) to 9 decimal places (nanocoin) for display.
- This is a **display-only** calculation — it is never used in consensus-critical arithmetic.
- `COIN_COUNT` is fixed at genesis. `SHARES_OUT` decreases as fees are retired.

### 2.1 Worked Example

Genesis: `SHARES_OUT = 2⁸⁶`, `COIN_COUNT = 2⁸³`.

Alice holds 2⁸³ shares (1/8 of total):
```
user_coins = (2⁸³ × 2⁸³) / 2⁸⁶ = 2¹⁶⁶ / 2⁸⁶ = 2⁸⁰
```

Wait — that gives a huge number. The coin display should give `COIN_COUNT / 8 = 2⁸³ / 8 = 2⁸⁰`? That's still enormous.

Let's use practical numbers. Genesis: `SHARES_OUT = 2⁸⁶`, `COIN_COUNT = 10,000,000,000` (10 billion, ~2³³).

Alice holds 2⁸³ shares (1/8 of total):
```
user_coins = (2⁸³ × 10,000,000,000) / 2⁸⁶
           = 10,000,000,000 / 8
           = 1,250,000,000 coins
           = 1,250,000,000.000000000 (display)
```

After 10% of shares are retired (fee accumulation), `SHARES_OUT = 0.9 × 2⁸⁶`:
```
user_coins = (2⁸³ × 10,000,000,000) / (0.9 × 2⁸⁶)
           = 10,000,000,000 / 7.2
           = 1,388,888,888.888888888 (display, truncated at nanocoin)
```

Alice's coin balance increased ~11.1% from passive inflation, even though her share count didn't change.

---

## 3. Recording Fee

When an assignment is recorded, shares are deducted as a recording fee and **retired** (removed from `SHARES_OUT`). The fee formula:

```
fee_shares = ceil( data_bytes × FEE_RATE_num × SHARES_OUT / FEE_RATE_den )
```

Where `FEE_RATE = FEE_RATE_num / FEE_RATE_den` is the rational fee rate from genesis.

### 3.1 Deterministic Rules

1. All arithmetic uses **arbitrary-precision integers** (`num-bigint`).
2. Multiply **before** dividing: `fee_shares = ceil( (data_bytes × FEE_RATE_num × SHARES_OUT) / FEE_RATE_den )`.
3. `ceil(a / b)` for positive integers: `(a + b - 1) / b` (integer division, truncating).
4. `data_bytes` is the byte count of the complete `PAGE` DataItem encoding (type code + size + all contents).
5. `SHARES_OUT` is the value **before** this block's fees are deducted.
6. The fee is deducted from the giver(s). The sum of all receiver amounts plus the fee must equal the sum of all giver amounts exactly.

### 3.2 Worked Examples

Genesis: `SHARES_OUT = 77,371,252,455,336,267,181,195,264` (~2⁸⁶), `FEE_RATE = 1/1,000,000`.

**Example 1:** 500-byte assignment.
```
fee = ceil(500 × 1 × 77,371,252,455,336,267,181,195,264 / 1,000,000)
    = ceil(38,685,626,227,668,133,590,597,632,000 / 1,000,000)
    = ceil(38,685,626,227,668,133,590,597.632)
    = 38,685,626,227,668,133,590,598
```

In coins (with `COIN_COUNT = 10,000,000,000`):
```
fee_coins = 38,685,626,227,668,133,590,598 × 10,000,000,000 / 77,371,252,455,336,267,181,195,264
          ≈ 0.000005 coins (5 microcoins)
```

**Example 2:** 2000-byte assignment, same chain.
```
fee = ceil(2000 × 77,371,252,455,336,267,181,195,264 / 1,000,000)
    = ceil(154,742,504,910,672,534,362,390,528,000 / 1,000,000)
    = 154,742,504,910,672,534,362,391
```

**Example 3:** After 50% of shares retired, `SHARES_OUT = 38,685,626,227,668,133,590,597,632`, 500-byte assignment.
```
fee = ceil(500 × 38,685,626,227,668,133,590,597,632 / 1,000,000)
    = ceil(19,342,813,113,834,066,795,298,816,000 / 1,000,000)
    = 19,342,813,113,834,066,795,299
```

Half the shares in absolute terms, but the same fee in coins (~5 microcoins) because the share/coin ratio also halved.

### 3.3 Balance Equation

For every assignment recorded in a block:

```
sum(giver_amounts) = sum(receiver_amounts) + fee_shares
```

This is verified exactly (arbitrary-precision integer equality). Any discrepancy causes the Recorder to reject the assignment.

---

## 4. Share Expiration

Shares that are not refreshed (self-assigned to a new key) within the expiration period are retired. Two modes:

### 4.1 Mode 1: Hard Cutoff

Shares are fully valid until the expiration deadline, then fully retired.

```
expiry_time = block_timestamp_of_receipt + EXPIRY_PERIOD
```

At each block, the Recorder sweeps all UTXOs. For any UTXO where `current_block_timestamp > expiry_time`:
1. Mark the UTXO as expired (no longer spendable).
2. Subtract the expired shares from `SHARES_OUT`.

**Worked Example:**
- Alice receives 1,000,000 shares in block at timestamp T₀.
- `EXPIRY_PERIOD` = 1 year = `189,000,000 × 365.25 × 86,400 = 5,960,878,200,000,000` AO timestamp units.
- At T₀ + 5,960,878,200,000,001: Alice's shares are expired and retired.
- To prevent expiration, Alice self-assigns before the deadline (costs a recording fee).

### 4.2 Mode 2: Age Tax

Shares are progressively taxed based on age, with an exponential curve. The tax fraction at a given age:

```
if age ≤ TAX_START_AGE:
    tax_fraction = 0
else:
    tax_fraction = 1 - 2^(-(age - TAX_START_AGE) / TAX_DOUBLING_PERIOD)
```

Where:
- `age = current_block_timestamp - block_timestamp_of_receipt` (in AO timestamp units).
- `TAX_START_AGE` and `TAX_DOUBLING_PERIOD` are from `TAX_PARAMS` in genesis.

The tax is applied at block recording time:

```
taxed_shares = floor(original_shares × tax_fraction)
effective_shares = original_shares - taxed_shares
```

Taxed shares are retired from `SHARES_OUT`.

**Deterministic computation:** The exponential `2^(-x)` must be computed using rational approximation or a lookup table with interpolation, specified to produce identical results on all implementations. The exact method is TBD during Phase 1 implementation — the spec will be updated with the chosen algorithm and test vectors.

**Worked Example** (5-year expiration, doubling every 4 months):
- `TAX_START_AGE` = 0 (tax begins immediately)
- `TAX_DOUBLING_PERIOD` = 4 months

| Age | Doublings | Tax Fraction | Remaining |
|:----|----------:|-------------:|----------:|
| 0 | 0 | 0% | 100% |
| 4 months | 1 | 50% | 50% |
| 8 months | 2 | 75% | 25% |
| 1 year | 3 | 87.5% | 12.5% |
| 2 years | 6 | 98.4% | 1.56% |
| 3 years | 9 | 99.8% | 0.20% |
| 5 years | 15 | 99.997% | 0.003% |

Practical effect: shares lose half their value every 4 months if not refreshed. Exponential decay ensures graceful degradation rather than a cliff.

---

## 5. Late Recording

An assignment agreement whose deadline has passed MAY still be recorded if **all** of the following hold:

1. The source shares (giver UTXOs) are still **unspent**.
2. No **explicit refutation** of the agreement has been recorded.
3. The source shares have not **expired**.

**Rationale:** If Alice signed an agreement to pay Bob, and the Recorder was temporarily down, it would be unfair to void the agreement when both parties still intend it. The deadline is primarily a staleness check, not an absolute cutoff.

**Refutation:** A giver can record a `REFUTATION` (type code 28) referencing the agreement's hash, explicitly voiding it. Once recorded, the agreement can never be recorded. Wallets SHOULD prompt users to refute stale agreements they no longer intend.

**Bounds:** Late recording is bounded by share expiration. Once the source shares expire, the agreement becomes permanently unrecordable regardless of refutation status.

---

## 6. Block Economics

Each block the Recorder constructs:

1. **Validates** all assignments (signatures, UTXOs, amounts, fees, deadlines/late-recording rules).
2. **Assigns sequence IDs** to new receiver keys (monotonically increasing from `FIRST_SEQ`).
3. **Deducts recording fees** from each assignment. Fee shares are retired.
4. **Runs expiration sweep** (if any UTXOs have passed their expiration time).
5. **Updates `SHARES_OUT`:** subtract total fees retired + total expired shares.
6. **Records `SHARES_OUT`** in the block for verification.

The Recorder does NOT receive the fee shares — they are destroyed. The Recorder's compensation is the value provided by running the chain (increased coin value for the Recorder's own share holdings, community goodwill, potential query fees in future phases).

---

## 7. Invariants

These must hold at all times and are checked by Validators:

1. **Conservation:** For every block, `new_SHARES_OUT = old_SHARES_OUT - total_fees_retired - total_expired_shares`.
2. **Balance:** For every assignment, `sum(giver_amounts) = sum(receiver_amounts) + fee_shares`.
3. **No negative shares:** All amounts are positive big integers.
4. **UTXO integrity:** Every giver references a valid, unspent, non-expired sequence ID. After recording, that sequence ID is marked spent.
5. **Key uniqueness:** No public key appears as a receiver more than once on the same chain.
6. **Timestamp ordering:** Every signature timestamp exceeds the timestamp of the block recording the signer's share receipt.
7. **Monotonic blocks:** Each block's timestamp exceeds the previous block's timestamp.
8. **Hash chain:** Each block's `PREV_HASH` equals `SHA2-256(parent_BLOCK_SIGNED_encoding)`.
