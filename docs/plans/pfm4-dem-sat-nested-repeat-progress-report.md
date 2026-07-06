# PFM4 DEM SAT Nested Repeat Progress Report

## Scope

This slice promotes selected nested zero-shift SAT/WCNF repeat folding for `shortest_error_sat_problem` and `likeliest_error_sat_problem`.
It covers large nested DEM repeat bodies whose promoted bodies have total detector shift zero and whose SAT semantics match a compact folded error list.
The selected bodies may contain `error` instructions, zero-probability errors for unweighted SAT, no-target errors, `shift_detectors 0`, `detector` declarations, standalone `logical_observable` declarations, and nested selected repeat blocks.

This is not a full SAT folded-traversal implementation.
Nonzero detector shifts, shifted nested repeats, non-flat or high-index dense-target structural SAT repeats beyond the selected compact shape, analyzer traversal, ErrorMatcher traversal, graphlike and hypergraph search behavior, sampler behavior, CLI behavior, Python, diagrams, and simulator-product APIs are unchanged by this slice.

## Implemented Evidence

- Replaced the flat-only SAT repeat fast path with selected zero-shift folded SAT repeat traversal that can recursively fold nested selected bodies without unrolling them.
- Preserved the existing repeat cap for nested bodies with nonzero detector shift.
- Preserved the existing unweighted SAT behavior where zero-probability mechanisms are structural and must not be skipped.
- Preserved the existing weighted WCNF behavior where zero-probability mechanisms are omitted and nonzero mechanisms are folded by concrete MAP parity cost across the full nested repeat multiplier.
- Kept no-target mechanisms in the compact folded SAT error list, matching the existing objective semantics.
- Added `pf4-dem-sat-nested-repeat-fold-rust` as a SAT/WCNF-sourced oracle row for the nested test filter.
- Extended the report-only `pf4-dem-sat-flat-repeat-fold` benchmark row with `stab_pf4_dem_sat_nested_repeat_fold` and `stab_pf4_dem_weighted_sat_nested_repeat_fold`.

## Tests

- `sat_problem_shortest_folds_large_nested_zero_shift_repeats` proves unweighted SAT folds selected detector-touching, zero-probability, and no-target nested zero-shift repeats while shifted nested repeats still reject before unbounded expansion.
- `sat_problem_likeliest_folds_large_nested_zero_shift_repeats_by_map_cost` proves weighted WCNF folds selected nested nonzero mechanisms by nested MAP parity cost, includes a high-probability even-repeat regression, omits zero-probability nested mechanisms, keeps no-target objective semantics, and preserves the shifted nested cap.

## Benchmark Evidence

The focused local compare used:

```sh
just bench::baseline --only pf4-dem-sat-flat-repeat-fold --out target/benchmarks/pfm4-dem-sat-nested-repeat-baseline
just bench::compare --only pf4-dem-sat-flat-repeat-fold --baseline target/benchmarks/pfm4-dem-sat-nested-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-sat-nested-repeat-compare
```

The compare measured `stab_pf4_dem_sat_nested_repeat_fold=0.000002338s`, which normalizes to approximately `8.554e17 folded-nested-errors/s`.
It measured `stab_pf4_dem_weighted_sat_nested_repeat_fold=0.000003550s`, which normalizes to approximately `5.634e17 folded-nested-errors/s`.
The row remains non-primary report-only because it measures Stab resource behavior without a faithful pinned-Stim timing ratio for oversized folded Rust API traversal.

## Verification

- `cargo test -p stab-core sat_problem_shortest_folds_large_nested_zero_shift_repeats --quiet`
- `cargo test -p stab-core sat_problem_likeliest_folds_large_nested_zero_shift_repeats_by_map_cost --quiet`
- `cargo test -p stab-core nested_zero_shift_repeats --quiet`
- `cargo test -p stab-core flat_zero_shift --quiet`
- `cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet`
- `cargo test -p stab-bench runner_smoke --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --milestone PF4 --structural`
- `just bench::smoke`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `git diff --check`
- `just maintenance::pre-commit`
- Large-file review check from `.agents/skills/full-code-review/SKILL.md`

## Audit And Review

Milestone-audit status: complete.
The implemented evidence satisfies the scoped selected nested zero-shift SAT/WCNF contract, keeps shifted nested repeats capped, documents unchanged non-goals, and ties the slice to source-owned oracle and report-only benchmark rows.

Full-code-review status: findings resolved.
The GPT-5.5/xhigh core sidecar found that `crates/stab-core/src/dem/sat.rs` had grown past the 1200-line threshold and that the nested weighted expected values were too coupled to the implementation helper.
The repeat-folding tests now live in `crates/stab-core/tests/dem_sat_repeat_folding.rs`, `crates/stab-core/src/dem/sat.rs` is back under the threshold, and the nested weighted tests use direct expected compact models for low-probability, high-probability even-repeat, deterministic even-repeat, deterministic odd-repeat, zero-probability, and no-target cases.
The GPT-5.5/xhigh docs and benchmark sidecar found a P2 documentation gap in the PFM4 benchmark inventory and missing verification evidence in this report.
The PFM4 benchmark inventory now names `pf4-dem-sat-flat-repeat-fold` and its folded SAT work units, and this report records the oracle, benchmark smoke, and large-file verification commands.
Residual risk: `ops/bench/src/baseline/pf4.rs` remains a near-threshold watch-list file at 1193 lines, but this slice did not push it past the review threshold.
