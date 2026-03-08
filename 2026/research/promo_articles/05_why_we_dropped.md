# Why We Dropped Proof of Work, Token Launches, and DeFi

*Target audience: Crypto-skeptic engineers who are interested in distributed systems but turned off by speculation and waste. The anti-pitch. (Hacker News, Lobste.rs, r/programming, local-first software community.)*

---

We started designing this system in 2018. The original documents specified RSA-3072, brainpool-256 ECDSA, and a custom big-integer library binding GMP through C++. There was no proof of work, no token sale, and no plan for one. The project stalled in 2021 with ~15,600 lines of C++ implementing the serialization layer and nothing else.

In 2026, we rewrote everything in Rust. Along the way, we made a series of decisions about what to leave out. These decisions define the project more than what we included.

## No Proof of Work

Proof of work solves a specific problem: how do you achieve consensus among anonymous, untrusted parties who might be Sybil attackers? The answer is: make it expensive to participate, so that attacking the network costs more than it's worth.

We don't have that problem. Our chains are not anonymous. Bob's Curry Goat chain has one recorder, operated by Gene, who lives three houses down from Faythe. Amara's tomato chain has one recorder, operated by the Riuki Cooperative, run on a Raspberry Pi in the meeting hall. The recorder is a known entity with a reputation. If Gene starts censoring transactions or rewriting blocks, the validator catches it, the community notices, and Bob moves his chain to a different recorder.

The trust model is: the recorder is a service provider, like a web host. You trust your web host not to delete your files, and if they do, you have backups and can switch providers. You don't need to burn gigawatts of electricity to make sure your web host is honest.

A chain in this system is a single-writer append-only log, signed by the blockmaker key. There's no mining race because there's no competition for who writes the next block -- the recorder does. Validation is separate: anyone can verify the hash chain and signatures independently. If the recorder misbehaves, the evidence is cryptographic and permanent.

This means the entire Sandy Ground beach economy -- seven chains, twelve agents, hundreds of transactions per day -- runs on a Raspberry Pi 5 drawing 5 watts. Not a data center. Not a GPU farm. A $80 single-board computer in a closet.

## No Token Sale

There is no AO token. There is no ICO. There is no pre-mine. There is no governance token. There is no utility token. There is no security token.

Each chain has its own shares. Bob's Curry Goat shares represent plates of curry goat. Rita's Mango Future shares represent seasonal fruit. These shares have value because they're redeemable for specific real-world goods from specific real-world people. They are not tradeable on Coinbase. They are not listed on Uniswap. They have no ticker symbol on CoinGecko.

This is a deliberate architectural choice, not an oversight. The moment you create a generic platform token, you create a speculative asset. Speculators show up. Price volatility follows. The community of people who want to buy curry goat gets drowned out by the community of people who want to flip curry goat tokens for a profit. The tail wags the dog.

Every blockchain project that launched a token eventually discovered that the token's speculative dynamics dominate the system's utility dynamics. We decided to skip that part entirely.

The project is funded by exactly zero dollars of token-sale money. The software is MIT-licensed. If someone wants to deploy it, they buy a Raspberry Pi.

## No Smart Contracts

There is no virtual machine. There is no scripting language. There is no Turing-complete execution environment. There are no decentralized applications running on a world computer.

Transactions in this system do one thing: transfer shares from givers to receivers, with a recording fee deducted. That's it. The "business logic" is: Alice gives Bob 12 CCC, Bob gives Alice a plate of curry goat. The plate of curry goat is off-chain. The 12 CCC transfer is on-chain. The recording fee (a few fractions of a cent) goes to the chain's infrastructure costs via share retirement.

Atomic multi-chain exchange (our most complex feature) uses a Conditional Assignment Agreement with an escrow state machine -- proposed, signed, recording, binding, finalized, or expired. It's six states and four transitions, implemented in about 500 lines of Rust. Not a smart contract -- a protocol step that the recorder enforces mechanically.

Smart contracts are general-purpose computation on a replicated state machine. They're powerful and dangerous. They enable flash loan attacks, reentrancy exploits, governance takeovers, and the entire category of DeFi exploits that have cost billions. We don't need general-purpose computation. We need share transfers. So we built share transfers.

## No DeFi

