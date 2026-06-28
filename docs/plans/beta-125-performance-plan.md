# 1.25x Beta Performance Plan

## Purpose

This plan tightens the Stab primary beta performance gate from `2.0x` to `1.25x` and then improves the currently slower or fragile rows until the stricter gate is honest, executable, and stable.
It follows the lessons in `docs/plans/lessons-learned.md`: benchmark comparability must be explicit, tiny measurements need deliberate evidence policy, paired submeasurements must not be hidden behind row medians, final reports must be clean, and waivers must stay source-owned and machine-checked.

## Current Evidence

The current clean beta report is `target/benchmarks/m12-primary-beta/compare.json` from Stab commit `2499f39b41e50478a8e2407f71da56f3442a7a97` with `local_modifications=false`.
It passes the existing `2.0x` beta gate with 72 comparable pass rows and 4 checked no-ratio waivers.

If the beta gate were changed to `1.25x` without further work, these comparable rows would fail:

| Row | Comparability | Current worst ratio | Blocking surface |
| --- | --- | ---: | --- |
| `m10-error-decomp` | `direct-match` | `1.875x` | Tiny independent/disjoint XYZ filters, especially `independent_to_disjoint_xyz_errors` and exact disjoint-to-independent conversion. |
| `m5-simd-bits` | `partial-match` | `1.488x` row-level | Bundled benchmark shape mixes direct Stim pairs with Stab-only masked, range, and copy contract extras. |
| `m4-circuit-parse` | `direct-match` | `1.2877x` | Sparse `.stim` parser pair `circuit_parse_sparse` to `stab_circuit_parse_sparse`. |

These rows are below `1.25x` today but should get headroom or explicit stability evidence before the stricter gate becomes completion evidence:

| Row | Comparability | Current worst ratio | Headroom concern |
| --- | --- | ---: | --- |
| `m5-sparse-xor` | `direct-match` | `1.1589x` | Row-XOR pair is close enough to deserve extra profiling and repeated evidence. |
| `m4-gate-lookup` | `partial-match` | `1.070x` | Tiny canonical hash pair should not be overfit, but it needs variance monitoring. |
| `m8-sample-primary-unrotated-surface-contract` | `cli-baseline` | `1.0188x` | Close to parity and already thresholded, but sampler timing should be checked after every optimization. |

The four current no-ratio beta waivers remain outside this performance optimization scope unless a faithful pinned-Stim ratio becomes available:

- `m4-circuit-canonical-print`
- `m7-convert-stim-canonical`
- `m8-measure-reader-ptb64-contract`
- `m10-dem-print-contract`

## Non-Goals

Do not reopen intentionally deferred Stim parity or ecosystem surfaces such as Python, JS/WASM, Crumble, diagrams, `explain_errors`, `repl`, QASM/Quirk, GPU, sweep-conditioned conversion, `m2d --ran_without_feedback`, full ErrorMatcher provenance, or new public graph/vector simulator APIs.
Do not add beta waivers for comparable rows.
Do not weaken the meaning of a ratio by comparing unmatched work.
Do not use dirty-worktree reports as final evidence.
Do not count Stab-only contract extras as strict pinned-Stim beta evidence unless the plan first defines a faithful Stim comparator.

## Milestone B0: Freeze Starting Evidence

Objective: establish a clean starting point before changing gate semantics or performance code.

Tasks:

- Regenerate a clean primary baseline and beta report from committed code before the first behavior change.
- Record the exact current failures under a temporary `1.25x` beta check or an analysis command that applies the same worst-ratio rule.
- Confirm that the starting failure set is exactly `m10-error-decomp`, `m5-simd-bits`, and `m4-circuit-parse`.
- Confirm that `m5-sparse-xor`, `m4-gate-lookup`, and `m8-sample-primary-unrotated-surface-contract` are below `1.25x` but close enough to track.
- Do not use this starting evidence as completion evidence after code changes.

Linked commands:

```sh
just bench::baseline --primary --out target/benchmarks/beta-125-start-baseline
just bench::primary-beta --baseline target/benchmarks/beta-125-start-baseline/baseline.json
```

Done criteria:

- A clean starting report exists with `local_modifications=false`.
- The plan's failure and headroom tables are updated if the clean evidence differs from the current report.
- Any discrepancy is explained before implementation begins.

## Milestone B1: Make The 1.25x Beta Gate Explicit

Objective: change beta-gate semantics to `1.25x` in code, tests, docs, and report wording while allowing the first strict run to fail for known rows during implementation.

Tasks:

- Change the beta gate constant from `2.0` to `1.25`.
- Update CLI help for `--require-beta-gate`.
- Update benchmark docs and roadmap docs that define the beta gate.
- Update tests that check beta-gate pass and fail messages.
- Add or update tests proving paired submeasurement worst-ratio evidence is used for beta at `1.25x`.
- Keep the `1.5x` profiler-note threshold distinct from the `1.25x` beta and timing-regression thresholds.
- Keep contract-only beta waiver validation unchanged except for message text that names the new gate.

