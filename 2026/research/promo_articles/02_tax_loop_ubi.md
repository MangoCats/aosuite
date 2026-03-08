# The Tax Loop: How UBI Tokens Become a Local Economy

*Target audience: Development economics, UBI advocates (Scott Santens community, r/BasicIncome, BIEN network, UNCDF/CGAP). Lead with Marshall Islands, explain the gap, show the solution.*

---

In December 2025, the Marshall Islands quietly did something no country had done before: it launched a blockchain-based universal basic income. Every citizen -- about 42,000 people spread across 29 atolls in the middle of the Pacific -- receives approximately $200 per quarter in USDM1 tokens on the Stellar blockchain, distributed through the Lomalo digital wallet app. The tokens are fully backed by U.S. Treasury bills held in trust via the Compact Trust Fund, which has grown at 6.9% annually since 2004.

This is real. It is running. It is the first national UBI funded by a permanent endowment rather than annual appropriations.

And it has a problem.

## The Problem: Tokens With Nowhere to Go

Nalu is a fisherman on Likiep, an atoll of about 400 people. When his first ENRA payment arrived -- $200 in his Lomalo wallet -- he was pleased but uncertain. What can he actually do with digital tokens on Likiep? There is one store. It doesn't accept digital payments. The copra trader pays cash. The supply boat captain wants cash for freight. Nalu's tokens sit in his wallet like a claim check with no counter to present it at.

This is not unique to the Marshall Islands. It is the fundamental problem of every digital currency distribution, every CBDC pilot, every community currency project: **why would anyone accept it?**

El Salvador tried to answer this question with a mandate. The 2021 Bitcoin Law required all businesses to accept Bitcoin. It failed. By 2025, Bitcoin transactions had dropped to near zero outside the government's own Chivo wallet. Forced adoption without organic utility doesn't work.

The Bristol Pound (2012-2021) tried a different approach. Bristol's local currency was accepted by the city council for council tax payments. This was its most distinctive feature and a primary driver of merchant participation. If you owed taxes and held Bristol Pounds, you had a guaranteed use for them. Merchants who accepted Bristol Pounds could pay their tax bill instead of converting back to sterling.

The Bristol Pound still folded after nine years. It never reached the scale needed for long-term viability. But the insight was correct: **tax acceptance creates a floor demand that voluntary currencies lack.**

## The Tax Loop

The most powerful adoption mechanism for a digital currency is not a mandate. It is not a marketing campaign. It is a closed loop:

```
Government distributes tokens (UBI, benefits, salaries)
       |
       v
Citizens spend tokens at local vendors
       |
       v
Vendors accumulate tokens and remit them as tax
       |
       v
Government recirculates tokens as next period's distribution
       |
       v
[repeat]
```

Each step creates value:

- **Distribution** solves cold start. Everyone has tokens on day one. No chicken-and-egg problem.
- **Spending** creates local commerce. Tokens circulate, generating economic activity.
- **Tax acceptance** creates permanent demand. Tokens always have a floor use -- you can always pay the government with them.
- **Recirculation** makes the system self-sustaining. The government doesn't need to inject new money each quarter; it recirculates what it collected in taxes.

This is, in fact, how national currencies gained dominance historically. Governments imposed tax obligations payable only in the government's currency, forcing everyone to acquire and use that currency. The tax loop is not a novel theory. It is a description of how money has always worked. The question is whether it can work at the scale of a Pacific atoll.

## The Missing Piece: Local Commerce

The Marshall Islands has the distribution step. ENRA tokens arrive in wallets every quarter. What it doesn't have is the local commerce layer -- the place where Nalu can spend his tokens and Tia (the shopkeeper) can accept them.

This is where the architecture we built matters.

**Tia runs the only store on Likiep.** She stocks rice, canned goods, fuel, fishing line. She extends credit to most families, tracking debts in a ruled notebook. Some debts get repaid. Some don't. Tia absorbs the losses.

**With Assign Onward, Tia gets her own blockchain.** Tia's General Store (TGS) chain -- running on a Raspberry Pi in her store, connected to her satellite internet. One TGS credit equals one dollar of store goods. Her nephew Mako, an IT worker on Majuro, sets it up in a weekend and acts as the exchange agent bridging ENRA tokens to TGS credits.

Now when Nalu walks in needing a fishing reel ($35) and a bag of rice ($12), he sends 47 ENRA tokens from his wallet. Mako's exchange agent automatically credits Nalu with 47 TGS on Tia's chain. Tia sees the payment on her tablet. She hands Nalu his goods. For the first time, a retail purchase on Likiep has been recorded on a transparent, verifiable ledger.

**Tia's chain is independent.** It is not a sidechain. It is not a layer-2. It is not a smart contract on someone else's platform. Tia's chain represents Tia's store, Tia's inventory, Tia's credit relationships. If the government's ENRA system goes down, Tia's chain keeps running -- her customers can still buy goods with TGS credits, and she can settle up with ENRA later.

**Mako's exchange agent bridges the gap.** He holds inventory in both ENRA tokens and TGS credits. When Nalu spends ENRA, Mako receives ENRA and sends TGS. When Tia wants to convert accumulated TGS back to ENRA (to pay the supply boat, or to remit taxes), Mako does the reverse. He earns a small spread for this service. This is a human role, not a smart contract -- Mako's reputation as Tia's nephew and an IT professional on Majuro is his collateral.