No liquidity pools. No yield farming. No automated market makers. No lending protocols. No staking rewards. No flash loans. No governance tokens with voting power proportional to holdings.

Exchange agents in this system are humans. Charlie is a launch captain who ferries tourists from cruise ships. He holds inventory in vendor chains and earns a spread converting tourist dollars to local credits. If he sets his spread too high, Ziggy undercuts him. If he runs out of inventory, he buys more from vendors. This is how currency exchange works in every tourism economy in the world -- people at airport kiosks and beach-side booths, buying and selling with a spread.

The "protocol" for exchange is: Charlie quotes a rate, Alice agrees, they do a two-leg transfer. No bonding curves. No impermanent loss. No oracle manipulation. Just two people agreeing on a price.

This means the system can't do the things DeFi can do: instant liquidation cascades, composable flash loans across protocols, algorithmic stable-asset pegging. We consider this a feature, not a limitation. Those capabilities have produced more value destruction than value creation for end users.

## No Consensus Mechanism (In the Usual Sense)

There's no Byzantine fault tolerance. No Raft. No Paxos. No Tendermint. No proof of stake. No validator set rotating through slot leaders.

Each chain has one recorder. The recorder writes blocks. Period. If two recorders disagree, you have a fork, and that's a social problem (which recorder does Bob want to use?), not a protocol problem.

"But what if the recorder goes rogue?" Then the validator detects it. Every block is hash-chained. Modifying any historical block changes all subsequent hashes. The validator independently verifies the hash chain and can anchor rolled-up hashes to a public chain for anyone to audit. The evidence of tampering is mathematical.

"But what if the recorder censors transactions?" Then you move to a different recorder. Your chain is a portable data structure -- an append-only log with a genesis block and a sequence of signed blocks. Any recorder that has the blockmaker key can continue the chain. In practice, the blockmaker key is held by the chain owner (Bob, Amara, Tia), not by the recorder. The recorder is a hosting provider, not a gatekeeper.

"But what about double-spending?" Each public key is used exactly once for receiving shares. Double-spend checking is a boolean per sequence number: spent or not. When Bob's recorder receives an assignment, it checks whether the source UTXO's key has been spent. Yes/no. No probabilistic finality. No confirmation count. No "wait six blocks to be sure." One block, final, done.

## What We Kept

**Cryptographic integrity.** Ed25519 signatures on every block. SHA2-256 hash chaining. Every transaction is signed by all participants with timestamped signatures. Separable items (large attachments like photos) are replaced by their SHA2-256 hash before signing, so signatures cover the content without including the bulk data.

**Share-based accounting.** Shares are arbitrary-precision integers (via `num-bigint`). Fee calculations use exact rational arithmetic with ceiling rounding. No floating point. No rounding errors. Every node computes identical results.

**Expiration.** Shares expire after a configurable period. Lost keys don't lock up value forever -- the abandoned shares get swept, and their value accretes to remaining shareholders. This is the equivalent of unclaimed gift card value -- it returns to the community instead of vanishing.

**Wire format thrift.** Variable Byte Coding (VBC) for integers, compact DataItem structures, minimal overhead. A typical transaction is a few hundred bytes. The format was designed for LoRa mesh networks at 22 kbps -- if it works over Meshtastic, it works over everything.

**Transparency.** All chains are publicly readable. Anyone can verify any transaction. The recorder publishes blocks; the validator verifies them; the community can audit everything. This is the value proposition for cooperatives and government programs: not privacy (chains are public), but accountability.

## The Result

Seven Rust crates. 255 tests (187 Rust + 68 PWA). ~16,000 lines of Rust across seven crates. A simulation suite with 19 agents trading across 7 chains on a map. Runs on a Raspberry Pi. MIT license.

No mining. No tokens. No smart contracts. No DeFi. No consensus mechanism. No venture capital. No foundation. No roadmap to decentralization.

Just share transfers between people who agreed on a price, recorded on an append-only log, verified by anyone who cares to look.

If that sounds boring, good. Financial infrastructure should be boring. The exciting version has cost people $68 billion in DeFi exploits since 2020. We'll take boring.

GitHub: [assignonward/aosuite](https://github.com/assignonward/aosuite)

---

*Assign Onward is an open-source project started in 2018 and rewritten in Rust in 2026. MIT license. No affiliation with any blockchain platform, exchange, or foundation. The code is the product.*