Linked tests:

- `cargo test -p stab-bench beta_gate --quiet`
- `cargo test -p stab-bench compare_row_result_records_ratio_and_beta_gate_status --quiet`
- `cargo test -p stab-bench primary_compare_rows_have_machine_readable_comparability_classes --quiet`

Done criteria:

- `--require-beta-gate` fails comparable rows above `1.25x`.
- It still rejects stale or misapplied no-ratio waivers.
- It still waives only measured `contract-only` rows named by `benchmarks/m12-primary-beta-waivers.json`.
- Documentation no longer describes the active beta gate as `2.0x`, except in historical evidence sections that clearly say the old gate was historical.

## Milestone B2: Fix `m5-simd-bits` Comparability Shape

Objective: make `m5-simd-bits` satisfy the stricter beta gate by correcting benchmark shape before attempting unnecessary SIMD optimization.

Problem statement:

The row-level ratio is above `1.25x` because the row bundles faithful direct pairs with Stab-only contract extras.
The direct `simd_bits_xor_10K` pair is already faster than pinned Stim, and the actual upstream `not_zero` workload is already guarded by a schema-version-2 threshold.

Tasks:

- Split the row or change row-level beta aggregation so unmatched Stab contract extras cannot dominate strict Stim-relative beta status.
- Prefer separate rows for direct Stim pairs and Stab-only extras if that keeps comparability clearer.
- Keep direct pairs for `simd_bits_xor_10K` and the actual pinned `simd_bits_not_zero_100K` source workload mirrored as `stab_simd_bits_not_zero_10K`.
- Keep masked XOR, range XOR, and copy workloads measured as Stab-only contract evidence until pinned Stim exposes faithful filters or a later plan defines Stab-only memory throughput gates.
- Update `benchmarks/m12-primary-thresholds.json`, profiler notes, benchmark runner tests, and progress docs in the same change set as the benchmark-shape change.

Linked tests and checks:

- Benchmark runner tests proving the direct row emits paired submeasurements.
- Benchmark runner tests proving contract extras are still present and labeled honestly.
- Threshold source validation for `benchmarks/m12-primary-thresholds.json`.
- `just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline <baseline> --report <report>`

Done criteria:

- Faithful M5 SIMD bit pairs pass the `1.25x` beta gate.
- Stab-only extras remain visible but no longer masquerade as strict Stim-relative row evidence.
- No threshold is added for unmatched work.

## Milestone B3: Optimize `m4-circuit-parse`

Objective: reduce the sparse parser pair `stab_circuit_parse_sparse` below `1.25x` with enough headroom to avoid flaky strict beta failures.

Problem statement:

The dense parser pair is faster than pinned Stim, but sparse parsing currently determines the row ratio.
Prior optimization removed temporary target vectors, repeated integer scans, and some generic dispatch, but the remaining sparse-parser cost is still slightly above the stricter gate.

Tasks:

- Run focused M4 compare evidence before editing parser code.
- Identify whether the remaining cost is parser dispatch, circuit item allocation, target construction, line scanning, or comment/tag handling.
- Extend fast paths only for shapes that appear in the benchmark or common Stim circuits and preserve full parser semantics.
- Investigate reusing target buffers or directly constructing small circuit instructions without extra intermediate containers.
- Avoid broad parser rewrites unless focused evidence shows the current structure is the bottleneck.
- Add regression tests before or alongside parser changes for comments, tags, numeric targets, no-argument gates, multi-target gates, malformed targets, and canonical round trips.

Linked tests and checks:

- `cargo test -p stab-core --test stim_format --quiet`
- `cargo test -p stab-core gates --quiet`
- `cargo test -p stab-core target --quiet`
- `just oracle::run --implemented-only`
- Focused M4 benchmark compare against a fresh primary baseline.

Done criteria:

- `m4-circuit-parse` worst paired ratio is `<=1.25x`, with a target of `<=1.15x` for headroom.
- The stable sparse pair is guarded by beta evidence and, if repeated clean evidence is stable enough, by a schema-version-2 threshold.
- Parser behavior remains compatible with implemented Stim v1.16.0 surfaces.

## Milestone B4: Rework `m10-error-decomp` Evidence And Arithmetic

Objective: make `m10-error-decomp` pass the stricter beta gate without hiding tiny slow filters or relying on timer artifacts.

Problem statement:

The current row has sub-nanosecond or few-nanosecond pinned Stim filters where timer overhead can dominate.
The `approx_p10` pair is already below `1.25x`, while exact, `p100`, and independent-to-disjoint pairs are not all safely below `1.25x`.

Tasks:

