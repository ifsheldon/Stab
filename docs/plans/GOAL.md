# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 is complete; P1 lean correctness ownership is the current milestone; P2 through P9 have not started.

## Objective

Complete every nondeferred Stim v1.16.0 core Rust and CLI behavior through the existing component architecture, then prove correctness, user-visible performance, memory use, and Stab self-regression with one concise evidence system.

## Scope

- In scope: both model dialects, all 81 canonical instructions and 12 aliases, all legal target shapes, six result formats and their applicable CLI routes, generators, sampling, detection, conversion, analysis, search, transforms, algebra capabilities, seven computational commands, and the Stim help discovery surface.
- Deferred: Python object bindings, JS/WASM, ecosystem integrations, `diagram`, `explain_errors`, full `ErrorMatcher` provenance, `repl`, public `TableauSimulator` and `FlipSimulator` products, QASM/Quirk exports, GPU execution, and exact Stim random streams.
- Agent inspection commands, JSON Lines diagnostics, decoder sessions, external circuit passes, `.stim -> .stim` conversion, and concise Stab-native help are tested Stab extensions or explicit divergences; they do not close Stim parity rows.
- Stable Python capabilities define relevant behavior for idiomatic Rust APIs. Python object shape and unstable C++ source/header compatibility are not goals.
- Safe typed resource limits may differ from Stim when the divergence is documented and tested.
- Deprecated Stim behavior is removed. Confirmed Stim bugs require an explicit divergence entry and independent regression test.
- Obsolete pre-1.0 Stab APIs receive migration notes, not compatibility shims.

## Current State

- The component-crate split is retained; no new product crate is planned.
- The current correctness and benchmark systems remain operational only until their lean replacements pass.
- P0 completed in `07ebf4c8`: the validated ledger contains 132 atomic families, 50 executable canonical owners, and 47 explicit P1 owner debts; [stim-parity.md](../stim-parity.md) is the generated current status view.
- Existing feature and qualification documents are historical or transitional inputs, not parallel status sources.
- Historical timing remains historical. No intermediate refactor revision may produce promotable evidence.
- Development occurs directly on `main`; do not create a branch or linked worktree.

## Execution Order

1. P0: freeze scope and replace status ledgers.
2. P1: build the lean behavior-oriented correctness suite.
3. P2: finish the breaking public architecture reset.
4. P3: close model, gate, and record-format parity.
5. P4: close sampling and detection parity.
6. P5: close analysis, transform, search, and algebra parity; keep decoder conformance separate as a Stab extension.
7. P6: close CLI and Rust workflow parity.
8. P7: replace performance machinery with one E2E suite.
9. P8: profile and optimize user-visible regressions.
10. P9: produce one controlled AArch64 evidence bundle and retire superseded history.

## Non-Negotiable Gates

- Preserve strict text grammars, typed DETS behavior, path-alias data-loss prevention, bounded process supervision, pinned-Stim comparison, resource limits, paired timing, complete output validation, and peak-RSS evidence.
- Every surviving test must protect a semantic, statistical, safety, resource, or user-visible contract.
- Release E2E Stim parity remains median and confidence upper bound `<= 1.25x`.
- Seeded Stab self-regression remains median and confidence upper bound `<= 1.15x`.
- Missing baselines are `unseeded`; do not add waivers, weaken work, or relax thresholds.
- Formal evidence requires one clean committed revision and a unique immutable output bundle.

## Current Milestone: P1

1. Organize canonical tests by parity behavior family and product owner instead of per-export qualification inventory.
2. Preserve the pinned result-format corpus, path-alias data-loss matrix, bounded process-supervisor tests, decoder conformance, and external circuit-pass proof.
3. Consolidate shared corpus schema and decoding support without moving semantic assertions out of their owning crates.
4. Give every implemented `needs-owner` row one meaningful semantic test and a real-process assertion when the contract is CLI-visible.
5. Delete derive, type-name, re-export, constant, marker, static-label, private-pointer-identity, and duplicate tests only after stronger semantic ownership is in place.
6. Use fixed-seed properties for parser/printer, algebra, folded-repeat, chunking, and codec invariants; test allocation bounds instead of storage identity.
7. Run `milestone-audit` and `full-code-review`, fix confirmed findings, regenerate the parity view, and commit P1 in focused slices.

## Active Sources

- [Stim core parity and lean evidence plan](stim-core-parity-and-lean-evidence-plan.md)
- [Agent-native component architecture](agent-native-modular-qec-architecture-plan.md)
- [Architecture rules](../architecture/README.md)
- Current source of parity truth: `oracle/stim-v1.16-parity.toml`
- Future source of performance truth: `benchmarks/suite.toml`

## Done

This goal is complete when the frozen parity ledger has no nondeferred `missing` row, the clean public architecture has one owner and one route per capability, the lean suite proves every completed behavior, the single E2E system passes its unchanged correctness, parity, memory, and seeded regression gates on controlled AArch64, one evidence bundle replays offline, superseded machinery and plans are deleted, CI passes the exact source commit, and the worktree is clean.
