# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 through P2 are complete. P3 is in progress; P4 through P9 have not started.

## Objective

Reach semantic feature parity with Stim v1.16.0 for the selected Rust and CLI product, then prove it with a concise behavior-oriented correctness suite and one end-to-end performance system. Prefer one production owner, one semantic test owner, and one generated status source over compatibility layers or mirrored ledgers.

## Scope

- In scope: both model dialects, 81 canonical instructions, 12 aliases, legal targets and arguments, six result formats and applicable CLI routes, generators, sampling, detection, conversion, analysis, search, transforms, algebra, seven computational commands, and help discovery.
- Deferred: Python and JS/WASM bindings, ecosystem integrations, `diagram`, `explain_errors`, full `ErrorMatcher` provenance, `repl`, public interactive simulators, QASM/Quirk, GPU execution, and exact Stim random streams.
- Deprecated Stim behavior is omitted. Confirmed Stim bugs and deliberate typed resource limits require explicit divergence rows and focused tests.
- Agent inspection, JSON Lines diagnostics, decoder sessions, external circuit passes, `.stim -> .stim` conversion, and concise Stab-native help remain tested Stab extensions.
- Obsolete pre-1.0 Stab APIs receive no compatibility shims.

## Current Truth

- `oracle/stim-v1.16-parity.toml` and its family fragments are the sole feature and evidence ledger. [stim-parity.md](../stim-parity.md) is generated from it and owns volatile counts.
- P1 replaced per-export and duplicate evidence with behavior-oriented canonical owners while preserving compatibility, safety, statistical, and resource contracts.
- P2 left `stab-core` as a checked convenience facade, moved algorithms and semantic tests to component owners, removed compatibility adapters, and established owned plan/session/transaction boundaries.
- The first P3 slice adds typed finite `AbsoluteTolerance` values and iterative circuit and DEM comparison over reachable compact trees. It records and reproduces Stim's repeat-tag and orphaned-repeat-storage bugs instead of copying them.
- No approximate-equality benchmark is active because the comparator has no measured release-workflow cost or other evidence that it accounts for at least 10% of a user workflow.
- The second P3 slice computes typed circuit reference signs in `stab-engine` by reusing the existing sampling reference and repeat-aware measurement-to-detection plan. A focused Rust owner and pinned `libstim.a` conversion probe cover output order, sparse observable ids, folded repeats, duplicate cancellation, noise-free reference behavior, Pauli targets combined with `XCZ` and `YCZ` sweep controls, empty output, typed failure, and configurable resource admission.
- Reference signs have no standalone benchmark because they are not a release E2E workflow and no profile attributes at least 10% of one to this API.
- P3 now has one missing contract: `resource-safety.model-remaining-parse-limits`.
- The historical correctness inventory is only a generated bridge for active benchmark prerequisites. Add no semantic ownership to it; P7 deletes it with the inherited benchmark system.
- Historical timing remains historical. Formal evidence waits for the final clean architecture and benchmark contracts.
- Development occurs directly on `main`; do not create a branch or linked worktree.

## Immediate Work

1. Implement `resource-safety.model-remaining-parse-limits` in `stab-model`. Introduce typed byte, represented-instruction, and represented-target admission at the parser boundary, prove exact-limit acceptance and first-excess rejection for both dialects, and reject before proportional allocation.
2. Start with failing owner tests, update the parity ledger only after behavior passes independently, regenerate `docs/stim-parity.md`, and keep shared syntax and policy in one production owner.
3. Add a P3 benchmark candidate only when it represents a future E2E workflow or profiling attributes at least 10% of one workflow to the changed path. Otherwise record the no-benchmark rationale.
4. Run the focused tests, parity PR owners, workspace checks, architecture checks, oracle contracts, benchmark smoke, `milestone-audit`, and `full-code-review`; fix confirmed findings and commit the bounded slice before continuing.

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
