# Goal: Stim Core Parity With Lean Evidence

Status: September review repairs and code reduction have passing local verification, source CI, and fresh controlled AArch64 full and soak evidence. Release use requires the release checker and both mandated CI jobs to pass the exact proposed evidence descendant. P0 through P9 remain complete for their recorded revisions.

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
- `oracle/stim-v1.16-parity.toml` and its family fragments own feature and evidence status plus required supporting fixture identities.
- [Generated parity view](../stim-parity.md) owns volatile counts and must match the ledger.
- [Architecture rules](../architecture/README.md) own durable component boundaries.
- `benchmarks/suite.toml` is the sole active performance source; [the generated suite view](../../benchmarks/SUITE.md) must match it.
- `benchmarks/current-aarch64-evidence.toml` names the controlled evidence pair for the repaired source revision.

Superseded plans, progress reports, qualification inventories, and benchmark manifests remain available through Git history. They do not create parallel requirements or promote current claims.

## Current Evidence

1. Measured source `b4a758db169fd343c93f2b84a5b1d68558c9e6c3` passed both mandated GitHub CI jobs in [run 33954379257](https://github.com/ifsheldon/Stab/actions/runs/33954379257). Local verification also passed 1,831 workspace tests, all 138 canonical owners, 459 implemented fixtures, and the 62-case live result-format differential.
2. The controlled AArch64 full and soak bundles under `benchmarks/evidence/aarch64/b4a758db169fd343c93f2b84a5b1d68558c9e6c3/` each cover all 29 release cases and pass correctness, memory, `1.25x` Stim parity, and seeded `1.15x` Stab self-regression.
3. Full worst median and confidence upper ratios are `1.179210x` and `1.190196x`; soak values are `1.163228x` and `1.199477x`. Maximum Stab peak RSS is `51,896,320` bytes for full and `52,101,120` bytes for soak.
4. Both bundles replay offline. They used CPU 0, recorded temperatures at or below `39 C`, ran with no configured swap and unchanged swap counters, and `/swap.img` was restored afterward at its prior priority `-2`. No benchmark process remained.
5. Workload definitions, timing identity, and the accepted self-regression baselines are unchanged. The historical `a8b56db3` pair remains the explicitly unseeded source of the first reviewed baseline; the `ef986632` pair remains evidence for its original source.
6. Controlled x86-64 timing remains unqualified. Cross-architecture CI proves correctness and contracts, not x86-64 performance.

## Release Use

- Run `just bench::e2e-release-check` from the clean proposed release revision.
- Require every mandated GitHub check to pass that exact revision. A passing ancestor is insufficient.
- Restrict post-measurement descendants to the paths accepted by the release checker. Any product, oracle, benchmark-runner, or workflow change requires fresh measured evidence.
- Preserve failed target artifacts and historical evidence; never reuse an immutable output path.

## Non-Negotiable Gates

- Preserve strict grammars, typed DETS semantics, path-alias data-loss prevention, bounded process supervision, meaningful resource limits, output validation, paired timing, and peak-RSS evidence.
- Every surviving test must protect semantic, statistical, safety, resource, or user-visible behavior. Do not add tests for derives, labels, re-exports, private pointer identity, or bookkeeping shape.
- A persistent benchmark must represent a user workflow or explain at least 10% of one. Internal helpers receive focused diagnostics only when a profile justifies them.
- Do not add waivers, reduce semantic work, or relax the `1.25x` parity or `1.15x` seeded-regression thresholds to obtain a pass.
- Development occurs directly on `main`; do not create a branch or linked worktree.

## Completion Contract

This goal is complete when the selected ledger has no missing nondeprecated, nondeferred behavior; every completed behavior has one meaningful owner; each capability has one production representation and one public route; the single E2E suite passes correctness, memory, Stim parity, and seeded self-regression on controlled AArch64; current evidence replays offline; superseded machinery is absent; the release checker passes; exact-revision CI is green; swap state is restored; no benchmark process remains; and the worktree is clean.
