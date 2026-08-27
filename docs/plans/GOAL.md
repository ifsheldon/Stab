# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 through P3 are complete. P4 through P9 have not started.

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
- The first P4 slice removes the private reject-versus-allow sweep compiler split. The sole sampling compiler lowers every legal sweep-controlled Pauli into the existing typed operation IR, and ordinary sessions use Stim's omitted all-false sweep semantics through repeats. Sampling compiler schema version 2 and the derived request and plan fingerprints identify the changed admission contract.
- The generated classical-control matrix owns ordinary sampling across every accepted sweep target orientation and compares omitted-sweep output to the all-false baseline. Existing release CLI sampling workflows remain the performance owners; adding a per-control benchmark would measure no distinct user hot path.
- A metadata-driven sampler owner constructs and executes every declared legal canonical gate/target pattern and a nested-repeat program. The audit found no missing legal sampler kernel after sweep admission; remaining compiler rejections are narrow invalid-shape failures already classified by the gate contract. Common semantic and statistical owners retain value-level coverage instead of being duplicated into the admission matrix.
- The historical correctness inventory is only a generated bridge for active benchmark prerequisites. Add no semantic ownership to it; P7 deletes it with the inherited benchmark system.
- Historical timing remains historical. Formal evidence waits for the final clean architecture and benchmark contracts.
- Development occurs directly on `main`; do not create a branch or linked worktree.

## Immediate Work

1. Continue P4 from the three remaining engine contracts: meaningful loop-folding selection, complete detection gate execution, and remaining sweep/feedback conversion.
2. Inventory each legal gate and target shape against the existing private execution IR before adding code. Consolidate duplicated lowering or selected-subset exits instead of creating per-surface compatibility paths.
3. Add one generated semantic matrix per real execution path, pinned-Stim statistical or exact comparators where applicable, and focused resource/cancellation owners. Keep complete gate coverage distinct from already verified common, noise, reference-correction, and feedback families.
4. Add or retain an E2E benchmark only when the workflow is user-visible and the changed path is measured. Do not create per-gate timing rows.
5. Run the focused tests, parity PR owners, workspace and architecture checks, oracle contracts, benchmark smoke, `milestone-audit`, and `full-code-review`; fix confirmed findings and commit each bounded P4 slice before continuing.

## Remaining Milestones

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
