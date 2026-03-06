# 2026/ — Development Folder

This folder contains all new work for the Assign Onward 2026 reboot. Everything outside this folder is read-only reference material.

## Contents

| File | Purpose |
|------|---------|
| [ProjectSummary.md](ProjectSummary.md) | Full project context: what AO is, how it works, tech stack, risks |
| [ROADMAP.md](ROADMAP.md) | Development roadmap (Phase 0–6, ~54 weeks) |

## Reference Material (read-only)

- `docs/html/` — Original 2018 design documents (~38 HTML files)
- `apps/` — Original C++ codebase (~15,600 lines, serialization layer only)
- `apps/CodeForm/byteCodeDefinitions.json` — Type code registry (useful reference for wire format)
- `apps/BlockDef/riceyCodes.cpp` — VBC encoding reference implementation

## Work Style

### General Principles

- **The repository is the source of truth.** The current state of the files in the repo is the current working document set. Historical versions live in git history — there is no need to keep superseded documents around or maintain changelog sections within files. If a document is obsolete, delete it; its content is recoverable from prior commits.
- **Don't accumulate dead weight.** When a document is replaced or a design decision changes, update or remove the affected files. The git log preserves the full history of what was decided and why (via commit messages with prompts and rationale).

### What Counts as a Successful Change

A change is successful and ready to commit when either:

1. **Approved by the user** — the user reviews and accepts the change, OR
2. **Tests pass** — sufficient unit and integration tests are in place, and the change passes all applicable tests.

Documentation-only changes (Phase 0 specs, README updates) fall under criterion 1. Code changes (Phase 1+) require criterion 2.

### Commits

Each successful change is committed with a message in this format:

```
<50 words or less: concise description of what the commit did>

Prompt(s):
<exact copy of the conversational prompt or prompts which led to the change>

Changes:
- <what this element changed> — <why, when non-obvious>
- <what this element changed> — <why, when non-obvious>
- ...
```

Example:

```
Add wire format spec with VBC test vectors

Prompt(s):
"Write deliverable 0B from the roadmap, starting with VBC encoding"

Changes:
- Added 2026/specs/WireFormat.md with complete VBC specification — resolves
  ambiguity in original VariableByteCoding.html around negative number encoding
- Added 2026/specs/conformance/vbc-vectors.json with 30 test values — provides
  ground truth for cross-implementation compatibility
- Updated 2026/README.md to list new spec files
```

### Documentation Structure

- **Concise and modular.** Each document has a single clear purpose.
- **Well-linked.** Related documents reference each other explicitly.
- **Target size: under 250 lines.** Break up when a document exceeds ~250 lines and has identifiable conceptual partitions. Hard maximum: 2000 lines.
- **Markdown format** for all new documents in `2026/`.

### Folder Organization

As Phase 0 deliverables and later code are produced, the folder will grow:

```
2026/
├── README.md              # This file
├── ProjectSummary.md      # Project context
├── ROADMAP.md             # Development roadmap
├── specs/                 # Phase 0 deliverables
│   ├── Architecture.md    # 0A: System architecture
│   ├── WireFormat.md      # 0B: Wire format specification
│   ├── CryptoChoices.md   # 0C: Cryptographic decisions
│   ├── EconomicRules.md   # 0D: Deterministic arithmetic
│   └── conformance/       # 0E: Test vectors
│       └── vectors.json
└── src/                   # Rust workspace (Phase 1+)
    ├── Cargo.toml
    ├── ao-types/
    ├── ao-crypto/
    ├── ao-chain/
    ├── ao-recorder/
    ├── ao-validator/
    ├── ao-cli/
    └── ao-web/
```
