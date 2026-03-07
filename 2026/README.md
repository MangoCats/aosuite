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

When a deliverable is approved, its heading in [ROADMAP.md](ROADMAP.md) is updated with the document's location in the project and the approval date (e.g., `— [specs/Architecture.md](specs/Architecture.md) ✓ 2026-03-05`).

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

### Lessons Learned

When a problem requires 3 or more failed attempts before finding the actual solution, write a brief post-mortem in `2026/lessons/` analyzing:

1. **What happened** — the symptom and the actual root cause.
2. **Where it went astray** — which assumption or step first diverged from the correct path.
3. **How it was resolved** — what finally identified the real issue.
4. **Prevention** — what check or practice would have caught it sooner.

The goal is to build a searchable record of debugging patterns so the same class of mistake isn't repeated. Keep each entry short (under 50 lines). Name files descriptively (e.g., `wrong-test-vector.md`, `cargo-feature-flag.md`).

### Documentation Structure

- **Concise and modular.** Each document has a single clear purpose.
- **Well-linked.** Related documents reference each other explicitly.
- **Target size: under 250 lines.** Break up when a document exceeds ~250 lines and has identifiable conceptual partitions. Hard maximum: 2000 lines.
- **Markdown format** for all new documents in `2026/`.

### Boundary: 2026 vs. sims

The `2026/` tree and the `sims/` tree are **independent development domains** maintained by separate teams. Neither team modifies code in the other's domain.

- **All ROADMAP work stays in `2026/`** (and its subfolders, excluding `sims/`). No ROADMAP deliverable touches the sims folder.
- **`sims/` is fully removable.** The 2026 implementation has zero dependencies on anything in `sims/`. Deleting the entire `sims/` folder must have no impact on building, testing, or running any 2026 code.
- **Sims are consumers, not components.** Simulations use the 2026 products (crates, APIs, formats) as external dependencies. They are not part of the implementation.
- **Sims developers own their own updates.** As 2026 APIs or data formats evolve, the sims team is responsible for updating their code to match. 2026 developers do not maintain sims compatibility.

### Folder Organization

As Phase 0 deliverables and later code are produced, the folder will grow:

```
2026/
├── README.md              # This file
├── ProjectSummary.md      # Project context
├── ROADMAP.md             # Development roadmap
├── lessons/               # Debugging post-mortems
├── specs/                 # Phase 0 deliverables
│   ├── Architecture.md
│   ├── WireFormat.md
│   ├── CryptoChoices.md
│   ├── EconomicRules.md
│   └── conformance/       # Test vectors
├── src/                   # Rust workspace (Phase 1+)
│   ├── Cargo.toml
│   ├── ao-types/
│   ├── ao-crypto/
│   ├── ao-chain/
│   ├── ao-recorder/
│   ├── ao-exchange/
│   ├── ao-validator/
│   ├── ao-cli/
│   └── ao-pwa/            # React PWA (Phase 3)
└── sims/                  # Independent simulation suite
    ├── scenarios/         # TOML scenario configs
    ├── src/               # Sim Rust source
    ├── viewer/            # Viewer PWA
    └── docs/
```
