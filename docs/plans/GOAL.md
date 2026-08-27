# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 through P2 are complete. P3 is next; P4 through P9 have not started.

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
- P2 has consolidated all six result codecs on `RecordFormat`, removed facade-owned algorithms and compatibility adapters, moved the CLI and the operational hot paths touched by P2 to direct component ownership, and reduced `stab-core` to a finite convenience facade. Untouched historical operational code may still exercise the facade as a consumer until P7 retires that benchmark system.
- P2 replay owns reusable mutable state, while incremental replay and conversion bind exactly one sink in short-lived transactions. Semantic tests now live with their production owners, every facade export is checked against its exact source crate, active fixtures are owner-valid, and the final audit and review are clean.
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

## P2 Closure Evidence

- `stab-core` is a checked convenience facade with no algorithms, duplicate models, universal error, backend placeholders, or legacy compiled adapters.
- CLI and operational hot paths touched by P2 compose the owning component crates directly. The finite facade inventory binds every root export to one exact source owner and rejects crate-level behavior, globs, local definitions, wrong owners, duplicates, omissions, and reordering.
- Sampling, measurement conversion, and DEM replay use compiler, immutable plan, mutable session, and typed sink contracts. Incremental conversion and replay bind one sink for one transaction lifetime, fail closed after an abandoned committed transaction, and report cumulative committed progress.
- Model, analysis, engine, and record tests execute in their semantic owner crates. The non-product graph/vector simulator cross-check was removed instead of being retained as false parity evidence.
- Workspace formatting, warnings-denied Clippy, all-feature tests, architecture and consumer checks, 100 parity owners, the 62-case result-format oracle, 313 compatibility rows, 122 blocker cases, the implemented oracle suite, 48 benchmark prerequisites, qualification status, and benchmark smoke pass.
- Milestone audit and full code review report no remaining P2 blocker or specification gap. P2 changed no active steady-state loop, warmed allocation, codec, or process boundary, so it makes no new timing claim.

## Immediate Work

1. Start P3 with the cohesive model-owned pair `circuit-model.approximate-equality` and `dem-model.approximate-equality`.
2. Read the pinned Stim implementation and tests first, then freeze exact tolerance, nesting, length, target, tag, and non-finite-number semantics in concise table-driven owner tests. Do not infer compatibility from Stab round trips.
3. Implement the smallest shared comparison mechanism in `stab-model` that preserves the distinct circuit and DEM structures without introducing a generic public abstraction.
4. Update the parity ledger and generated view only after each family has one independently executable semantic owner. Run model tests, parity checks and owners, the workspace checks, architecture checks, oracle contracts, and benchmark smoke.
5. Run `milestone-audit` and `full-code-review`, fix confirmed findings, and commit the bounded P3 slice before selecting reference-sign or remaining parse-limit work.

## Remaining Milestones

- P3: model, gate, and result-format parity.
- P4: sampling and detection parity.
- P5: analysis, transform, search, and algebra parity.
- P6: CLI and Rust workflow parity.
- P7: one user-visible E2E performance suite.
- P8: profile and optimize user-visible regressions.
- P9: one controlled AArch64 evidence bundle and retirement of superseded machinery.

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
- Planned P7 performance source: `benchmarks/suite.toml`

## Completion

This goal is complete when no nondeferred parity row is `missing`, every completed behavior has one meaningful owner, the public architecture has one route per capability, the single E2E suite passes correctness, Stim parity, memory, and seeded self-regression gates on controlled AArch64, its evidence replays offline, superseded machinery is deleted, CI passes the exact source commit, and the worktree is clean.
