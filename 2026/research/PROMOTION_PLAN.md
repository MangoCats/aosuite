# Assign Onward: Low-Capital Promotion Plan

*March 2026. Finding enthusiast adopters and giving aosuite visibility to people able and willing to deploy it.*

---

## Core Assets Available Now

Before spending anything, take stock of what already exists:

1. **Working software.** Phases 0-6 complete: 7 Rust crates, 187 Rust tests + 68 PWA tests, full protocol from genesis through atomic multi-chain exchange.
2. **Live simulation suite.** Sims A-D: agent framework, viewer PWA with map view, time controls, audit overlay, adversarial agents. The `island-life.toml` scenario runs the full IslandLife cast on Anguilla geography with competing exchange agents and realistic market dynamics.
3. **Three deployment narratives.** Tourism Vendors (Sandy Ground), Island UBI (Likiep), Farming Cooperatives (Riuki) -- grounded in real-world research, honest about costs and risks.
4. **Open source repo.** GitHub (assignonward/aosuite) + GitLab mirror. MIT license.
5. **Documentation site.** mangocats.com/ao/ with 2026 review and all deployment stories.

The sim viewer is the most powerful promotional tool. A running `island-life` simulation shows the concept better than any slide deck or whitepaper ever could.

---

## Strategy: Show, Don't Pitch

Blockchain projects are drowning in pitches. The space is saturated with whitepapers promising revolution. What's rare is *working software that demonstrates a concrete use case visually*. The sim viewer does this. Every promotional action should lead people to watch the simulation run, not read a document.

---

## Channel 1: Hosted Demo (Cost: ~$5/month)

**Deploy the sim viewer as a public web demo.**

Run `island-life.toml` continuously on a cheap VPS (or even a free-tier instance). Visitors land on the map view showing Anguilla, watch agents trade in real time, click into any agent to see their wallet, transaction history, and app screen. Time controls let them scrub through hours of simulated economic activity in minutes.

This is the front door for everything else. Every link shared, every talk given, every forum post points here.

**Actions:**
- Deploy sim viewer + sim coordinator to a publicly accessible URL (a subdomain of mangocats.com or a dedicated domain)
- Add a landing overlay explaining what the viewer shows: "This is a simulation of 12 independent agents -- vendors, tourists, exchange agents, a validator -- trading across 7 blockchains on a Caribbean island. Everything you see is running on real Assign Onward protocol software."
- Add clickable annotations on the map: "Click Bob to see his curry goat chain," "Click Charlie to see how exchange agents bridge currencies," "Click Victor to see the audit overlay"
- Link to deployment stories from the viewer: "This is a simulation. Here's how it would work in practice" -> TourismVendors.html, IslandUBI.html, FarmingCooperatives.html

**Future sim enhancement:** A "guided tour" mode that walks a first-time viewer through the simulation step by step, pausing at key moments to explain what's happening and why it matters.

---

## Channel 2: Targeted Community Engagement (Cost: $0)

The goal is not mass awareness. It's finding the 50-100 people worldwide who have the motivation and capability to run a pilot. These people are in specific, reachable communities.

### 2a: Cooperative Technology / Platform Cooperativism

**Who:** People building technology for cooperatives, credit unions, mutual aid networks. They already understand federated governance and are frustrated with existing tools.

