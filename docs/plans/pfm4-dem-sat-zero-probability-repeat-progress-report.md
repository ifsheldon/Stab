# PFM4 DEM SAT Zero-Probability Repeat Progress Report

## Scope

This PFM4 slice owns one folded SAT generation subcase: large DEM repeat bodies whose body has zero detector shift and contains only `error(p)` instructions, including `error(0)` instructions, for unweighted `shortest_error_sat_problem`.

The selected behavior is structural, not probabilistic:

- Unweighted shortest-error SAT ignores probabilities for cost purposes, so repeated `error(0)` mechanisms remain real structural mechanisms.
- A large flat zero-shift repeat of the same body can be represented by one copy of each body error because additional identical copies do not reduce the minimum number of selected mechanisms needed to realize any parity.
- The fold is allowed only for flat repeat bodies containing error instructions and no detector shifts, nested repeats, detector annotations, logical-observable annotations, or other non-error items.

This slice does not change weighted SAT zero-probability elision, graphlike search, hypergraph search, analyzer traversal, ErrorMatcher traversal, DEM sampling, shifted repeat bodies, nested repeat bodies, non-flat bodies, dense detector or observable caps, Python APIs, diagrams, or deferred simulator-product surfaces.

## Comparator And Evidence Plan

Comparator class: structural Rust parity for selected large flat zero-shift zero-probability SAT repeat bodies.

The primary semantic comparator is exact WCNF equality between a large selected repeat body and the compact single-body model for `shortest_error_sat_problem`.
Pinned Stim v1.16.0 would materialize the repeated clauses, so the benchmark row remains contract-only and report-only rather than a direct Stim timing ratio.

## Implemented Slice

The existing SAT-local flat-repeat fold now keeps one flattened structural copy of every flat body error for `SatProblemMode::Unweighted`, including zero-probability errors.
The weighted branch is unchanged: it still omits zero-probability variables and folds nonzero probabilities by concrete MAP parity cost.

Keep these rejection boundaries:

- Shifted zero-probability repeats stay capped because detector offsets differ by iteration.
- Nested or non-flat bodies stay capped.
- Folded results that still imply detector or observable vectors beyond `MAX_SAT_DENSE_TARGET_COUNT` keep the existing dense target rejection.

## Tests

Added or updated targeted tests:

- `sat_problem_shortest_folds_large_flat_zero_shift_zero_probability_repeats`, comparing a large flat zero-probability repeat against the compact single-body WCNF.
- `pf4_dem_search_sat_folds_flat_zero_probability_zero_shift_repeat_bodies`, proving the integration path uses the same compact WCNF equivalence.
- Existing shifted zero-probability repeat rejection tests remain in force.
- `pf4_dem_search_skips_zero_probability_repeat_bodies` now expects dense-target rejection for the high-index structural SAT model after the selected flat fold reaches target validation.

## Oracle And Benchmarks

Existing SAT-sourced evidence rows were updated instead of adding a broad new row:

- Extend `pf4-dem-sat-flat-repeat-fold-rust` to name selected unweighted zero-probability structural folding.
- Extend `pf4-dem-sat-flat-repeat-fold` with `stab_pf4_dem_sat_zero_probability_flat_repeat_fold`.
- Add `folded-zero-probability-errors/s` measurement work units for the new submeasurement.

The benchmark remains non-primary `contract-only` and report-only.

Fresh focused compare:

```text
stab_pf4_dem_sat_flat_repeat_fold=0.000002270s, rate=8.811e11 folded-errors/s
stab_pf4_dem_sat_zero_probability_flat_repeat_fold=0.000000706s, rate=2.833e12 folded-zero-probability-errors/s
stab_pf4_dem_weighted_sat_flat_repeat_fold=0.000003484s, rate=5.741e11 folded-errors/s
```

Artifacts:

- `target/benchmarks/pfm4-dem-sat-zero-probability-repeat-baseline/baseline.json`
- `target/benchmarks/pfm4-dem-sat-zero-probability-repeat-compare/compare.json`

## Verification

Focused commands:

```sh
cargo fmt --all --check
cargo test -p stab-core sat_problem --quiet
cargo test -p stab-core --test dem_search pf4_dem_search_sat --quiet
cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4
just bench::smoke
just bench::baseline --only pf4-dem-sat-flat-repeat-fold --out target/benchmarks/pfm4-dem-sat-zero-probability-repeat-baseline
just bench::compare --only pf4-dem-sat-flat-repeat-fold --baseline target/benchmarks/pfm4-dem-sat-zero-probability-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-sat-zero-probability-repeat-compare
git diff --check
```

Milestone-audit status: complete for this selected slice.
The audit found no blocking implementation or evidence findings.
The completion matrix is satisfied by the SAT unit test at `crates/stab-core/src/dem/sat.rs`, the PF4 integration test at `crates/stab-core/tests/dem_search.rs`, the SAT-sourced oracle row `pf4-dem-sat-flat-repeat-fold-rust`, and the `pf4-dem-sat-flat-repeat-fold` benchmark submeasurement.
Residual shifted, nested, non-flat, and high-index dense-target structural SAT repeat work remained explicit follow-up scope for this slice, not a defect in this slice.
The later selected nested zero-shift SAT/WCNF follow-up in `pfm4-dem-sat-nested-repeat-progress-report.md` promotes part of that former nested follow-up scope while shifted, non-flat, and high-index dense-target structural SAT repeats remain active outside the selected shape.
Full-code-review status: findings resolved.
The core GPT-5.5/xhigh sidecar found no blocking issues and confirmed the unweighted branch keeps zero-probability mechanisms structurally while the weighted branch still elides them.
It also confirmed dense target caps still run before vector allocation for the high-detector structural case and that the selected fold is scoped to nonempty zero-shift all-error bodies.
Residual risks are documented: the large-repeat WCNF comparator is compact semantic equivalence rather than byte-for-byte Stim unrolling, and `crates/stab-core/src/dem/sat.rs` remains close to the 1200-line watch threshold.
The high-observable zero-probability flat-repeat cap now has dedicated follow-up evidence in `pfm4-dem-sat-high-observable-cap-progress-report.md`.
The docs and benchmark GPT-5.5/xhigh sidecar found two P3 documentation issues in the older flat-repeat report, both fixed by aligning the benchmark artifact commands and scoping its previous review-status note to the prior slice.
