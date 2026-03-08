# Transparent Cooperative Accounting Without Trusting the Treasurer

*Target audience: Cooperative technology community (platform.coop, Open Food Network), agricultural development organizations (FAO, UNCDF, CGAP), and microfinance practitioners. Lead with the trust problem, show the chain-per-farmer model.*

---

Benson is the treasurer of the Riuki Farmers' Cooperative in Kiambu County, Kenya. He is honest. He keeps careful records in a ruled notebook. He has held the position for seven years because nobody else wants the job and because the cooperative's 60 members trust him -- mostly.

The problem is not Benson. The problem is the notebook.

When Amara delivers 200 kilos of tomatoes to the cooperative truck and the truck returns from Wakulima market with KES 18,000 for the load, Amara has no way to verify what price was actually achieved, what the transport cost was, or whether her share of the proceeds is correct. She trusts Benson, mostly, but she's heard stories. A 2023 survey by the Kenya National Bureau of Statistics found that 34% of Sacco members reported concerns about financial transparency. The Sacco Societies Regulatory Authority deregistered over 200 Saccos between 2020 and 2024 for governance failures, many involving misappropriation of member funds.

This is not about catching thieves. It's about removing the *possibility* of theft from the equation entirely. When the books are transparent and every member can verify every transaction independently, the treasurer doesn't need to be trusted -- and the treasurer doesn't need to worry about being suspected.

## The Architecture: A Federation of Chains

Most blockchain proposals for agriculture put all transactions on one shared chain. This is wrong for cooperatives. A cooperative is a federation of independent producers who share resources voluntarily. The accounting system should mirror this structure.

In the Assign Onward model, the Riuki Cooperative runs three types of chains:

**1. Individual farmer chains.** Each member gets their own blockchain. Amara's chain is AFT: Amara's Farm Tomatoes. At the start of the season, Amara issues shares representing her expected harvest -- say, 2,000 kilos. The shares don't promise delivery on a specific date; they represent her production capacity for the season. Kwame has his cabbage chain. Fatuma has her spinach chain. Peter has his maize chain. Each farmer controls their own chain. Nobody can issue shares on Amara's chain except Amara.

**2. The cooperative chain.** RCC: Riuki Cooperative Credits represents the cooperative's pooled resources and shared services. When members contribute produce for the cooperative truck to sell at Wakulima, the transaction is recorded on both the individual farmer's chain (outgoing tomatoes) and the RCC chain (incoming produce for sale). When the truck returns with payment, the distribution is recorded: exactly how much was sold, at what price, minus verifiable transport costs, with each member's share calculated transparently.

**3. An exchange chain.** MPC: M-Pesa Credits bridges between the on-chain cooperative economy and Kenya's M-Pesa mobile money system. Wanjiku, Amara's daughter who works as a software developer in Nairobi, acts as the initial exchange agent, converting between MPC and M-Pesa at par.

Sixty farmer chains, one cooperative chain, one exchange chain -- all running on a single Raspberry Pi with a cloud backup on a KES 500/month VM in Nairobi.

## A Season on the Chain

**Planting.** The cooperative buys bulk seeds and fertilizer. The purchase is recorded on the RCC chain: total cost, supplier, quantity. Each member's advance is recorded on their individual chain: Amara received KES 5,000 worth of inputs, debited against her future harvest. No ambiguity. No disputed notebook entries.

**Harvest.** Ouma, the collection point operator, weighs tomatoes as they arrive. 180 kilos from Amara, Grade A, received 7:15 AM Thursday. Recorded on Amara's AFT chain. The transaction is small -- a few hundred bytes -- compact enough to transmit even over spotty Safaricom coverage.

**Market sale.** The cooperative truck goes to Wakulima. The buyer pays KES 15 per kilo for 1,200 kilos from six farmers. KES 18,000 total. Transport cost: KES 2,000 (documented, on-chain). Net proceeds: KES 16,000, distributed proportionally to each farmer's contribution. Amara contributed 180 of 1,200 kilos (15%), so she receives KES 2,400. This arithmetic is on-chain, verifiable by any member with a phone.

Compare this with the notebook system: Benson writes "KES 18,000 from Wakulima" and distributes cash. Nobody can verify the sale price independently. Nobody knows if the transport cost was really KES 2,000 or KES 3,000. Nobody can check the percentage calculation. The system works as long as everyone trusts Benson, and it falls apart the moment anyone doesn't.

## Diaspora Investment

James is Amara's brother. He works in Dubai and sends money home -- part of the $205 billion in on-chain crypto value flowing to Sub-Saharan Africa annually, dominated by remittances. Under the old system, James sends KES 30,000 via M-Pesa. Amara uses it to buy seeds and fertilizer. James has no visibility into whether the money was invested productively or spent on other needs. He trusts his sister, but the World Bank estimates that only 10-15% of remittances to Africa are used for productive investment; the rest goes to consumption.

With Amara's AFT chain, James can see exactly what his sister produced last season, what prices she achieved, and how reliable she was in meeting commitments. He buys KES 30,000 worth of shares in Amara's next season -- pre-purchasing tomatoes at a discount. Amara gets working capital before planting. James gets a claim on a portion of the harvest. The profit (or loss) is transparent on-chain.

