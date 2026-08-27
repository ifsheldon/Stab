# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 through P5 are complete. P6 is next. P7 through P9 have not started.

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
- `benchmarks/suite.toml` will become the sole active performance source in P7.

Historical plans, progress reports, qualification inventories, and benchmark manifests are context only. They must not create parallel requirements or promote current claims.

## Current Milestone: P6

1. Close the seven computational commands and help for every nondeprecated in-scope argument and combination through the built release binary.
2. Remove legacy top-level dispatch, deprecated frame and observable-order aliases, hidden `sample_dem` observable aliases, stale help advertising, and duplicate normalization routes instead of preserving compatibility shims.
3. Keep `.stim -> .stim` conversion, concise Stab-native help, agent inspection, JSON Lines diagnostics, decoder sessions, and external circuit passes as explicitly tested Stab extensions rather than parity claims.
4. Expose direct component-crate workflows and keep `stab-core` a thin convenience facade with no algorithm, duplicate model, or catch-all error ownership.
5. Preserve command-wide typed file-role validation before truncation, strict result grammars, bounded streaming, broken-pipe propagation, and exact output routing.
6. Promote the final missing parity family only after real-binary success and failure tests prove removed routes are absent and the generated ledger remains consistent.

P6 closes only after targeted component and real-binary tests, implemented fixtures, parity check and rendering, workspace formatting, warnings-denied Clippy, all workspace tests, milestone audit, full code review, and repair of every confirmed finding.

## Remaining Sequence

- P6 closes real-binary CLI behavior and direct Rust workflows, then removes deprecated dispatch and duplicate routing.
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
