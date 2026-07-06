# PFM4 DEM SAT Flat Repeat Progress Report

## Scope

This PFM4 slice owns one folded SAT/WCNF generation subcase: large DEM repeat bodies whose body has zero detector shift and contains only `error(p)` instructions. Weighted SAT folds nonzero mechanisms while skipping zero-probability mechanisms; unweighted SAT folds structurally, including zero-probability mechanisms, because shortest-error parity ignores probabilities but still treats each mechanism as selectable structure.

The selected behavior is compact SAT generation for repeated identical error mechanisms:

- `shortest_error_sat_problem` keeps Stim-style probability-insensitive shortest-error semantics by representing each repeated flat body error once, including zero-probability mechanisms.
- `likeliest_error_sat_problem` folds each repeated flat body error into a single parity variable that preserves the minimum concrete assignment cost for the parity, not the marginal odd-parity sampling probability.
- Small repeated bodies remain expanded so existing exact Stim-compatible WCNF text for ordinary repeat cases stays stable.

This slice does not change graphlike search, hypergraph search, analyzer traversal, ErrorMatcher traversal, shifted repeat bodies, nested repeat bodies, repeat bodies containing non-error instructions, high-index dense detector or observable caps, Python APIs, diagrams, or deferred simulator-product surfaces.

## Comparator And Evidence Plan

Comparator class: structural Rust parity for selected large flat zero-shift SAT repeat bodies.
Exact WCNF text remains the comparator for existing small repeat cases; large selected repeats use semantic WCNF equivalence against a compact folded model because pinned Stim v1.16.0 would materialize repeated clauses instead of producing the compact representation.

## Implemented Slice

`shortest_error_sat_problem` and `likeliest_error_sat_problem` now use a SAT-local traversal path instead of the generic materialized flattening budget when generating `FlattenedError` rows.
The traversal preserves the existing cap for ordinary repeated active errors, shifted repeats, nested repeats, and repeat bodies containing non-error items.
When it sees a selected large flat zero-shift repeat body, it appends a single compact body-equivalent error list at the current detector offset and then continues with the post-repeat detector offset.

For weighted SAT, the compact error probability encodes the concrete MAP parity preference: probabilities below `0.5` keep the original probability, probabilities above `0.5` keep the original probability for odd repeat counts and use the complement for even repeat counts, `0.5` stays free, even deterministic repeats are omitted, and odd deterministic repeats become hard true clauses.

## Tests

New targeted tests:

- `sat_problem_shortest_folds_large_flat_zero_shift_repeats`.
- `sat_problem_likeliest_folds_large_flat_zero_shift_repeats_by_map_cost`.
- `sat_problem_likeliest_treats_deterministic_error_as_hard`.
- `pf4_dem_search_sat_folds_flat_nonzero_zero_shift_repeat_bodies`.

Existing tests remain in force for:

- Exact small repeat offset flattening.
- Excessive shifted active repeat rejection.
- Weighted zero-probability repeat skipping.
- Weighted deterministic-error hard-clause encoding.
- Unweighted probability-insensitive shortest-error behavior, including folded zero-probability structural mechanisms.

## Oracle And Benchmarks

Metadata updates:

- Added `pf4-dem-sat-flat-repeat-fold` benchmark metadata and runner coverage with selected flat SAT repeat submeasurements plus `folded-errors/s` and `folded-zero-probability-errors/s` measurement work units.
- Added PF4 oracle metadata row `pf4-dem-sat-flat-repeat-fold-rust` to identify the selected SAT flat-repeat folding evidence without claiming full folded traversal.

The row stays non-primary `contract-only` because the compact folded large-repeat representation is semantic Stab evidence, not a faithful pinned-Stim text or timing ratio.

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

Focused commands run after implementation:

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
The implemented evidence satisfies the scoped contract for flat zero-shift SAT/WCNF repeat bodies while keeping shifted, nested, non-flat, analyzer, ErrorMatcher, graphlike, hypergraph, and high-index dense-target structural repeat work outside this slice.
The later selected nested zero-shift SAT/WCNF follow-up is tracked separately in `pfm4-dem-sat-nested-repeat-progress-report.md`.

Previous full-code-review status for the original all-nonzero unweighted and weighted flat-repeat slice: findings resolved.
The later zero-probability structural SAT extension has its own review status in `pfm4-dem-sat-zero-probability-repeat-progress-report.md`.
The core GPT-5.5/xhigh sidecar found that weighted SAT folding by odd-parity marginal probability would change Stim-style concrete MAP semantics; weighted SAT now folds by concrete MAP parity cost, deterministic weighted errors become hard clauses, and tests include a small-probability counterexample that would fail under marginal parity folding.
The docs and benchmark GPT-5.5/xhigh sidecar found scope and provenance drift; the docs now distinguish structural unweighted folding from weighted nonzero-mechanism folding, and the SAT flat-repeat benchmark/oracle evidence is split into `pf4-dem-sat-flat-repeat-fold` and `pf4-dem-sat-flat-repeat-fold-rust` with SAT/WCNF upstream provenance.
The sidecar re-check found one stale command block in `rpf4-dem-search-sat-progress-report.md`, which was corrected, and the core re-check reported no remaining findings.