**Where:**
- [platform.coop](https://platform.coop) community and mailing lists
- CoopTech Slack/Discord channels
- Internet of Ownership network
- Open Food Network community (open source food cooperative platform)
- Cooperative Development Foundation forums

**Message:** "We built open-source software for cooperative accounting -- each member runs their own chain, the cooperative chain records shared transactions, everything is transparent and auditable. Here's a live simulation of it running. Here's what it would look like for a Kenyan farming Sacco." Link to demo + FarmingCooperatives.html.

### 2b: Development Economics / Financial Inclusion

**Who:** Researchers, NGO technologists, and policy people working on financial inclusion in Pacific islands, Caribbean, and Sub-Saharan Africa. They know the problems intimately and are looking for tools.

**Where:**
- UNCDF (UN Capital Development Fund) -- they run the Pacific Financial Inclusion Programme
- CGAP (Consultative Group to Assist the Poor) -- World Bank fintech for development
- Alliance for Financial Inclusion mailing lists
- GSMA Mobile Money programme community
- Pacific Islands Forum Secretariat ICT working groups
- Caribbean Fintech Association

**Message:** "The Marshall Islands launched blockchain-based UBI in December 2025. The tokens have nowhere to go locally. We built software that creates the local commerce layer -- vendor chains that give UBI tokens utility, with a tax loop that closes the adoption cycle. Here's a simulation. Here's the detailed story for a Pacific atoll." Link to demo + IslandUBI.html.

### 2c: Caribbean / Pacific Tech Communities

**Who:** Developers and tech entrepreneurs in small island economies who could be the "Wanjiku" or "Mako" in the deployment stories -- technically capable people with local connections.

**Where:**
- Caribbean Developers community (caribbeandevs.com)
- Silicon Caribe
- Pacific Islands ICT Association
- Samoa Tech meetups
- Marshall Islands community on Reddit/Facebook (small but real -- diaspora in Arkansas, Hawaii, Oregon)
- Anguilla (specifically) tech community -- the island in the simulation

**Message:** "We simulated your island's economy running on independent blockchains. Beach vendors, exchange agents, tourists -- all trading on transparent ledgers with fraction-of-a-percent fees instead of 3-5% card processing. The simulation uses real Anguilla geography. Take a look." Link to demo.

### 2d: Crypto-Skeptic Technologists

**Who:** Software engineers who are interested in distributed systems but turned off by speculation, proof-of-work waste, and crypto hype. They're the builders who could contribute or deploy.

**Where:**
- Hacker News (strategic, well-timed posts -- not spam)
- Lobste.rs
- r/programming, r/rust (for the implementation)
- Martin Kleppmann's community (author of "Designing Data-Intensive Applications" -- his work on local-first software overlaps philosophically)
- Secure Scuttlebutt / local-first software community

**Message:** "No proof of work. No speculation. No token launch. Millions of tiny independent blockchains, each representing a real business -- a curry goat stall, a bike rental shop, a farming cooperative. Runs on a Raspberry Pi. Written in Rust. Here's a live simulation with 12 agents trading across 7 chains." Link to demo + GitHub.

---

## Channel 3: Content That Finds Its Audience (Cost: $0)

### 3a: Blog Posts / Articles

Write 3-5 focused articles, each targeting a specific audience. Publish on the project site and cross-post to relevant platforms.

**Article 1: "What If Every Beach Vendor Had Their Own Blockchain?"**
Target: General tech audience (Hacker News, Medium, dev.to). Lead with the Sandy Ground simulation. Contrast with existing payment infrastructure costs. Link to live demo.

**Article 2: "The Tax Loop: How UBI Tokens Become a Local Economy"**
Target: Development economics, UBI advocates (Scott Santens' community, r/BasicIncome, BIEN network). Lead with Marshall Islands ENRA, explain the gap, show how vendor chains close it. Link to IslandUBI.html.

**Article 3: "Transparent Cooperative Accounting Without Trusting the Treasurer"**
Target: Cooperative tech community, agricultural development. Lead with the trust problem in Kenyan Saccos, show the chain-per-farmer model. Link to FarmingCooperatives.html.

**Article 4: "Building a Blockchain Simulator in Rust"**
Target: Rust developers, simulation enthusiasts. Technical deep-dive on the agent framework, TOML scenario configuration, the viewer PWA architecture. Link to GitHub sims code.

**Article 5: "Why We Dropped Proof of Work, Token Launches, and DeFi"**
Target: Crypto-skeptics who might build if the philosophy resonates. Explain what AO is *not* and why. The anti-pitch.

### 3b: Conference Talks (Free to Submit)

Submit talks to conferences where the target audience gathers. The live demo is the talk -- run the simulation on screen and narrate the economic dynamics as they unfold.

**Priority conferences:**
- **RustConf / EuroRust** -- "A Caribbean Economy in 7,000 Lines of Rust" (technical, implementation-focused)
- **Platform Cooperativism Consortium annual conference** -- "Blockchain for Cooperatives: A Simulation" (cooperative governance focus)
- **Pacific Islands Forum ICT Ministerial** -- "Local Commerce Layers for Digital Currency Distribution" (policy focus)
- **Caribbean Fintech Conference** -- live demo with Anguilla geography
- **FOSDEM** (free, Brussels) -- open source track, "Federated Microblockchains"
- **Strange Loop** / **local meetups** -- lower barrier, good for testing the talk

### 3c: Video Walkthrough (Cost: $0)

Record a 5-minute screen capture of the sim viewer running `island-life.toml`. Narrate what's happening: "Here's Bob opening his curry goat stall... Alice just arrived by cruise ship and is buying BCG through Charlie's exchange... watch the transaction arc on the map... now let's click into Charlie and see his multi-chain portfolio..." Post to YouTube. This becomes the shareable asset for all channels.

A second video for the `audit-adversarial.toml` scenario: "Watch Mallory try to double-spend... the recorder rejects it... Victor's validator catches the attempt... here's what the audit overlay looks like."

---

## Channel 4: Direct Outreach to Potential Champions (Cost: $0)

The deployment stories each depend on a "champion" -- a technically capable person with local connections (Wanjiku, Mako, Gene). Finding these people is the highest-value promotional activity.

**Approach:**
1. Identify 20-30 specific individuals or small organizations working at the intersection of technology and the target communities
2. Send a personalized message (not a mass email) with the live demo link and the relevant deployment story
3. Ask: "Does this match a problem you've seen? Would you want to try it?"

**Who to find:**
- The person who built the Lomalo wallet for Marshall Islands ENRA (they know the gap firsthand)
- Developers at Stellar who worked on the USDM1 integration (they might see the complementary layer)
- The Caribbean Developers community organizer
- Agricultural extension officers in Kenya who work with Saccos and have technical backgrounds
- The Open Food Network team (they already build cooperative commerce tools)
- Encointer project contributors (community currency on Substrate -- different approach, shared problem space)
- Bristol Pound Legacy people (they lived through the tax-loop experiment and know why it ultimately failed)

---

## How Sims Drive Each Channel

The sims are not just a development tool -- they're the primary promotional asset. Every channel above leads back to the running simulation.

### Current Sims (A-D) Promotional Value

| Sim | What It Shows | Who It Convinces |
|-----|--------------|-----------------|
| `island-life.toml` | Full Caribbean vendor economy with competing exchanges | Tourism/Caribbean audience, general tech |
| `price-war.toml` | Two exchanges competing on the same pair, price discovery | Economists, market design people |
| `audit-adversarial.toml` | Attacker agents rejected, validator catches everything | Security-minded engineers, skeptics |
| `exchange-3chain.toml` | Cross-chain trading across 3 vendor chains | People who ask "but how do different chains interoperate?" |

### Future Sim Development for Promotion

**Sim-E (Phase 6 dependency: CAA atomic exchange)** is already on the roadmap and would add:
- `island-life-full.toml` -- the canonical demonstration scenario, designed to tell the complete IslandLife story through simulation data. This becomes *the* demo.
- `chaos.toml` -- resilience under failure. Powerful for convincing skeptics that the system handles real-world conditions.

**Beyond Sim-E, new scenarios purpose-built for promotion:**

**`likiep-ubi.toml` -- Island UBI scenario**
- Agents: Nalu (fisherman), Tia (shopkeeper/chain operator), Mako (exchange agent bridging ENRA tokens to TGS), government distributor (issues quarterly UBI), 10-15 atoll residents
- Geography: Likiep Atoll, Marshall Islands (real lat/lon for the islets)
- Dynamics: Government distributes UBI -> residents spend at Tia's store -> Tia accumulates tokens -> Tia remits tax payment -> government recirculates. The tax loop running visually.
- Demonstrates: The complete adoption flywheel from IslandUBI.html. Low transaction volume (30-50/day) on a single Pi. Satellite outage simulation (agents queue transactions, sync when connectivity returns).
- **Promotional power:** Show this to anyone working on Pacific island digital finance and they'll immediately recognize the problem it solves.

**`riuki-cooperative.toml` -- Farming Cooperative scenario**
- Agents: Amara (tomatoes), Kwame (cabbage), Fatuma (spinach), Peter (maize), Benson (treasurer/coordinator), Ouma (collection point operator), Wanjiku (exchange agent bridging MPC to M-Pesa), James (diaspora investor in Dubai)
- Geography: Kiambu County, Kenya (Riuki village, Wakulima market in Nairobi, collection points)
- Dynamics: Planting season advances -> harvest deliveries recorded -> cooperative truck sells at market -> transparent profit distribution -> diaspora investment -> bad harvest event
- Demonstrates: The complete cooperative accounting cycle from FarmingCooperatives.html. Individual farmer chains + cooperative chain. Transparent middleman pricing. Diaspora investment flow.
- **Promotional power:** Show this to cooperative technology people and agricultural development organizations.

**`sandy-ground-tourist.toml` -- Tourism onboarding scenario**
- Focused version of island-life: just the tourist experience. Alice arrives, discovers vendors via map, buys CCC from Charlie, pays Bob, pays Patrice, pays Lucia. 90-second cycle matching the "launch ride" onboarding from TourismVendors.html.
- Viewer shows Alice's AOE screen throughout -- what the tourist actually sees on their phone.
- **Promotional power:** The most digestible demo for a general audience. "Watch a tourist's entire payment experience from arrival to departure."

**`connectivity-stress.toml` -- Low-bandwidth resilience demo**
- Simulates intermittent connectivity: agents go offline, queue transactions, reconnect, sync. Shows that the system works in the conditions described in the deployment stories (satellite outages, spotty mobile data).
- **Promotional power:** Directly addresses the "but what about connectivity?" objection that anyone familiar with Pacific islands or rural Africa will immediately raise.

---

## Sequencing

**Month 1: Foundation**
- Deploy hosted sim demo (island-life.toml running publicly)
- Record the 5-minute video walkthrough
- Write Article 1 ("Every Beach Vendor Had Their Own Blockchain")
- Post to Hacker News, r/rust, r/programming

**Month 2: Targeted Outreach**
- Write Articles 2 and 3 (UBI tax loop, cooperative accounting)
- Post to target community forums (platform.coop, CGAP, Caribbean Developers)
- Begin direct outreach to 10 identified potential champions
- Submit talk proposals to 2-3 conferences

**Month 3: Deepen**
- Write Articles 4 and 5 (Rust technical, anti-pitch)
- Develop `likiep-ubi.toml` scenario (if base code supports it)
- Follow up on outreach responses
- Engage in discussions that Articles 1-3 generated

**Ongoing:**
- Respond to GitHub issues and discussions from interested developers
- Iterate on the hosted demo based on what questions people ask
- Build new scenarios as promotion reveals which stories resonate most

---

## What Success Looks Like

This plan does not aim for mass adoption or viral growth. It aims for:

- **5-10 serious GitHub contributors** who understand the architecture and want to build on it
- **2-3 potential deployment partners** -- a Caribbean tech entrepreneur, a Pacific island IT person, a cooperative technology organization -- who see a match with a real problem they're already working on
- **1 pilot conversation** -- someone says "I want to try this on my island / at my cooperative / with my vendors" and has the capability to follow through

The simulation is the hook. The deployment stories provide context. The open-source code provides credibility. The honest cost accounting (infrastructure tables in every story) provides trust. The goal is not to convince the world -- it's to find the handful of people who were already looking for something like this and didn't know it existed.

---

## What This Plan Does NOT Include

- **Paid advertising.** Not cost-effective for a niche technical product targeting a small audience.
- **Token launch / ICO.** Antithetical to the project's philosophy. No speculation, no fundraising via token sales.
- **Partnership with existing crypto exchanges.** AO chains are not tradeable cryptocurrencies. Listing on Coinbase is not the goal.
- **Hiring a marketing firm.** The budget is zero. The audience is technical. Authenticity matters more than polish.
- **Social media presence management.** No Twitter/X account posting daily. Quality content on relevant platforms when there's something worth saying.

---

## Budget Summary

| Item | Cost | Notes |
|------|------|-------|
| VPS for hosted demo | ~$5-10/month | Smallest instance that runs the sim |
| Domain (if new) | ~$12/year | Or use existing mangocats.com subdomain |
| Conference travel | $0 (remote) or variable (in-person) | Submit to conferences with virtual options first |
| Content creation | $0 | Written by project contributors |
| Video recording | $0 | Screen capture + narration |
| **Total ongoing** | **~$5-10/month** | |

Everything else is time, not money.
