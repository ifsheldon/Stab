# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 and P1 are complete. P2 is in progress; P3 through P9 have not started.

## Objective

Reach semantic feature parity with Stim v1.16.0 for the selected Rust and CLI product, then prove it with a concise behavior-oriented correctness suite and one end-to-end performance system. Prefer one production owner, one semantic test owner, and one generated status source over compatibility layers or mirrored ledgers.

## Scope

- In scope: both model dialects, 81 canonical instructions, 12 aliases, legal targets and arguments, six result formats and applicable CLI routes, generators, sampling, detection, conversion, analysis, search, transforms, algebra, seven computational commands, and help discovery.
- Deferred: Python and JS/WASM bindings, ecosystem integrations, `diagram`, `explain_errors`, full `ErrorMatcher` provenance, `repl`, public interactive simulators, QASM/Quirk, GPU execution, and exact Stim random streams.
- Deprecated Stim behavior is omitted. Confirmed Stim bugs and deliberate typed resource limits require explicit divergence rows and focused tests.
- Agent inspection, JSON Lines diagnostics, decoder sessions, external circuit passes, `.stim -> .stim` conversion, and concise Stab-native help remain tested Stab extensions rather than Stim parity claims.
- Obsolete pre-1.0 Stab APIs receive no compatibility shims.

## Current Truth

- `oracle/stim-v1.16-parity.toml` and its family fragments are the sole feature and evidence ledger. [stim-parity.md](../stim-parity.md) is generated from it.
- Current family, implementation, divergence, and canonical-owner totals are generated in [stim-parity.md](../stim-parity.md); active prose does not duplicate those volatile counts.
- P1 removed or consolidated per-export, structural-only, duplicate, exhaustive, and representation-specific tests while preserving compatibility, data-loss, process, statistical, and resource-boundary coverage.
- P2 has consolidated all six result codecs on `RecordFormat`, removed facade-only circuit path helpers and forwarding modules, removed fictitious backend selection, and moved the CLI onto direct model, record, analysis, and engine ownership with private typed diagnostic composition. The remaining P2 work is facade domain-error and compatibility-adapter deletion plus executable consumer fixtures.
- The historical correctness inventory remains only as a mechanically generated benchmark-prerequisite bridge. Add no semantic ownership to it; delete it when P7 replaces the inherited benchmark system.
- New model-wide parse byte, instruction-count, and target-count policies belong to P3, not P1.
- Historical timing remains historical. Formal evidence must wait for the final clean architecture and benchmark contracts.
- Development occurs directly on `main`; do not create a branch or linked worktree.

## P1 Closure Evidence

- The behavior-oriented parity suite resolves and passes every canonical owner reported by the generated parity ledger, including pinned-Stim differentials, strict result formats, file-identity safety, statistical semantics, fixed-seed properties, and exact resource boundaries.
- `oracle/qualification-cases.json` now generates the finite benchmark prerequisite bridge directly. It rejects retired ownership fields, and benchmark validation proves exact bidirectional equality with runtime prerequisite IDs.
- Workspace formatting, warnings-denied Clippy, all workspace tests, the live result-format corpus, the implemented oracle suite, parity PR owners, compatibility matrix, correctness PR and full tiers, benchmark contracts, generated status, benchmark smoke, pre-commit, and diff checks pass.
- Milestone audit and full code review found no remaining P1 blocker after exact logical-search boundaries, retained-state-term ordering, deterministic property coverage, and bridge enforcement were repaired.

Canonical family owners use concise table-driven or generated matrices when cases share one public contract. They split at different public semantics, failure classes, cancellation behavior, or resource boundaries, not once per upstream test case.

## Remaining Order

1. P2: finish the breaking public architecture reset.
2. P3: close model, gate, and result-format parity.
3. P4: close sampling and detection parity.
4. P5: close analysis, transform, search, and algebra parity.
5. P6: close CLI and Rust workflow parity.
6. P7: replace performance machinery with one user-visible E2E suite.
7. P8: profile and optimize user-visible regressions.
8. P9: produce one controlled AArch64 evidence bundle and retire superseded history.

## Non-Negotiable Gates

- Preserve strict grammars, typed DETS behavior, path-alias data-loss prevention, bounded process supervision, pinned-Stim comparison, meaningful resource limits, paired timing, output validation, and peak-RSS evidence.
- Every surviving test must protect semantic, statistical, safety, resource, or user-visible behavior.
- Release E2E Stim parity remains median and confidence upper bound `<= 1.25x`.
- Seeded Stab self-regression remains median and confidence upper bound `<= 1.15x`; missing baselines are `unseeded`.
- Do not add waivers, weaken work, or relax thresholds to obtain a pass.
- Formal evidence requires one clean committed revision and one unique immutable output bundle.

## Sources

- [Active plan](stim-core-parity-and-lean-evidence-plan.md)
- [Agent-native architecture](agent-native-modular-qec-architecture-plan.md)
- [Architecture rules](../architecture/README.md)
- Current parity source: `oracle/stim-v1.16-parity.toml`
- Future performance source: `benchmarks/suite.toml`

## Completion

This goal is complete when no nondeferred parity row is `missing`, every completed behavior has one meaningful owner, the public architecture has one route per capability, the single E2E suite passes correctness, Stim parity, memory, and seeded self-regression gates on controlled AArch64, its evidence replays offline, superseded machinery is deleted, CI passes the exact source commit, and the worktree is clean.
