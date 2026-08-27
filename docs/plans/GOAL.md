# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 through P6 are complete. P7 is next. P8 and P9 have not started.

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
- P7 introduces `benchmarks/suite.toml` as the sole active performance source before deleting both superseded benchmark systems.

Historical plans, progress reports, qualification inventories, and benchmark manifests are context only. They must not create parallel requirements or promote current claims.

## Current Milestone: P7

1. Create one `benchmarks/suite.toml` containing the fixed 12 workflow families and 29 family-scale cases from the active plan, with exact arguments, deterministic inputs, semantic work, output validation, memory policy, Stim parity policy, and Stab regression policy.
2. Run CLI families process-to-process through release binaries and the Rust pipeline through its stable component APIs. Include startup, parsing, compilation, execution, codecs, and I/O in E2E timings.
3. Reuse the bounded process supervisor and paired alternating sampler. Report wall time, semantic throughput, peak RSS, output size, paired median, and fixed-seed bootstrap confidence interval without deleting samples or subtracting startup.
4. Require exact correctness prerequisites before timing. Validate deterministic output exactly and stochastic output through fixed semantic or statistical witnesses outside timed regions.
5. Preserve the `1.25x` Stim parity and `1.15x` seeded self-regression gates. Missing baselines remain unseeded, and no waiver, reduced work, or relaxed threshold can produce a pass.
6. Retire each legacy benchmark and qualification route as its conclusion moves into the new suite. End P7 with one runner, one policy source, at most 30 release cases, and at most 15 profile-justified diagnostics.

P7 closes only after schema and rejection tests, process-supervisor adversarial tests, output-witness tests, a deterministic dry run of every case, generated documentation, benchmark smoke, workspace checks, milestone audit, full code review, and deletion of the superseded active machinery.

## Remaining Sequence

- P7 replaces both benchmark systems with one capped E2E suite of user workflows.
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
