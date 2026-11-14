# Goal: Stim Core Parity With Lean Evidence

Status: P10 evidence repair is active. A November audit found DEM printing, output-flush, standard-stream aliasing, argument-grammar, random-tableau, and benchmark peak-RSS defects plus weak fixture and statistical evidence; see the plan's P10 milestone. P10's fresh controlled pair is published for measured source `2c229ccd` after the November toolchain and engine work closed the performance gaps that blocked it. Exact-revision CI is outstanding, and the shared process supervisor and documentation cleanup that landed after `7b0aa4f3` changed non-evidence paths, so a fresh pair is due before release use. P0 through P9 remain complete for their recorded revisions.

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
- `oracle/stim-v1.16-parity.toml` and its family fragments own feature and evidence status; `oracle/fixtures/manifest.csv` owns supporting pinned-Stim fixtures.
- [Generated parity view](../stim-parity.md) owns volatile counts and must match the ledger.
- [Architecture rules](../architecture/README.md) own durable component boundaries.
- `benchmarks/suite.toml` is the sole active performance source; [the generated suite view](../../benchmarks/SUITE.md) must match it.
- `benchmarks/current-aarch64-evidence.toml` names the controlled evidence pair for the repaired source revision.
- [Open items](open-items.md) lists known gaps that no milestone owns.

Superseded plans, progress reports, qualification inventories, and benchmark manifests remain available through Git history. They do not create parallel requirements or promote current claims.

## Current Evidence

1. Measured source `2c229ccd59b51657e0f68e349f4de98e157672ef` is clean and local. Local verification of its product ancestor `61cede6b` passed 1,567 workspace tests, all 141 canonical owners, all 199 fixture manifest rows, and the 83-case live result-format differential; the commits between them change plan documents only. No GitHub CI run exists for the measured source or its evidence descendant, because these commits have not been pushed.
2. The controlled AArch64 full and soak bundles under `benchmarks/evidence/aarch64/2c229ccd59b51657e0f68e349f4de98e157672ef/` each cover all 25 release cases under the `e2e-user-workflow-v3` boundary and pass correctness, memory, and `1.25x` Stim parity on all 24 cases that have a Stim counterpart.
3. Full worst median and confidence upper ratios are `0.970867x` (`sample-dem.medium`) and `0.991665x` (`sample-surface.d25-b8`); soak values are `0.961373x` and `0.970937x` (both `sample-dem.medium`). Maximum Stab peak RSS is `111,095,808` bytes for full and `111,108,096` bytes for soak, both on `analyze-decompose.d25`.
4. Both bundles replay offline. They used CPU 4, recorded temperatures at or below `66 C`, ran with no configured swap and unchanged swap counters, and `/swap.img` was restored afterward at its prior priority `-1`. No benchmark process remained.
5. This is the first formal pair for the current suite, so both bundles report self-regression as `unseeded`, and `benchmarks/suite.toml` holds exactly the 25 baselines derived from the pair. The retired `b4a758db` pair and its 29-case matrix remain in Git history.
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
