# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 through P7 are complete. P8 is next. P9 has not started.

## Objective

Reach semantic feature parity with Stim v1.16.0 for the selected Rust and CLI product, then prove it with a concise behavior-oriented correctness suite and one end-to-end performance system. Prefer one production owner, one semantic test owner, and one generated status source over compatibility layers, duplicate representations, or mirrored ledgers.

## Scope

- Include both model dialects, every nondeprecated gate and alias, legal arguments and targets, six result formats, the selected Rust workflows, seven computational commands, and help discovery.
- Defer Python, JS/WASM, ecosystem integrations, `diagram`, `explain_errors`, full `ErrorMatcher` provenance, `repl`, public interactive simulators, array/state-vector/arbitrary-unitary exports, QASM/Quirk, GPU execution, and exact Stim random streams.
- Omit deprecated Stim behavior. Record confirmed Stim bugs and deliberate typed resource limits as explicit divergences with pinned reproductions and focused Stab tests.
- Keep agent inspection, JSON Lines diagnostics, decoder sessions, external circuit passes, `.stim -> .stim` conversion, and concise Stab-native help as tested Stab extensions, not parity claims.
- Do not preserve obsolete pre-1.0 Stab APIs with compatibility shims.

## Sources Of Truth

- [Active implementation plan](stim-core-parity-and-lean-evidence-plan.md) owns rationale, architecture, milestones, tests, benchmarks, and acceptance criteria.
- `oracle/stim-v1.16-parity.toml` and its family fragments own feature and evidence status.
- [Generated parity view](../stim-parity.md) owns volatile counts and must match the ledger.
- [Architecture rules](../architecture/README.md) own durable component boundaries.
- `benchmarks/suite.toml` is the sole active performance source; [the generated suite view](../../benchmarks/SUITE.md) must match it.

Historical plans, progress reports, qualification inventories, and benchmark manifests are context only. They must not create parallel requirements or promote current claims.

## Current Milestone: P8

1. Use a fresh smoke or full diagnostic run to reproduce each comparable case that exceeds the unchanged `1.25x` Stim gate. Validate semantic work, output witnesses, and peak RSS before profiling.
2. Prioritize the largest user-visible losses: `sample-surface`, `detect-observables`, `sample-folded-ptb64`, `m2d-packed-sweep`, `sample-dem`, and `qec-cli-pipeline`. Treat one-sample P7 ratios only as triage signals, not claims.
3. Profile the complete workflow and attribute the dominant cost to startup, parsing, compilation, execution, conversion, encoding, allocation, or I/O. Add a temporary focused probe only when the profile cannot isolate the owner directly.
4. Optimize the canonical production owner without duplicating representations, weakening resource limits, changing semantic work, adding waivers, or relaxing thresholds.
5. After each change, run the semantic owner tests, affected pinned differentials, the exact E2E cases, and peak-RSS checks. Remove temporary probes unless a profile proves they explain at least 10% of a release workflow or isolate a confirmed regression.
6. Record accepted changes and rejected experiments in the P8 checkpoint of the active plan. Do not promote dirty, smoke-tier, or shared-host measurements into release evidence.

P8 closes only when every Stim-comparable release case passes both paired median and confidence-upper-bound `<= 1.25x`, memory passes, seeded cases satisfy `<= 1.15x`, milestone audit and full code review have no unresolved finding, and the suite still has exactly one workload and policy owner.

## Remaining Sequence

- P8 profiles and fixes failing E2E cases without weakening work or thresholds.
- P9 produces one replayable controlled-host evidence bundle and deletes superseded machinery and status prose.

## Non-Negotiable Gates

- Preserve strict grammars, typed DETS semantics, path-alias data-loss prevention, bounded process supervision, meaningful resource limits, output validation, paired timing, and peak-RSS evidence.
- Every surviving test must protect semantic, statistical, safety, resource, or user-visible behavior. Do not add tests for derives, labels, re-exports, private pointer identity, or bookkeeping shape.
- A persistent benchmark must represent a user workflow or explain at least 10% of one. Internal helpers receive focused diagnostics only when a profile justifies them.
- Release E2E Stim parity requires paired median and confidence upper bound `<= 1.25x`.
- Seeded Stab self-regression requires median and confidence upper bound `<= 1.15x`; a missing baseline is `unseeded`.
- Do not add waivers, reduce semantic work, or relax thresholds to obtain a pass.
- Formal evidence requires one clean committed revision and one unique immutable output bundle.
- Development occurs directly on `main`; do not create a branch or linked worktree.

## Completion

This goal is complete when every nondeprecated, nondeferred selected behavior is implemented or an approved divergence; every completed behavior has one meaningful owner; each capability has one production representation and one public route; the single E2E suite passes correctness, memory, Stim parity, and seeded self-regression gates on controlled AArch64; the evidence replays offline; superseded compatibility and qualification machinery is deleted; CI passes the exact evidence commit; and the worktree is clean.