- First determine whether the current tiny filters are measuring arithmetic or timer overhead.
- Add larger faithful paired case-array benchmark variants if the current filters are too small for meaningful strict beta evidence.
- Keep reported timing normalized to seconds per operation when batching or case arrays are used.
- Keep Stim and Stab workloads symmetric.
- Profile and optimize arithmetic only after the evidence shape is honest.
- Investigate fast paths for exact conversion, independent-to-disjoint conversion, repeated probability constants, branch reduction, and avoidable floating-point transformations.
- Update `benchmarks/profiler-notes/m12/m10-error-decomp.md` and `benchmarks/profiler-notes/m12/optimization-log.json` with before and after evidence.

Linked tests and checks:

- Existing probability and error-decomposition tests.
- New unit tests for any arithmetic fast path with exact and approximate cases.
- Benchmark runner tests proving all direct pairs are present and paired as intended.
- Focused M10 benchmark compare against a fresh primary baseline.

Done criteria:

- `m10-error-decomp` passes beta at `<=1.25x` under the same worst paired-ratio rule used by the gate.
- If the benchmark shape changes from tiny filters to larger faithful arrays, the docs explain why the replacement is more meaningful and the old tiny filters no longer decide strict beta completion.
- No slow direct submeasurement is hidden behind the row median.

## Milestone B5: Add Headroom To Near Misses

Objective: make the stricter beta gate robust enough that clean reruns do not flap around `1.25x`.

Tasks for `m5-sparse-xor`:

- Run focused evidence separating row-XOR and item-XOR.
- Profile the row-XOR path before changing data structures.
- Try small-row specialization, capacity reuse, branch reduction, or merge-loop simplification only if evidence points there.
- Preserve sorted-unique invariants and symmetric-difference behavior.
- Target `<=1.10x` for row-XOR if practical.

Tasks for `m4-gate-lookup`:

- Keep canonical hash pair thresholded.
- Do not overfit nanosecond noise.
- Add repeated clean evidence if the row drifts above `1.15x`.
- Keep alias, lowercase, and invalid lookup contract extras outside strict Stim-relative evidence.

Tasks for `m8-sample-primary-unrotated-surface-contract`:

- Recheck after other changes because sampler timing can move with unrelated parser or generator changes.
- Optimize only if repeated clean evidence drifts above `1.15x`.
- Keep public sampler semantics and oracle parity unchanged.

Linked tests and checks:

- `cargo test -p stab-core --test bits --quiet` or the relevant sparse XOR tests.
- `cargo test -p stab-core gates --quiet`
- Sampler and oracle tests if M8 sampler code changes.
- Focused benchmark reports for each changed row.

Done criteria:

- Near-miss rows remain below `1.25x` in final clean evidence.
- Any row still close to `1.25x` has a profiler note explaining why no speculative optimization was made.

## Milestone B6: Documentation And Source Synchronization

Objective: keep behavior, plans, reports, thresholds, waivers, and profiler notes aligned.

Tasks:

- Update `docs/plans/rust-stim-drop-in-rewrite.md` so the active beta gate is `1.25x`.
- Update `benchmarks/README.md` so `--require-beta-gate` and timing-regression thresholds no longer imply different ratio limits.
- Update `docs/plans/m12-progress-report.md` with historical wording for the old `2.0x` gate and current wording for the new `1.25x` gate.
- Update `docs/plans/post-beta-fix-report.md` only where it would otherwise mislead a future agent about final current evidence.
- Update `benchmarks/profiler-notes/m12/optimization-log.json` for every row optimized or benchmark-shape-changed by this plan.
- Update `benchmarks/profiler-notes/m12/*.md` for each row that remains close to or above profiler-note thresholds.
- Update `docs/plans/milestone-spec-gaps.md` only for true under-specification revealed by implementation, not for ordinary bugs or missing tests.

Done criteria:

- No current-source doc describes the active beta gate as `2.0x`.
- Historical mentions of `2.0x` are explicitly historical.
- Row counts, waiver counts, threshold counts, report paths, and command names agree across docs and machine-readable files.

## Milestone B7: Final Clean Evidence, Audit, And Review

Objective: prove the stricter beta gate from committed code and close the plan with audit and full review.

Required final commands:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::run --implemented-only
just bench::baseline --primary --out target/benchmarks/beta-125-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/beta-125-primary-baseline/baseline.json --report target/benchmarks/beta-125-primary-compare
just bench::primary-beta --baseline target/benchmarks/beta-125-primary-baseline/baseline.json
just bench::primary-regression --baseline target/benchmarks/beta-125-primary-baseline/baseline.json --report target/benchmarks/beta-125-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/beta-125-primary-baseline/baseline.json
just maintenance::pre-commit
```

Final done criteria:

- The beta report is generated from committed code with `local_modifications=false`.
- Every comparable primary row passes beta at `<=1.25x` using the worse of row median and paired submeasurement ratios.
- The only beta waivers are measured no-ratio `contract-only` rows with checked source-owned entries.
- Timing-regression has no ambiguous `not-configured` rows.
- Memory regression passes all primary rows.
- Milestone-audit finds no implementation blocker.
- Full-code-review finds no correctness, compatibility, benchmark-policy, or documentation blocker.
- The worktree is clean unless the user explicitly accepts uncommitted follow-up work.
