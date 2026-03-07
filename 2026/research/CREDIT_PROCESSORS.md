# Global Credit Card Processor Financial Flows

**Research date**: March 2026 | **Data vintage**: FY 2024 unless noted
**Methodology**: Compiled from SEC filings, central bank reports, Nilson Report, and industry analyses.

> **Data quality legend** used throughout this document:
> - **[D]** = Direct: from official filings, regulators, or audited reports
> - **[M]** = Mixed: derived from official growth rates applied to prior-year actuals, or from reputable industry analysts (Nilson, CFPB, Fed)
> - **[I]** = Inferred: analyst estimates, market-sizing firms, or calculated from available data

---

## Table of Contents

1. [Industry Overview](#1-industry-overview)
2. [Major Network Financials](#2-major-network-financials)
3. [Revenue Sources Deep Dive](#3-revenue-sources-deep-dive)
4. [Expense Categories Deep Dive](#4-expense-categories-deep-dive)
5. [Regional Market Analysis](#5-regional-market-analysis)
6. [The Payment Flow: Who Gets What](#6-the-payment-flow-who-gets-what)
7. [Competing Payment Rails](#7-competing-payment-rails)
8. [Data Tables](#8-data-tables)
9. [Sources](#9-sources)

---

## 1. Industry Overview

**Based on direct information only.**

### Global Scale (2024)

| Metric | Value | Source |
|---|---|---|
| Global card purchase + cash volume | $51.92 trillion | Nilson [D] |
| Global network brand purchase volume | $36.74 trillion | Nilson [D] |
| Global card transactions | ~728 billion | Nilson [D] |
| US card spending (Visa+MC+Amex+Discover) | $10.7 trillion | Nilson [D] |
| US merchant processing fees | $187.2 billion | Nilson [D] |
| US credit card interchange fees | $148.5 billion | Nilson [D] |
| Global card fraud losses | $33.4 billion | Nilson [D] |

### Market Share by Network (2024)

| Network | Global Volume (est.) | Cards | Global Share (txns) | Model |
|---|---|---|---|---|
| UnionPay | ~$35.7T | 9.6B (59%) | ~32-36% | State network/switch |
| Visa | ~$16T | 4.6B (24%) | ~30% | Four-party network |
| Mastercard | ~$9.8T (GDV) | 3.5B (15%) | ~19% | Four-party network |
| American Express | $1.55T | 146M | ~4% | Closed-loop issuer+network |
| JCB | ~$346-410B | 158M | ~1-2% | Hybrid issuer+network |
| Discover/Diners | $224.6B (cards) | 60.6M | ~1% | Closed-loop issuer+network |
| RuPay | Not separately reported | 750M+ | India-only | Government-backed network |

*Note: UnionPay's dominance is driven by mandatory domestic Chinese usage. Outside China, Visa and Mastercard control ~80%+ of card transactions.* [D/M]

---

## 2. Major Network Financials

### Visa Inc. (FY2024, ended Sep 30, 2024)

**Based on direct information only.**

| Line Item | Amount | YoY |
|---|---|---|
| Service revenues | $16.1B | +9% |
| Data processing revenues | $17.7B | +11% |
| International transaction revenues | $12.7B | +9% |
| Other revenues | $3.2B | +29% |
| **Gross revenues** | **~$49.7B** | |
| Client incentives (contra-revenue) | ($13.8B) | +12% |
| **Net revenues** | **$35.9B** | **+10%** |
| Operating expenses | $12.3B | +6% |
| Operating income | $23.6B | +12% |
| **Net income** | **$19.7B** | **+14%** |
| Net profit margin | ~55% | |

**Geographic split**: US $14.8B (41%), International $21.1B (59%) [D]

### Mastercard Inc. (FY2024, ended Dec 31, 2024)

**Based on direct information only.**

| Line Item | Amount | YoY |
|---|---|---|
| Payment Network segment | $17.3B | +10% |
| Value-Added Services segment | $10.8B | +17% |
| **Net revenue** | **$28.2B** | **+12%** |
| Operating expenses | $12.6B | +13% |
| Operating income | $15.6B | +13% |
| **Net income** | **$12.9B** | **+15%** |
| Net profit margin | ~46% | |

**Geographic split**: North America $12.4B (44%), International $15.8B (56%) [D]

### American Express (FY2024)

**Based on direct information only.**

| Line Item | Amount | YoY |
|---|---|---|
| Discount revenue (merchant fees) | ~$35.2B | +5% |
| Net interest income | $15.5B | +18% |
| Net card fees (annual fees) | $8.4B | +16% |
| Other fees & commissions | ~$6.8B | est. |
| **Total revenue net of interest** | **$65.9B** | **+9%** |
| Total consolidated expenses | $47.9B | +6% |
| Provision for credit losses | $5.2B | +6% |
| **Net income** | **$10.1B** | **+25%** |

**Geographic split**: US $51.5B (78%), EMEA $6.2B (9%), Asia-Pacific $4.7B (7%), LatAm $3.9B (6%) [D]

### Discover Financial Services (FY2024 -- last standalone year before Capital One acquisition)

**Based on direct information only.**

| Line Item | Amount | YoY |
|---|---|---|
| Net interest income | ~$14.3B | +11% |
| Non-interest income | ~$3.6B | +18% |
| **Revenue net of interest** | **$17.9B** | **+13%** |
| Provision for credit losses | ~$4.66B | down |
| Total other expense | ~$5.9B | |
| **Net income** | **$4.5B** | **+62%** |

*Note: Discover is primarily a lender that also runs a network. 80% of revenue is net interest income.* [D]

### UnionPay (China)

**Based on a mix of direct and inferred data.**

UnionPay is state-owned and does not publish audited financials. Revenue estimated at $5-8B [I]. Key known metrics:
- 9.6B cards issued, ~96% China domestic market share [D/M]
- Domestic MDR ~0.38-0.45%, of which UnionPay keeps ~0.1% [M]
- Accepted in 192 countries, 67M merchants [D]

### JCB (Japan)

**Based on a mix of direct and inferred data.**

JCB is private. Revenue estimated at ~$2.8B [I]. ~158M cards, ~18.8% Japan credit card market share (behind Visa 42.3%, Mastercard 36.6%) [M]. International acceptance via reciprocal agreement with Discover Global Network.

### RuPay (India / NPCI)

**Based on a mix of direct and inferred data.**

NPCI operates on a not-for-profit basis; no revenue disclosed. 750M+ cards (mostly debit), ~38% of Indian credit card transactions by volume but ~16% by value [M]. Zero MDR on debit by law since 2020; government subsidizes ~$180-240M/year to banks [D].

---

## 3. Revenue Sources Deep Dive

### 3.1 Interchange Fees (paid by merchant's bank to cardholder's bank)

**Based on direct information only.** Interchange is set by the networks but flows between banks -- not directly to Visa/Mastercard.

#### US Interchange Ranges (2024)

| Card Type | Visa | Mastercard |
|---|---|---|
| Consumer credit, card-present | 1.43% + $0.10 | 1.58% + $0.10 |
| Consumer credit, card-not-present | 1.65% + $0.15 | 1.73% + $0.10 |
| Premium/rewards credit | 1.65-2.40% + $0.10 | 1.90-2.50% + $0.10 |
| Commercial/corporate CNP | 2.70% + $0.10 | Similar range |
| Regulated debit (Durbin, >$10B issuer) | 0.05% + $0.21 | 0.05% + $0.21 |
| Exempt debit, card-present | ~0.80% + $0.15 | ~1.05% + $0.15 |

#### Global Interchange Comparison

| Region | Credit Interchange | Debit Interchange | Regulatory Framework |
|---|---|---|---|
| **US** | 1.15-2.40% | 0.05%+$0.21 (reg) / 0.70-1.00% (exempt) | Durbin Amendment (debit only) |
| **EU** | **0.30% cap** | **0.20% cap** | IFR 2015/751, extended to 2029 |
| **UK** | 0.30% cap | 0.20% cap | Maintained post-Brexit |
| **China** | **0.35% uniform** | 0.35% uniform | PBOC regulation (2016) |
| **India (credit)** | 1.2-1.8% | 0.3-0.9% (capped) | RBI regulation |
| **India (RuPay debit/UPI)** | **0%** | **0%** | Zero MDR law (2020) |
| **Africa (SA)** | ~1.68% (3D auth) | Varies | SARB review |
| **Latin America** | 1.5-2.0% | ~1.0-1.5% | Varies by country |
| **SE Asia** | 1.5-2.5% | Varies | Largely unregulated |

[D for rates; M for some regional figures]

### 3.2 Assessment / Network Fees (paid to Visa/Mastercard by acquirers)

**Based on direct information only.**

| Fee | Visa | Mastercard |
|---|---|---|
| Base assessment (credit) | 0.14% | 0.12-0.14% |
| Base assessment (debit) | 0.13% | 0.12-0.14% |
| Per-authorization fee | $0.0195 | $0.0195 (NABU) |
| Cross-border (USD settlement) | ~1.0% combined | ~0.60% |
| Cross-border (non-USD) | ~1.4% combined | ~1.00% |

These assessment fees are how Visa and Mastercard themselves earn revenue. On a $100 domestic transaction, the network collects roughly $0.13-$0.18 -- a tiny fraction of the total merchant fee.

### 3.3 Other Revenue Streams

| Source | Description | Who Earns It |
|---|---|---|
| Value-added services | Fraud tools, analytics, consulting, tokenization | Visa, Mastercard (growing ~17% YoY) |
| Net interest income | Interest on revolving balances | Amex ($15.5B), Discover ($14.3B), issuing banks |
| Annual card fees | Premium card membership | Amex ($8.4B), issuing banks |
| Data processing | Transaction routing, authorization, clearing | Visa ($17.7B), Mastercard |
| Currency conversion markup | 1-3% FX fee on cross-border | Issuing banks + networks |

---

## 4. Expense Categories Deep Dive

### 4.1 Network Operating Expenses

**Based on a mix of direct and inferred data.**

| Category | Visa (FY2024) | Mastercard (FY2024) |
|---|---|---|
| **Personnel** | ~$5.0B [I] | $6.7B [D] |
| **Marketing/advertising** | ~$1.4B [I] | ~$1-2B [I] |
| **Network & processing** | ~$1.0B [I] | included in G&A |
| **Depreciation & amortization** | ~$1.0B [I] | ~$1.0B [I] |
| **Litigation provisions** | ~$1.5B [I] | ~$0.5B [I] |
| **General & administrative** | ~$1.8B [I] | ~$10.0B total G&A [M] |
| **Professional fees** | ~$0.6B [I] | included in G&A |
| **Total operating expenses** | **$12.3B [D]** | **$12.6B [D]** |

*Note: Visa breaks expenses into 7 line items; Mastercard consolidates most under G&A. Individual Visa line items are estimates; totals are official.*

### 4.2 Client Incentives (Contra-Revenue)

The single largest "expense" for Visa and Mastercard is client incentives -- payments to issuers and merchants to use their network:

| | Visa | Mastercard |
|---|---|---|
| Client incentives | $13.8B [D] | ~$11.6B [M] |
| As % of gross revenue | ~28% | ~29% |

### 4.3 Fraud Costs

**Based on a mix of direct and inferred data.**

| Metric | Value | Source |
|---|---|---|
| Global card fraud losses (2024) | $33.4B | Nilson [D] |
| US share of global fraud | 41.9% (from 26.3% of volume) | Nilson [D] |
| Card-not-present fraud share | ~71% of US losses (~$10B) | Fed/Nilson [D] |
| Synthetic identity fraud (2025) | $23B worldwide | Industry [I] |
| Account takeover fraud (2025) | $17B worldwide | Industry [I] |

**Who bears fraud losses** (US debit cards, 2023 Fed data) [D]:
- Merchants: 49.9% (up from 38.3% in 2011)
- Issuers: 28.3% (down from 59.8% in 2011)
- Cardholders: 21.8% (up from <1.8% in 2011)
- Networks (Visa/MC): ~0%

**Fraud prevention market**: $33.1B in 2024, projected $90B by 2030 (CAGR 18.7%) [I]

### 4.4 Rewards and Cash Back

**Based on a mix of direct and inferred data.**

| Metric | Value | Source |
|---|---|---|
| US consumer rewards earned (2022) | >$40B | CFPB [D] |
| Breakdown: points | $21B | CFPB [D] |
| Breakdown: cash back | $15.2B | CFPB [D] |
| Breakdown: miles | $5.2B | CFPB [D] |
| US cashback market (2023) | $38.4B | Industry [M] |
| Unredeemed rewards liability | >$33B | CFPB [D] |
| Amex variable engagement costs | ~$30-32B [I] | Amex earnings |

Rewards are funded primarily from interchange fees. CFPB finding: consumers who carry debt earn 27% of rewards but pay 94% of interest/fees -- revolvers subsidize transactors' rewards. [D]

### 4.5 Chargebacks

**Based on a mix of direct and inferred data.**

| Metric | Value | Source |
|---|---|---|
| Global chargeback volume (2025) | 261 million | Industry [I] |
| Global chargeback value (2025) | $33.8B | Industry [I] |
| Average cost per chargeback to merchant | $191 | Industry [I] |
| True cost multiplier | $3.75 per $1 of fraud | Industry [I] |
| Friendly fraud share of chargebacks | 61-75% | Industry [I] |
| First-party fraud share (2024) | 36% (up from 15% in 2023) | Industry [M] |

### 4.6 Lobbying and Legal

**Based on direct information only.**

| Entity | US Lobbying (2024) | Political Contributions |
|---|---|---|
| Visa | $7.72M | $1.89M |
| Mastercard | $5.21M | $796K |
| American Bankers Assn | $7.39M | N/A |

**Major legal costs**:
- Visa/MC interchange settlement: amended to ~$38B in projected merchant savings over 5 years (rate reductions of 4-7 bps, rate freeze) [D]
- DOJ antitrust suit against Visa (Sep 2024): alleging monopoly over debit network processing [D]

### 4.7 Marketing

**Based on a mix of direct and inferred data.**

| Company | Marketing Spend (2024) | Notes |
|---|---|---|
| American Express | $6.3-6.8B | +16-18% YoY; record 13M new cards |
| Visa | ~$1.5-2.0B [I] | Olympic/FIFA sponsorships |
| Mastercard | ~$1-2B [I] | "Priceless" campaign, 120+ countries |
| Implied Amex customer acquisition cost | ~$523/card [I] | $6.8B / 13M cards |

---

## 5. Regional Market Analysis

### 5.1 United States

**Based on direct information only.**

| Metric | Value |
|---|---|
| Card payment volume (2024) | $11.9T |
| Merchant processing fees | $187.2B |
| Visa share (US purchase volume) | 61.1% ($6.58T) |
| Mastercard share | 25.8% ($2.78T) |
| Amex share | 11.1% ($1.19T) |
| Discover share | 2.0% ($212B) |
| Average credit interchange | ~1.8-2.0% |
| Merchant total cost | 1.3-3.5% |
| Cash share of POS transactions | ~16% (down from 44% in 2014) |

The US has the **highest merchant acceptance costs globally** and the most disproportionate fraud rate (41.9% of global fraud from 26.3% of volume).

### 5.2 Western Europe (EU + UK)

**Based on direct information only** (regulatory caps); **mixed** for volumes.

| Metric | Value |
|---|---|
| Euro area card volume (2024) | ~EUR 3.2T (~$3.5T) |
| Card transactions | ~84.4B |
| YoY growth | +7-9% |
| Cards as % of transactions | 57% |
| Credit interchange cap | **0.30%** |
| Debit interchange cap | **0.20%** |
| Average merchant total cost | ~0.96% |
| Cash share of POS | ~25-33% (varies: Sweden ~10%, Germany ~40%) |

Key domestic schemes: Cartes Bancaires (France), girocard (Germany), Bancontact (Belgium), iDEAL (Netherlands).

### 5.3 India

**Based on a mix of direct and inferred data.**

| Metric | Value |
|---|---|
| Card payment value (2024) | ~$329B |
| Credit card spend (FY2024) | INR 18.26T (~$219B), +27% |
| UPI volume (2024) | **172.2B transactions, ~$2.95T** |
| UPI volume (2025) | **228.3B transactions, ~$3.58T** |
| UPI share of all payment volumes | 85% |
| RuPay debit MDR | **0% (by law)** |
| UPI MDR | **0% (by law)** |
| Dominant UPI apps | PhonePe 48.4%, Google Pay 36.9% |

India is the most dramatic example of government-backed rails disrupting card networks. UPI processes more transactions monthly than most card networks handle globally.

### 5.4 China

**Based on a mix of direct and inferred data.**

| Metric | Value |
|---|---|
| UnionPay transaction volume | ~$35.7T |
| UnionPay domestic market share | ~96% |
| Uniform interchange rate | 0.35% (PBOC regulation) |
| Total MDR | ~0.38-0.45% |
| Alipay users | 1.4B MAU |
| WeChat Pay users | ~935M |
| Alipay+WeChat mobile share | >90% of mobile payments |
| Mobile payment volume | >$80T (includes transfers) |

China is the world's most cashless large economy for urban transactions. QR-code mobile wallets (Alipay, WeChat Pay) have substantially displaced physical cards for everyday purchases.

### 5.5 Africa

**Based on a mix of direct and inferred data.**

| Metric | Value |
|---|---|
| Kenya M-Pesa users | 34M Kenya, 70M+ Africa |
| M-Pesa volume (2025) | >$450B (incl. P2P) |
| Kenya mobile money penetration | 91% of adults |
| Nigeria electronic payments (2024) | ~$650B (+80% YoY) |
| South Africa interchange (3D auth) | ~1.68% |
| Cash at POS (Nigeria, 2024) | ~40% (down from 91% in 2019) |

Africa leapfrogged cards entirely -- mobile money (M-Pesa) is the primary digital payment method. Card penetration remains low outside South Africa.

### 5.6 Latin America (focus: Brazil)

**Based on direct information only** (PIX data from Banco Central).

| Metric | Value |
|---|---|
| PIX transactions (2024) | 63.4B (+53% YoY) |
| PIX value (2024) | $4.6T |
| PIX penetration | 93% of Brazilian adults |
| PIX merchant fees | 0-0.5% |
| Regional credit interchange | 1.5-2.0% |
| Cash at POS (region) | ~25% |
| Mexico cash at POS | 47% (down from 66% in 2022) |

PIX surpassed combined debit + credit card volume in Brazil by 80% in 2024. Credit cards retain share in installment purchases ("parcelado").

### 5.7 Southeast Asia

**Based on a mix of direct and inferred data.**

| Metric | Value |
|---|---|
| Indonesia digital payment volume (2024) | ~$39.5B (+335% YoY surge) |
| Credit card interchange | 1.5-2.5% (largely unregulated) |
| Cash at POS: Singapore | ~13% |
| Cash at POS: Indonesia | ~50-60% |
| Cash at POS: Philippines | ~60-70% |
| Dominant wallets | GrabPay (SG/MY), GoPay (ID), GCash (PH) |

ASEAN is building cross-border QR code payment links between national schemes. E-wallets and A2A transfers are growing faster than card payments.

---

## 6. The Payment Flow: Who Gets What

### On a typical $100 US credit card purchase:

**Based on direct information only.**

```
$100.00  Customer pays
 -$1.70   Interchange fee -> Issuing bank (1.70%)
 -$0.14   Assessment fee  -> Card network (0.14%)
 -$0.02   Per-txn fee     -> Card network ($0.0195)
 -$0.30   Acquirer markup  -> Payment processor
=========
$97.84   Merchant receives ($2.16 total cost = 2.16% effective rate)
```

| Recipient | Amount | Range | What It Funds |
|---|---|---|---|
| **Issuing bank** | ~$1.50-1.75 | 1.4-2.1% + $0.10 | Rewards, fraud losses, credit risk, operations |
| **Card network** (Visa/MC) | ~$0.13-0.18 | 0.13-0.17% | Network infrastructure, R&D, brand |
| **Acquirer/processor** | ~$0.07-0.50 | Variable | Processing, terminals, customer support |
| **Merchant receives** | ~$97.60-98.30 | | |

**American Express difference**: Amex is both network and issuer, so it keeps both the interchange-equivalent AND network fee. Amex MDR is typically 2.5-3.5%, hence lower merchant acceptance.

**EU comparison**: On EUR 100, total merchant cost is ~EUR 0.50-1.50 (vs $1.70-2.40 in US) due to regulatory caps.

**China comparison**: On CNY 100, total MDR is ~CNY 0.38-0.45 (0.38-0.45%).

**India (UPI) comparison**: On INR 100 via UPI, merchant cost is **INR 0** (zero MDR, government-subsidized).

### Where the total global fee pool goes

**Based on a mix of direct and inferred data.**

On ~$52T in global card transactions (2024):

| Recipient | Estimated Revenue | Source |
|---|---|---|
| Issuing banks (interchange) | $150-200B | [I] ~0.3-0.4% blended globally |
| Card networks (Visa+MC) | $64B | [D] combined net revenue |
| Acquirers/processors | $60-80B | [I] Fiserv, Worldpay, Stripe, etc. |
| **Total fee pool from merchants** | **$250-350B/year** | [I] |

### Major acquirers/processors (2024)

| Company | Revenue | Payment Volume | Role |
|---|---|---|---|
| Fiserv | $20.5B | Largest by merchant vol | Merchant acquirer |
| PayPal | ~$31B | ~$1.5T | Digital wallet + processing |
| Block (Square) | $24.1B | $241B GPV | SMB POS + Cash App |
| Stripe | ~$18-20B [I] | $1.4T (+38% YoY) | Online/API-first processor |
| FIS | $14.5B | 45% Worldpay stake | Banking tech |
| Global Payments | $10.1B | -- | Merchant + issuer solutions |
| Adyen | EUR 2.0B net | $1T+ | Enterprise unified commerce |

---

## 7. Competing Payment Rails

**Based on direct information only** (transaction volumes); **mixed** for projections.

The card industry faces its first serious structural threat from government-backed real-time payment systems:

| System | Country | 2024 Volume | Merchant Fee | Status |
|---|---|---|---|---|
| **UPI** | India | 172B txns, $2.95T | **0%** | Dominant (85% of payments) |
| **PIX** | Brazil | 63.4B txns, $4.6T | 0-0.5% | 5x card volume in Brazil |
| **Alipay+WeChat** | China | >$80T combined | ~0.1-0.6% | >90% mobile share |
| **M-Pesa** | Kenya/Africa | >$450B | 0.5-1.5% | 91% penetration Kenya |
| **FedNow** | US | 1.5M txns (early) | TBD | Launched Jul 2023, slow adoption |
| **SEPA Instant** | EU | 23% of retail EUR txns | Low | Mandate effective 2025 |

**Pattern**: Where governments deploy zero/low-fee instant payment rails, card transaction share declines rapidly. Cards still dominate in the US and most of Europe but face growing structural pressure.

**Other disruptors** (smaller impact so far):
- **BNPL**: ~$560B global market (2025), ~1.1% of US card volume. Banks losing est. $8-10B/year [I]
- **Stablecoins**: $27.6T transfer volume (2024) but ~80% is bot/arbitrage activity, not consumer payments [M]
- **Open Banking / PSD3**: EU mandate for better APIs; "pay by bank" could bypass card rails if made as seamless as tap-to-pay [M]

---

## 8. Data Tables

Detailed data tables are provided in the companion spreadsheet:

**[CREDIT_PROCESSORS_DATA.csv](CREDIT_PROCESSORS_DATA.csv)**

Sheets/sections in the CSV:
1. Network financials comparison
2. Revenue breakdown by source
3. Operating expense breakdown
4. Regional market size and fee comparison
5. Fraud statistics by region
6. Interchange fee comparison by region
7. Competing payment rails

---

## 9. Sources

### Official Filings and Regulators
- Visa FY2024 10-K and Earnings Release (SEC EDGAR)
- Mastercard FY2024 10-K and Earnings Release (SEC EDGAR)
- American Express FY2024 10-K and Earnings Release
- Discover Financial Services FY2024 Earnings Release
- Federal Reserve: 2023 Interchange Fee Revenue and Fraud Losses
- Federal Reserve Bank of Kansas City: Card-Present and Card-Not-Present Fraud Rates
- European Central Bank: Payments Statistics H1/H2 2024
- Reserve Bank of India: MDR Rationalisation, Digital Payments Reports
- Banco Central do Brasil: PIX Statistics
- People's Bank of China: Uniform Interchange Rate Regulation (2016)
- South African Reserve Bank: Interchange Rates (Dec 2024)
- EU Regulation 2015/751 (Interchange Fee Regulation)
- CFPB: Credit Card Rewards Issue Spotlight (May 2024)
- CFPB: 2023 Consumer Credit Card Market Report

### Industry Reports
- Nilson Report: Global Brand Cards Midyear 2024, Card Fraud Losses 2024, US Merchant Processing Fees 2024
- OpenSecrets: Visa and Mastercard Lobbying Profiles (2024)
- NPCI/UPI Transaction Statistics
- Capital One Shopping: Credit Card Market Share 2025

### Analyst Estimates and Market Research
- Grand View Research: Fraud Detection & Prevention Market
- Chargeflow / Chargebacks911: Chargeback Statistics
- GlobalData: India Card Payments
- Worldpay: Global Payments Report
- PCMI: Payment Methods Latin America
- CoinLaw: Network Statistics (UnionPay, RuPay, Alipay/WeChat, JCB)
- Clearly Payments: Payment Processing Market by Region
- EBANX: PIX Monthly Transaction Projections
- FXC Intelligence: FedNow 2024 Update
