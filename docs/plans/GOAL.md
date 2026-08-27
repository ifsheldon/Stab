# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 through P4 are complete. P5 is in progress; P6 through P9 have not started.

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
- The final P3 slice gives both model parsers one typed admission policy for original bytes, physical lines, compact declarations, retained targets, and repeat nesting. Byte admission precedes bounded byte-metadata preparation; declarations count before repeat expansion and circuit fusion; decoded targets are admitted before retention; wide fast paths cannot bypass the cumulative target budget. Byte preparation is bounded by source-byte and admitted-line policy and intentionally precedes semantic declaration and target admission.
- One table-driven owner proves exact reduced boundaries, first excess, typed operation/resource/dialect/source-line/span data, zero limits, and byte-before-UTF-8 precedence through string and byte entry points. A separate allocation owner proves that model-item storage grows only from successful declarations: thousands of units after a rejected or invalid command do not change allocation count or peak bytes, and blank or comment-only source reserves no model-item storage.
- Parser admission has no benchmark row because rejection is not a release E2E workflow and no profile attributes at least 10% of one workflow to these checks. Existing circuit and DEM parsing workflows remain the diagnostic performance owners.
- Source-current diagnostics after removing input-derived reservation passed: `m4-circuit-parse` measured `0.554x` and `0.503x` Stim for its dense and sparse pairs, while serial sealed-worker probes measured `0.922499x` for circuit parse and `0.915128x` for DEM parse. These dirty-tree diagnostics make no promotable timing claim.
- P3 has no missing nondeferred parity rows. Exact parser boundaries, optimization-independent accounting, and rejected-source allocation are explicit resource-limit divergences because Stim v1.16.0 exposes no comparable configurable policy.
- P4 has one model-owned classical-control classifier and one private engine operation for active record or sweep controls. Sampling and direct detection consume the same target-shape truth; classical `CZ` no-ops bypass irrelevant record-history validation; omitted sweeps remain false; and the small-frame executor no longer falls back solely because a sweep target exists.
- One metadata-driven owner executes every declared legal gate shape through measurement sampling, measurement conversion, direct detector-frame sampling, and automatic detection sampling, including nested repeats. Common semantic and statistical owners retain value-level coverage instead of being duplicated into this admission matrix.
- Detection compilation validates the sole sampling plan before choosing zero, static, or sweep reference state. `m2d --skip_reference_sample` therefore cannot bypass sampler validation or truncate outputs first. Feedback inlining shares the classifier, drops legal all-classical `CZ` no-ops, and records pinned Stim's mixed record/sweep transform bug as an explicit divergence.
- Sampling, measurement-to-detection conversion, and direct detector-frame execution retain compact repeats in validated flat operation tapes and use fixed-depth compile and execution stacks. Structural validation never executes repeat counts. Detection conversion computes expanded work arithmetically during allocation-free admission and executes cross-iteration lookbacks without materializing repeated terms. Direct-frame SPP operations are lowered during plan compilation rather than per shot, while detector and record-observable extraction use the same compact `ConversionPlan` as fused conversion. Frame preflight charges minimum retained payloads, materialization measures actual vector capacities, and no plan escapes when the actual aggregate exceeds its byte limit. The fixed 100,000-repeat syntax cap is gone; record shape, expanded instructions, aggregate repeat iterations, compact terms, and compact bytes are independent typed budgets.
- One semantic owner now covers every legal sweep-controlled and measurement-record-controlled Pauli orientation, classical `CZ` no-ops, omitted and explicit sweeps, and feedback crossing compact nested repeats. Sweep conversion combines a static all-zero-sweep reference with frame-derived per-record corrections, including Pauli observables in both reference modes. Real CLI tests compare folded and unrolled `m2d` output, appended and side-output Pauli corrections, and `detect` PTB64 grouping; explicit pinned-Stim probes reproduce the sweep semantics.
- Reference-sample repeat folding now requires at least 64 units of reusable work, enough estimated saved stabilizer work to pay for snapshot capture and comparison, no record or sweep controls, and exact recurrence of stabilizer and correlated-error state. Snapshot storage exists only for profitable folded work that fits the existing session ceiling; unprofitable and storage-ineligible candidates fall back to iteration without changing output. Ordinary circuits, `Iterate`, and determined-measurement analysis retain the prior boundary. `ReferenceSampleLoopPolicy::Iterate` and `--skip_loop_folding` disable reuse; sampling compiler schema 5, executable-contract schema 4, and agent plan schema 4 bind the current admission and execution policy.
- P4 is complete. Public qubit counting preserves pinned Stim's `MPAD` behavior while execution uses a separate pad-free width; shallow repeat breadth is independent of nesting; measurement-bearing shots admit one million expanded operation dispatches while zero-width repeats execute in constant work; active record and sweep feedback crosses two folded boundaries; SPP lowering carries typed visitor failures directly; and reference-fold counters are test-only.
- P4 makes no performance conclusion from dirty-tree diagnostics. Its sampling and detection workflows are fixed P7 release-matrix members, where reproducible E2E parity and self-regression evidence belong.
- The historical correctness inventory is only a generated bridge for active benchmark prerequisites. Add no semantic ownership to it; P7 deletes it with the inherited benchmark system.
- Historical timing remains historical. Formal evidence waits for the final clean architecture and benchmark contracts.
- Development occurs directly on `main`; do not create a branch or linked worktree.

## Immediate Work

1. Inventory each P5 `missing` family against current implementation and pinned Stim behavior; promote only direct executable owners.
2. Close analysis and transform gaps at their existing reverse-tracker, tableau, search, and transform owners without adding parallel representations.
3. Run focused pinned comparisons, workspace checks, milestone audit, and full code review for each bounded P5 slice before promotion.

## Remaining Milestones

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
