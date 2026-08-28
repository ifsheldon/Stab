# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 through P8 are complete. P9 is locally complete; exact-descendant GitHub CI is pending.

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

Superseded plans, progress reports, qualification inventories, and benchmark manifests remain available through Git history. They do not create parallel requirements or promote current claims.

## Current Milestone: P9

1. Clean measured revision `a8b56db319410f1d52bc64bfb7ee6a63c01c490f` passes formatting, strict workspace Clippy, workspace tests, architecture and external-consumer checks, API and link checks, the live 62-case result-format corpus, all implemented fixtures, and all 138 canonical owners in full and soak tiers.
2. The 29-case controlled AArch64 full and soak bundles under `benchmarks/evidence/aarch64/a8b56db319410f1d52bc64bfb7ee6a63c01c490f/` pass correctness, memory, and Stim parity. Full worst median and upper ratios are `1.1385x` and `1.2244x`; soak values are `1.1705x` and `1.1892x`.
3. Both bundles replay offline. They used CPU 0, stayed below `62 C`, retained `/swap.img`, and observed no page-in or page-out movement within either run.
4. Both reports remain correctly `unseeded`. The suite contains exactly the reviewed 29-entry self-regression candidate derived from the worse full and soak bounds; the seeding pair does not claim a regression pass.
5. Evidence descendant `992bd1b5d109b2e0cd673556366d06b0e912a50d` passes `just bench::e2e-release-check`; fresh copies of both committed bundles replay all 29 cases. Milestone audit and full code review found no remaining blocker after one stale P9 sentence was corrected.
6. Run the final verification commands from the final documentation descendant. Required GitHub CI must then pass that exact commit before making the release claim. Controlled x86-64 remains explicitly unqualified.

## Remaining Sequence

- P9 closes after final local verification and exact-revision GitHub CI pass.

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

This goal is complete when every nondeprecated, nondeferred selected behavior is implemented or an approved divergence; every completed behavior has one meaningful owner; each capability has one production representation and one public route; the single E2E suite passes correctness, memory, and Stim parity on controlled AArch64; existing self baselines pass regression or the first unseeded pair seeds exactly one reviewed baseline without a retroactive pass claim; the evidence replays offline; superseded compatibility and qualification machinery is deleted; CI passes the exact evidence descendant; and the worktree is clean.