This is not charity. It's a documented investment in a verifiable productive enterprise. Remittances become productive capital instead of opaque consumption transfers. Families want to invest in each other; they just lack the transparent, accountable vehicle.

## When the Harvest Fails

In February, a hailstorm destroys 60% of Amara's crop. The damage is documented: Ouma inspects the plot and records the loss on Amara's AFT chain, with photos that could be stored as separable data items (the protocol supports binary attachments; a photo upload UI is planned but not yet built). James, watching from Dubai, can see exactly what happened. The shares representing the destroyed crop lose value -- there's less harvest to redeem them against. But the loss is documented, transparent, and verifiable.

Compare this with telling Benson "the harvest was bad" and Benson writing a number in his ledger. No verification, no documentation, no basis for crop insurance claims or disaster relief.

Over time, multi-season on-chain histories of both good and bad harvests become the foundation for actuarial assessment. An insurance provider can calculate risk from actual historical records rather than regional averages. Affordable crop insurance for smallholders currently covers less than 3% of African farmers. Transparent production data is the prerequisite for changing that.

## The Credit-Building Feedback Loop

This is where the cooperative model generates compounding returns:

1. Farmer uses chain for one season -- transparent production record exists
2. Record demonstrates reliability -- microfinance institution offers small loan
3. Loan + chain for second season -- larger record, better terms
4. After several seasons -- portable, verifiable financial identity

Amara has never had a bank loan. Kenya Commercial Bank wants collateral, financial statements, documented income. Amara has a half-acre plot (no title deed -- it's family land), a handful of M-Pesa receipts, and Benson's word. After two seasons on-chain, she has a documented record of production, sales, and income that any lender can verify. She can finally apply for a loan.

Traditional credit bureaus don't reach these farmers. Cooperative chain histories could serve as portable financial reputation. Not a credit score imposed by an opaque algorithm, but raw data: "Amara produced X kilos over Y seasons, achieved Z prices, repaid all advances on time." The lender can draw their own conclusions.

## Infrastructure Costs

| Item | Cost | Who Pays |
|------|------|----------|
| Raspberry Pi + SSD + case | KES 10,000 (~$80) one-time | Wanjiku or cooperative fund |
| Solar battery / UPS | KES 5,000 (~$40) one-time | Same |
| Safaricom data bundle | KES 1,000/month (~$8) | Recording fees |
| Cloud backup VM (Nairobi) | KES 500/month (~$4) | Recording fees |
| Validator anchoring | KES 300-600/month (~$2-5) | Recording fees |
| **Total ongoing** | **~KES 2,000/month ($15)** | |

With 60 farmers making several transactions each per week, the cooperative generates hundreds of on-chain transactions per month. Recording fees from this volume cover the data bundle, cloud backup, and validator costs. All ongoing infrastructure expenses are paid by transaction fees once volume reaches steady state.

What recording fees do *not* cover: Wanjiku's volunteer labor (she's doing this for family, and because she thinks it could become a business), smartphone costs for members (most already have phones for M-Pesa), and the initial hardware investment (which the cooperative could fund from its revolving fund or an NGO partner might provide).

The biggest operational risk is dependency on a single technical champion. If Wanjiku stops volunteering, the cooperative needs another technically capable person -- and rural Kiambu is not Nairobi. A scalable deployment would need a support organization that trains and supports local champions across multiple cooperatives simultaneously.

## Scale

Kenya has over 15,000 registered Saccos. Sub-Saharan Africa has 450 million smallholder farmers. The gap between "transparent accounting tool exists" and "450 million farmers use it" is enormous.

But the gap between "no tool exists" and "one cooperative uses it" is crossable. Each link in the chain matters independently: transparent accounting builds trust whether or not it ever leads to crop insurance. Fair market prices are valuable whether or not diaspora investment follows. Credit history helps whether or not it scales beyond Kiambu.

If members don't find the transparent ledger easier than Benson's notebook -- because the app is confusing, because the phone battery is dead at the collection point, because Safaricom coverage is spotty that day -- then the first link breaks and everything downstream is moot. A pilot would test whether the first few links hold under real farming conditions, not whether the entire chain works end-to-end from day one.

## The Software

The core software exists. Seven Rust crates, 255 tests (187 Rust + 68 PWA), MIT-licensed. A simulation suite shows cooperative-style multi-chain dynamics running on a map with time controls. The recorder is designed to run on a Raspberry Pi (not yet field-tested on one). The wallet runs in a browser. M-Pesa integration would use the existing M-Pesa API through the exchange agent role -- the exchange agent architecture is built, but the M-Pesa bridge is not yet implemented. Photo attachments for crop documentation are supported by the protocol (separable data items with binary blobs) but the upload UI is not yet built.

If you work with farming cooperatives, Saccos, agricultural finance, or cooperative technology -- and you recognize the trust problem described here -- the architecture is documented, the code is open, and we'd like to hear from you.

GitHub: [assignonward/aosuite](https://github.com/assignonward/aosuite)

---

*Assign Onward is open source (MIT license). There is no token sale, no company, no platform fee. The Kenya research draws on publicly available data from the Kenya National Bureau of Statistics and Sacco Societies Regulatory Authority.*
