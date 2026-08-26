# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 scope and source-of-truth replacement is the current milestone; P1 through P9 have not started.

## Objective

Complete every nondeferred Stim v1.16.0 core Rust and CLI behavior through the existing component architecture, then prove correctness, user-visible performance, memory use, and Stab self-regression with one concise evidence system.

## Scope

- In scope: both model dialects, every nondeprecated gate and alias, all legal target shapes, six result formats, generators, sampling, detection, conversion, analysis, search, transforms, algebra capabilities, seven computational commands, help, and agent inspection commands.
- Deferred: Python, JS/WASM, ecosystem integrations, `diagram`, `explain_errors`, full `ErrorMatcher` provenance, `repl`, interactive simulators, QASM/Quirk exports, GPU execution, and exact Stim random streams.
- Safe typed resource limits may differ from Stim when the divergence is documented and tested.
- Deprecated Stim behavior is removed. Confirmed Stim bugs require an explicit divergence entry and independent regression test.
- Obsolete pre-1.0 Stab APIs receive migration notes, not compatibility shims.

## Current State

- The component-crate split is retained; no new product crate is planned.
- The current correctness and benchmark systems remain operational only until their lean replacements pass.
- Existing feature documents are broad historical inputs. P0 replaces their current-status role with one generated parity ledger.
- Historical timing remains historical. No intermediate refactor revision may produce promotable evidence.
- Development occurs directly on `main`; do not create a branch or linked worktree.

## Execution Order

1. P0: freeze scope and replace status ledgers.
2. P1: build the lean behavior-oriented correctness suite.
3. P2: finish the breaking public architecture reset.
4. P3: close model, gate, and record-format parity.
5. P4: close sampling and detection parity.
6. P5: close analysis, transform, search, and decoder parity.
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

## Current Milestone: P0

1. Create `oracle/stim-v1.16-parity.toml` with atomic `done`, `missing`, `deferred`, and `divergence` behavior rows.
2. Reconcile the pinned Stim source, current feature documents, public Rust APIs, CLI routes, and unsupported errors into that ledger.
3. Add deterministic parity validation, execution, and Markdown rendering operations.
4. Generate `docs/stim-parity.md` and make CI reject status drift.
5. Mark older implementation and qualification plans as superseded, then run milestone-audit before P1.

## Active Sources

- [Stim core parity and lean evidence plan](stim-core-parity-and-lean-evidence-plan.md)
- [Agent-native component architecture](agent-native-modular-qec-architecture-plan.md)
- [Architecture rules](../architecture/README.md)
- Future source of parity truth: `oracle/stim-v1.16-parity.toml`
- Future source of performance truth: `benchmarks/suite.toml`

## Done

This goal is complete when the frozen parity ledger has no nondeferred `missing` row, the clean public architecture has one owner and one route per capability, the lean suite proves every completed behavior, the single E2E system passes its unchanged correctness, parity, memory, and seeded regression gates on controlled AArch64, one evidence bundle replays offline, superseded machinery and plans are deleted, CI passes the exact source commit, and the worktree is clean.
