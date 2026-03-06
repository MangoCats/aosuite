# Economic Rules Specification — Deliverable 0D

Deterministic arithmetic rules that all nodes must compute identically. Every formula uses arbitrary-precision integers with explicit rounding. Division is always the last operation.

Related specs: [Architecture.md](Architecture.md) (0A), [WireFormat.md](WireFormat.md) (0B), [CryptoChoices.md](CryptoChoices.md) (0C).

---

## 1. Genesis Parameters

A chain starts with a genesis block declaring these economic parameters:

| Parameter | Type | Typical Value | Description |
|-----------|------|---------------|-------------|
| `SHARES_OUT` | big integer | ~2⁸⁶ | Initial shares outstanding |
| `COIN_COUNT` | big integer | 10,000,000,000 | Fixed display coin count (never changes) |
| `FEE_RATE` | rational | 1/1000000 | Fraction of SHARES_OUT retired per data byte (see §3) |
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

Genesis: `SHARES_OUT = 2⁸⁶`, `COIN_COUNT = 10,000,000,000` (10 billion).

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

**What FEE_RATE means:** `FEE_RATE` is the fraction of `SHARES_OUT` retired per byte of recorded data. With `FEE_RATE = 1/1,000,000`, each byte retires one-millionth of all remaining shares. A 500-byte transaction retires 500/1,000,000 = 0.05% of `SHARES_OUT`. In coin terms, the fee is approximately `data_bytes × FEE_RATE × COIN_COUNT` coins — stable regardless of how many shares have been retired, because the share-to-coin ratio adjusts proportionally.

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
          ≈ 5,000,000 coins (0.05% of COIN_COUNT)
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

Half the shares in absolute terms, but the same fee in coins (~5,000,000 coins) because the share/coin ratio also halved.

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
- `EXPIRY_PERIOD` = 1 year = `31,557,600 × 189,000,000 = 5,964,386,400,000,000` AO timestamp units.
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

## 7. Chain Lifetime and Migration

### 7.1 Share Pool Decay

Because recording fees retire shares and new shares are never created, `SHARES_OUT` decays exponentially over the life of a chain. Each transaction retires a fraction of the remaining pool:

```
fraction_per_tx ≈ data_bytes / FEE_RATE_den
```

For a 500-byte transaction with `FEE_RATE = 1/1,000,000`, each transaction retires ~0.05% of `SHARES_OUT`. After N transactions:

```
SHARES_OUT_N = SHARES_OUT_0 × (1 - fraction_per_tx)^N
```

Starting at 2⁸⁶ with `COIN_COUNT = 10,000,000,000` (~2³³), nanocoin precision requires at least ~2³³ shares — a loss of 53 bits of range. The number of transactions to reach that point:

```
N ≈ 53 × ln(2) / fraction_per_tx ≈ 73,400 transactions (at 500 bytes, FEE_RATE = 1/1,000,000)
```

| Daily Transaction Rate | Approximate Years to Lose Nanocoin Precision |
|-----------------------:|---------------------------------------------:|
| 1 tx/day | ~201 years |
| 10 tx/day | ~20 years |
| 50 tx/day | ~4 years |
| 100 tx/day | ~2 years |

Share expiration and age tax accelerate this decay further, since every refresh transaction also costs a fee.

### 7.2 Finite Chain Lifetime Is By Design

Chains are **not intended to run forever.** A chain serves a community for a bounded period, after which share holders migrate to a successor chain. This is a feature, not a limitation:

1. **Parameter re-evaluation.** Genesis parameters (`FEE_RATE`, `EXPIRY_PERIOD`, `EXPIRY_MODE`, `TAX_PARAMS`) are immutable. Migration to a successor chain is the mechanism for adjusting these parameters based on operational experience.

2. **Fresh arithmetic headroom.** The successor chain starts with a full `SHARES_OUT` pool, restoring nanocoin precision for another generation.

3. **Graceful transition via age tax.** Mode 2 (age tax) naturally encourages regular share refreshing. When a successor chain is announced, share holders refresh by migrating to the new chain instead of self-assigning on the old one. The age tax ensures that abandoned shares on the old chain decay to insignificance without requiring any explicit shutdown.

4. **Recorder succession.** Migration is also the mechanism for transitioning to a new Recorder operator, upgrading cryptographic algorithms (via new type codes on the successor chain), or splitting/merging communities.

### 7.3 Migration Process

Migration from an old chain to a successor chain is an exchange operation (see [Architecture.md](Architecture.md) §3.2):

1. The Recorder announces a successor chain with its genesis block and parameters.
2. The Recorder (or a designated exchange agent) offers 1:1 coin-equivalent exchange between the old and successor chains.
3. Share holders transfer their old-chain shares to the exchange agent and receive equivalent shares on the successor chain (minus recording fees on both chains).
4. The age tax on the old chain ensures stragglers lose value gradually rather than being cut off abruptly. Wallet software SHOULD notify users of pending migrations.
5. The old chain remains readable indefinitely for audit purposes, even after all shares have expired or migrated.

The Recorder's choice of `FEE_RATE` and `EXPIRY_MODE` on the successor chain can reflect lessons learned — a busier-than-expected community might choose a lower `FEE_RATE` to extend chain lifetime, while a community that found expiration too aggressive might lengthen `EXPIRY_PERIOD` or adjust `TAX_PARAMS`.

---

## 8. Invariants

These must hold at all times and are checked by Validators:

1. **Conservation:** For every block, `new_SHARES_OUT = old_SHARES_OUT - total_fees_retired - total_expired_shares`.
2. **Balance:** For every assignment, `sum(giver_amounts) = sum(receiver_amounts) + fee_shares`.
3. **No negative shares:** All amounts are positive big integers.
4. **UTXO integrity:** Every giver references a valid, unspent, non-expired sequence ID. After recording, that sequence ID is marked spent.
5. **Key uniqueness:** No public key appears as a receiver more than once on the same chain.
6. **Timestamp ordering:** Every signature timestamp exceeds the timestamp of the block recording the signer's share receipt.
7. **Monotonic blocks:** Each block's timestamp exceeds the previous block's timestamp.
8. **Hash chain:** Each block's `PREV_HASH` equals `SHA2-256(parent_BLOCK_SIGNED_encoding)`.
