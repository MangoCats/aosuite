# Cooperative Metadata Conventions

Conventions for encoding agricultural cooperative transaction metadata using existing AO separable item types. These are application-level conventions, not protocol changes — any chain can adopt or ignore them.

## Motivation

The [Farming Cooperatives](../../docs/html/FarmingCooperatives.html) deployment story describes Ouma recording "180 kilos, Grade A, received 7:15 AM Thursday" as a delivery to the cooperative collection point. The AO type system already supports `NOTE` (type 32), `DESCRIPTION` (type 34), and `DATA_BLOB` (type 33) as separable items that can be attached to assignments. This document standardizes how cooperatives encode structured metadata in these fields for interoperability, accounting, and audit.

## Design Principles

1. **Use existing types.** No new type codes needed. All metadata lives in `NOTE`, `DESCRIPTION`, and `DATA_BLOB` separable items.
2. **Human-readable first.** `NOTE` content is UTF-8 text with a simple `key:value` line format. Any tool that can display `NOTE` content shows useful information without special parsing.
3. **Machine-parseable.** The `key:value` format is trivially parseable for aggregation, reporting, and export.
4. **Optional.** Cooperatives can use any subset of these fields. Missing fields mean "not recorded," not "zero."

## NOTE Format

A `NOTE` (type 32) separable item attached to an assignment's PARTICIPANT container. Content is UTF-8 text, one field per line, format `key:value`. Keys are lowercase ASCII, values are trimmed. Lines starting with `#` are comments.

### Delivery Record Fields

Attached to assignments where a farmer delivers produce to the cooperative:

| Key | Value | Example |
|-----|-------|---------|
| `type` | `delivery` | `type:delivery` |
| `crop` | Crop name (free text) | `crop:tomatoes` |
| `weight_kg` | Weight in kilograms (decimal) | `weight_kg:180.5` |
| `grade` | Quality grade (cooperative-defined) | `grade:A` |
| `lot` | Lot or batch identifier | `lot:2026-W10-003` |
| `location` | Collection point name | `location:Riuki Collection Point` |

Example NOTE content:
```
type:delivery
crop:tomatoes
weight_kg:180
grade:A
lot:2026-W10-012
location:Riuki Collection Point
```

### Sale Record Fields

Attached to assignments where the cooperative sells produce to a buyer:

| Key | Value | Example |
|-----|-------|---------|
| `type` | `sale` | `type:sale` |
| `crop` | Crop name | `crop:tomatoes` |
| `weight_kg` | Weight sold | `weight_kg:500` |
| `price_per_kg` | Price per kg in local currency | `price_per_kg:45.00` |
| `buyer` | Buyer name | `buyer:Nairobi Fresh Market` |
| `market` | Market or destination | `market:Wakulima` |

### Cost Allocation Fields

Attached to assignments representing cooperative expense distributions:

| Key | Value | Example |
|-----|-------|---------|
| `type` | `cost` | `type:cost` |
| `category` | Expense category | `category:transport` |
| `description` | Free-text description | `description:Truck to Wakulima market` |
| `total` | Total cost in local currency | `total:15000` |
| `split` | Number of members sharing cost | `split:47` |

### Advance/Credit Fields

Attached to assignments for advance payments against future deliveries:

| Key | Value | Example |
|-----|-------|---------|
| `type` | `advance` | `type:advance` |
| `season` | Growing season identifier | `season:2026-long-rains` |
| `purpose` | Purpose of advance | `purpose:seed+fertilizer` |

## DESCRIPTION for Extended Notes

`DESCRIPTION` (type 34) is used for longer free-text context that doesn't fit the structured `key:value` format — inspector notes, damage assessments, quality observations. UTF-8 text, no format constraints.

## DATA_BLOB for Attachments

`DATA_BLOB` (type 33) is used for binary attachments — photos of crop conditions, weighbridge receipts, signed delivery slips. The first 4 bytes are a MIME-type length (big-endian u32), followed by the MIME type string, followed by the binary content.

```
[4 bytes: MIME length][MIME string][binary content]
```

Example: A JPEG photo of a delivery weighing.
```
[00 00 00 0A]image/jpeg[...JPEG bytes...]
```

Since `DATA_BLOB` is separable, the actual binary content can be stripped from the on-chain record while preserving the hash for later verification. This keeps chain storage efficient while allowing photo documentation to be verified against the chain.

## Aggregation

A cooperative management tool can scan blocks for assignments with `NOTE` items, parse the `key:value` format, and generate:

- **Delivery ledger:** Per-farmer totals by crop, grade, and period.
- **Sale reports:** Revenue by market, buyer, and crop.
- **Cost allocation:** Per-member expense breakdown.
- **Advance tracking:** Outstanding advances per member per season.
- **Provenance trail:** Farm-to-market chain for any lot number.

## Supply Chain Provenance

A lot identifier (`lot:2026-W10-012`) appearing in both a `type:delivery` NOTE (farmer→cooperative) and a `type:sale` NOTE (cooperative→buyer) creates a verifiable provenance link. Combined with the EXCHANGE_LISTING separable item (type 37) for cross-chain trades, a complete farm-to-consumer trail can be reconstructed from on-chain data.

## Compatibility

These conventions are forward-compatible — adding new `key:value` fields requires no protocol changes. Parsers should ignore unrecognized keys. The `type` field disambiguates record kinds, so different cooperatives can extend the format independently.
