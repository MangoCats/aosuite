# What If Every Beach Vendor Had Their Own Blockchain?

*Target audience: General tech (Hacker News, dev.to, Medium). Lead with the simulation, ground in real economics.*

---

Bob sells curry goat on Sandy Ground beach in Anguilla. He does about $200 a day in sales when a cruise ship is in port. He has no credit card terminal -- the monthly fee alone would eat his margin. He takes cash only, keeps receipts in a cigar box, and has never been able to get a bank loan because he can't document his income.

Patrice sells handmade jewelry from a blanket ten meters away. Same story. Lucia rents kayaks down by the water. Same story. Sharon sells fruit smoothies. Same.

These four vendors collectively move maybe $600 a day in tourist spending. Visa and Mastercard will never notice them. They are too small, too informal, too cash-dependent to matter to the global payments industry. And yet they *are* the economy of Sandy Ground beach.

## The Simulation

We built a simulation of Sandy Ground's economy. Not a whitepaper. Not a pitch deck. A running system.

Twelve independent software agents -- vendors, tourists, exchange agents, a validator -- trade across seven blockchains on a map of Anguilla. Bob's Curry Goat chain (BCG) issues shares representing plates of food. Rita's Mango Futures chain (RMF) represents seasonal fruit. Dave's E-Bikes chain (DEB) represents rental hours. Each vendor controls their own chain. Nobody else can issue shares on Bob's chain. Nobody can inflate Rita's mango supply.

Charlie, a launch captain who ferries tourists from cruise ships to shore, runs an exchange. He issues Charlie's Cruise Credits (CCC), buys inventory on vendor chains, and sells to tourists. When Alice steps off the launch, she buys CCC from Charlie with her credit card. She spends CCC at Bob's stall, at Patrice's blanket, at Lucia's kayak stand. At the end of the day, vendors cash out through Charlie.

Ziggy runs a competing exchange. He undercuts Charlie's rates. The simulation shows their prices converging as they compete for the same customers -- real market dynamics emerging from simple agent rules.

You can watch all of this happen on a map, in real time, with time controls that let you speed up, slow down, or scrub through the history. Click any agent to see their wallet, their transaction log, their perspective on the economy.

## What's Actually Different

Every blockchain project claims to fix payments. Most of them are solving a problem that Visa already solved, badly, while adding speculation on top. This is different in a few specific ways:

**One chain per vendor, not one chain for everything.** Bob's Curry Goat chain is *Bob's*. It represents his food, his production capacity, his business reputation. It's not a token on someone else's platform. It's not a smart contract on Ethereum. It's a standalone blockchain that Bob could run on a Raspberry Pi in his kitchen if he wanted to. This matters because it means Bob's business records can't be censored, inflated, or shut down by a platform operator. If the exchange agent disappears, Bob still has his chain and his transaction history.

**Shares represent real goods, not speculative tokens.** One BCG coin equals one plate of curry goat. The shares have value because Bob makes good curry goat and people want to eat it. There is no secondary market. There is no price speculation. There is no "BCG to the moon." When shares expire (configurable -- think of it like a gift card expiration), the value returns to the community of remaining shareholders rather than being lost.

**Exchange agents are humans with skin in the game, not automated market makers.** Charlie bridges the gap between tourist credit cards and local vendor chains. He earns a spread (about 5%) for providing this service. He takes the credit card risk on the tourist side. He holds inventory in vendor chains. If he cheats, vendors stop selling to him and tourists stop buying from him. His reputation is his business. This is how currency exchange has worked in tourism economies for decades -- the only difference is that the transactions are now transparent and auditable.

**No proof of work. No mining. No energy waste.** The recorder (the server that hosts chains) is a lightweight Rust application running on commodity hardware. The entire Sandy Ground economy runs on a Raspberry Pi with a cloud backup. Recording fees -- tiny per-byte charges paid in retired shares -- cover the infrastructure costs. No external subsidy needed once transaction volume reaches a modest level.

**Designed for low bandwidth.** A typical transaction is a few hundred bytes. The wire format was designed for LoRa mesh networks running at 22 kbps, not for fiber-optic data centers. The entire day's transactions for a beach market fit in the bandwidth of a single low-resolution photo. This isn't an afterthought -- it's a core design requirement for deployment in places where connectivity is expensive and unreliable.

## The Economics

Let's be specific about costs:

**What vendors pay now (credit cards):**
- 3.2% per transaction
- EC$50/month terminal fee
- 3-5 day settlement delay
- Chargeback risk (average cost: $191 per incident)
- Most beach vendors can't afford this, so they're cash-only

**What vendors would pay with this system:**
- ~0.1% recording fee per transaction
- ~5% exchange spread to Charlie (negotiable, drops with competition)
- No monthly fixed costs
- Instant settlement
- No chargebacks (transactions are final)

The exchange spread is higher than a card fee for a single transaction. But it includes the currency exchange service (tourists arriving with USD, vendors pricing in EC$), and it has no fixed monthly component. For a vendor doing $50/day, the card terminal's $50/month fee alone is a 3.3% tax before a single transaction is processed. And vendors who couldn't accept cards at all -- Patrice, Lucia, Sharon -- now have access to tourist spending they were previously shut out of.

**Infrastructure costs:**
- Raspberry Pi + SSD + UPS: ~$150 one-time
- Cloud backup: ~$10/month
- Validator anchoring: ~$2-5/month
- Domain name: ~$12/year
- Total ongoing: ~$15-20/month, covered by recording fees at modest volume

Someone has to front the Pi and the first few months of cloud costs. After that, the system pays for itself through transaction fees. This is not free -- but it's cheaper than the monthly credit card terminal fee that most beach vendors can't afford in the first place.

## What It's Not

This is not a cryptocurrency. BCG tokens are not traded on exchanges. They're not an investment vehicle. They're closer to a gift card for Bob's curry goat stand -- a prepaid credit redeemable for a specific good from a specific vendor.

This is not DeFi. There are no smart contracts, no yield farming, no liquidity pools, no governance tokens. The "protocol" is: Bob makes food, issues shares representing food, people buy shares, people eat food.

This is not a platform. There is no Assign Onward Inc. taking a cut. The software is MIT-licensed. Anyone can run a recorder. Anyone can run an exchange. The protocol is open. If Bob doesn't like Gene's recording service, he can move his chain to a different recorder.

This is not going to replace Visa. It's not trying to. It's for the vendors that Visa will never serve -- the ones whose $200/day in sales doesn't justify the infrastructure costs of the global payments network. The ones keeping receipts in cigar boxes.

## Try It

The simulation runs live at [link to hosted demo]. Watch twelve agents trade across seven chains on a map of Anguilla. Click any agent to see their wallet. Scrub through time to watch market dynamics emerge. The entire system -- agents, recorder, exchange logic, viewer -- is open source and written in Rust.

If you know a beach vendor, a market stallholder, or a tourism economy that could use transparent, low-cost payment infrastructure -- this is what we built it for.

GitHub: [assignonward/aosuite](https://github.com/assignonward/aosuite)

---

*Assign Onward is an open-source project. The software is MIT-licensed. There is no token sale. There is no ICO. There is no company. The code is the product.*