## Closing the Loop

The Marshall Islands doesn't currently have a VAT, but the IMF and Pacific Financial Technical Assistance Centre have been pushing for broad-based VAT implementation in 2026. When it arrives, the loop can close:

1. Government distributes $200/quarter in ENRA tokens to every citizen
2. Citizens spend ENRA at Tia's store (via exchange agent)
3. Tia accumulates TGS credits, which she converts back to ENRA via Mako
4. Tia remits VAT in ENRA tokens to the government
5. Government recirculates collected ENRA as next quarter's UBI distribution

The tokens now have permanent velocity. Each dollar of UBI generates multiple transactions of economic activity before returning as tax revenue. The government gets something it never had before: a transparent audit trail of how UBI funds flow through the economy on remote atolls. This matters because the Compact Trust Fund is backed by U.S. taxpayer money -- demonstrating exactly where every dollar goes is a powerful accountability tool for maintaining donor confidence.

## What It Costs

This is not free. Here are the real infrastructure costs for running the system on Likiep:

| Item | Cost |
|------|------|
| Raspberry Pi + SSD + UPS | ~$150 one-time |
| Cloud backup VM (Majuro) | ~$10/month |
| Satellite internet (incremental) | ~$0 (Tia's existing service) |
| Validator anchoring | ~$2-5/month |
| **Total ongoing** | **~$15/month** |

On an atoll of 400 people doing 30-50 transactions per day, recording fees (tiny per-byte charges paid in retired shares) cover the cloud backup and validator costs. All ongoing infrastructure expenses are covered by transaction fees at this modest volume. No external subsidy is needed for the infrastructure itself.

What recording fees do *not* cover: Mako's initial hardware purchase, Mako's volunteer time for maintenance, and Tia's existing internet service. At pilot scale, Mako is doing this as a family favor and a bet on the future. If it works and expands to other atolls, his role grows into a paid IT support position.

There is a real dependency risk: if Mako loses interest or becomes unavailable, who maintains the Pi? On a remote atoll, there may not be another person with the technical skills. The cloud backup provides continuity if hardware fails, but it can't fix a software configuration problem remotely without someone on-site who knows what they're doing.

## The Low-Bandwidth Advantage

Most blockchain systems assume reliable broadband internet. On Likiep, the satellite link drops during storms, overloads during school hours, and costs more per megabyte than anywhere in the continental US.

A typical Assign Onward transaction is a few hundred bytes. At LoRa mesh speeds (22 kbps), that would be under one second to transmit. A full day's activity might be 50-100 transactions, totaling a few tens of kilobytes. The entire chain could sync between the Pi and a backup server on Majuro using the bandwidth of a single low-resolution photo.

In an emergency -- typhoon, cable cut, satellite failure -- a Meshtastic LoRa node ($30 hardware, battery or solar powered) on each islet could in principle relay transactions across the atoll entirely off-grid. Tia's store keeps operating while the rest of the world's digital payment infrastructure is down. When connectivity returns, the local chain syncs up automatically. LoRa mesh transport is not yet integrated into the software, but the wire format was designed from the beginning to support it -- every byte is compact enough for constrained radios, and the protocol requires no always-on connection.

This isn't an afterthought. The wire format was designed from the beginning for environments where every byte costs money and every connection is unreliable. The LoRa transport layer is a planned addition, not yet built.

## Where Else This Applies

Any country or territory that distributes benefits to a dispersed population with limited banking infrastructure:

- **Tonga** (remittances = 40% of GDP, 170 islands)
- **Samoa** (remittances = 30% of GDP)
- **Dominica** (already deployed the MLajan mobile wallet post-hurricane)
- **Vanuatu** (83 islands, 15% GDP from remittances)

In each case: government distribution seeds the economy with tokens, local vendor chains give the tokens somewhere to go, tax acceptance creates the demand that closes the loop. Whether the flywheel spins depends on whether each participant finds it easier than the status quo -- which, on islands that have managed with cash and notebooks for generations, is not guaranteed.

## What We Built

The core software exists. Seven Rust crates, 349 tests passing, MIT-licensed. Full protocol from genesis block through atomic multi-chain exchange. A simulation suite with agents that demonstrate the economic dynamics on a map. The recorder is designed to run on a Raspberry Pi (not yet field-tested on one). The wallet runs in a browser. M-Pesa and other fiat on/off-ramp integrations are not yet built -- the exchange agent role is implemented, but bridging to external payment systems would require additional development.

What doesn't exist yet: a deployment on an actual atoll, with actual fishermen and shopkeepers, in actual conditions of intermittent satellite connectivity and 90-degree heat. Nor does LoRa mesh transport or fiat payment integration exist in code yet -- those are designed for but not implemented. The software is the easy part. Finding the Tia and the Mako -- the shopkeeper willing to try something new and the IT person willing to spend a weekend setting it up -- that's the hard part.

If you work in Pacific island development, digital financial inclusion, or UBI implementation, and this matches a problem you've seen -- the code is open, the architecture is documented, and we'd like to hear from you.

GitHub: [assignonward/aosuite](https://github.com/assignonward/aosuite)

---

*Assign Onward is open source (MIT license). There is no token sale, no ICO, no company. The Marshall Islands research is based on publicly available information about the ENRA program; we have no affiliation with the Marshall Islands government or the Lomalo wallet.*
