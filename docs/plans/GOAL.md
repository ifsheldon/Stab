# Goal: Stim Core Parity With Lean Evidence

Status: Active. P0 through P8 are complete. P9 is next.

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

## Current Milestone: P9

1. Freeze one clean committed measured revision after the P8 checkpoint. Run formatting, strict workspace Clippy, workspace tests, architecture and external-consumer checks, generated docs, the live result-format corpus, all implemented fixtures, and the full and soak parity tiers from that revision.
2. Run the complete 29-case E2E suite at full and soak tiers on controlled AArch64 CPU 0. Use unique absent bundle paths, require temperature below `100 C`, record configured swap before and after, reject any swap page-in or page-out movement, and leave the prior swap configuration unchanged.
3. Require every Stim-comparable case to pass paired median and confidence upper bound `<= 1.25x`, every case to pass memory, exact semantic witnesses to pass, and both bundles to replay offline. Preserve failed paths and never rerun into them.
4. Because no AArch64 self baseline exists, retain the first full and soak reports as explicitly `unseeded`. Generate exactly one baseline candidate from that pair, review it, and add those exact identities to `benchmarks/suite.toml`; do not claim that the seeding run passed self-regression.
5. Create the narrow evidence descendant allowed by `docs/RELEASING.md`: add the immutable full and soak bundles under `benchmarks/evidence/aarch64/`, the current evidence pointer, generated suite view, reviewed baseline, and synchronized named status prose. Change no product or runner code in that descendant.
6. Run `just bench::e2e-release-check`, offline replay, milestone audit, full code review, and the final verification commands. Verify the worktree is clean, no benchmark process remains, swap state is unchanged, and required GitHub CI passes the exact evidence descendant before making a release claim.

## Remaining Sequence

- P9 produces the replayable controlled-host evidence, seeds the first self baseline without retroactive claims, and retires superseded machinery and status prose.

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
