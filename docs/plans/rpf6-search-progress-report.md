# RPF6 Search Progress Report

## Summary

This report records the promoted PF6 direct DEM graphlike and hypergraph exact-output subset, selected high-observable analyzer-to-search behavior, and the generated-QEC graphlike, hypergraph, and SAT/WCNF search subset.
It adds executable Rust evidence, selected exact direct DEM cases, selected high-observable graphlike and hypergraph search cases, selected ordering-insensitive target-signature comparators, and report-only benchmark runners for generated rotated-surface-code and repetition-code logical-error search and selected generated-QEC SAT/WCNF encoding without claiming full PF6 search parity.

## Implemented Surfaces

- Direct DEM graphlike search rejection for models without an undetectable logical error and exact canonical output for the pinned Stim v1.16.0 distance-one, distance-two, distance-three, and canonical-ordering cases.
- Direct DEM hypergraph search rejection for models without an undetectable logical error and exact canonical output for the pinned Stim v1.16.0 distance-one, distance-two, distance-three, canonical-ordering, and bounded hyper-error cases.
- Graphlike and hypergraph search over the pinned Stim v1.16.0 `many_observables` analyzed MPP circuit shape, proving the returned search errors still toggle the high logical observable `L1200`.
- Generated rotated-surface-code graphlike search with decomposed graphlike DEMs, matching the pinned Stim v1.16.0 generated-search instruction count of 5.
- Generated repetition-code graphlike search with decomposed graphlike DEMs, matching the pinned Stim v1.16.0 generated-search instruction count of 7.
- Generated rotated-surface-code graphlike search with ungraphlike DEMs and `ignore_ungraphlike_errors=true`, matching the pinned Stim v1.16.0 instruction count of 5.
- Generated rotated-surface-code graphlike search rejection for ungraphlike DEMs when `ignore_ungraphlike_errors=false`.
- Generated rotated-surface-code and repetition-code hypergraph search with the pinned Stim v1.16.0 instruction counts of 5 and 7.
- Generated-QEC graphlike and hypergraph search outputs now prove deterministic `error(1)` rows, per-row detector and observable uniqueness, canonical target-set uniqueness, zero detector parity, exact `L0` observable parity, and graphlike or hypergraph per-error detector-weight bounds.
- Generated rotated-surface-code and repetition-code shortest and weighted SAT/WCNF encoding produce nontrivial WDIMACS output with positive soft and hard clauses.
- Ungraphlike generated rotated-surface-code DEMs still produce structural shortest SAT/WCNF output instead of depending on graphlike decomposition.
- Weighted SAT/WCNF output now keeps the header clause count equal to the emitted clause lines even when low quantization rounds a soft clause to zero.

## Tests

Implemented Rust tests:

- `pf6_generated_qec_graphlike_search_has_structural_signature`
- `pf6_generated_qec_hypergraph_search_has_structural_signature`
- `pf6_generated_sat_wcnf_qec_encoding_is_structural`
- `pf6_direct_dem_graphlike_search_matches_upstream_distance_cases`
- `pf6_direct_dem_hypergraph_search_matches_upstream_distance_cases`
- `pf6_graphlike_search_many_observables_preserves_high_observable_id`
- `pf6_hypergraph_search_many_observables_preserves_high_observable_id`

The tests live in `crates/stab-core/tests/dem_search.rs` and are derived from `vendor/stim/src/stim/search/graphlike/algo.test.cc`, `vendor/stim/src/stim/search/hyper/algo.test.cc`, and `vendor/stim/src/stim/search/sat/wcnf.test.cc`.
They assert exact canonical direct DEM outputs for the selected pinned direct search cases, assert the generated-code search result sizes that pinned Stim v1.16.0 asserts, require deterministic error-only DEM output, canonicalize each returned target set, reject duplicate target sets, require detector parity to cancel, require the expected logical observable parity for `L0` generated-QEC and `L1200` high-observable cases, cover the ungraphlike generated surface-code rejection path, and structurally verify WDIMACS headers, emitted clause counts, soft clauses, hard clauses, and weighted top weights for selected generated-QEC SAT/WCNF encodings.

## Oracle Rows

Implemented rows:

- `pf6-search-generated-qec-rust`
- `pf6-search-generated-sat-wcnf-rust`
- `pf6-search-direct-dem-graphlike-rust`
- `pf6-search-direct-dem-hypergraph-rust`
- `pf6-search-many-observables-graphlike-rust`
- `pf6-search-many-observables-hypergraph-rust`

The broad row `pf6-search-generated` remains manifest-only because full generated-circuit search parity still includes broader generated-code families, exact or structural target-set comparators for broader tie-sensitive outputs, broader SAT or WCNF families, additional resource behavior, and sparse reverse tracker integration.

## Benchmark Rows

Rows with new report-only runner coverage:

- `pf6-graphlike-search-generated`, measured as `stab_pf6_graphlike_search_generated_surface`.
- `pf6-hypergraph-search-generated`, measured as `stab_pf6_hypergraph_search_generated_surface`.
- `pf6-generated-sat-wcnf`, measured as `stab_pf6_shortest_sat_generated_surface` and `stab_pf6_likeliest_sat_generated_surface`.

These rows measure generated rotated-surface-code DEM search or SAT/WCNF encoding after source-owned Rust analysis and decomposition.
They remain `non-primary-report-only` and `contract-only` because pinned Stim exposes these search and SAT APIs through C++ API and perf surfaces, not a faithful public CLI baseline for Stab.
They were not added to `benchmarks/m12-primary-thresholds.json`.
Fresh local probe command `just bench::compare --only pf6-generated-sat-wcnf --baseline target/benchmarks/pf6-generated-sat-wcnf-probe/baseline.json --report target/benchmarks/pf6-generated-sat-wcnf-compare` measured `stab_pf6_shortest_sat_generated_surface=0.002952415s`, or approximately `8.129e3 detectors/s`, and `stab_pf6_likeliest_sat_generated_surface=0.001445527s`, or approximately `1.660e4 detectors/s`, as report-only evidence on the local machine.
The direct DEM exact-output and high-observable tests are tiny compatibility fixtures rather than throughput paths, so they do not add separate benchmark rows or threshold entries.

## Remaining PF6 Search Work

- Broader generated-circuit search families beyond the promoted rotated-surface-code and repetition-code cases.
- Broader direct DEM search resource and tie-sensitive families beyond the selected pinned distance, hyper-error, and high-observable exact or structural cases.
- Exact or structural target-set comparators for broader generated families and tie-sensitive outputs beyond the selected generated-QEC structural signature-invariant checks.
- Broader generated SAT or WCNF encoding families beyond the selected generated-QEC structural row.
- Loop-folded generated search behavior.
- Sparse reverse detector-frame tracker analyzer/search integration beyond the promoted supported-Clifford generated repeat-folding subset.
- Full ErrorMatcher provenance, heralded matching, repeat-contained noise stack frames, and `stim explain_errors` CLI remain deferred.

## Audit And Review Notes

Milestone-audit found the selected generated-QEC SAT/WCNF evidence complete against the current PFM6 text after keeping broader generated SAT/WCNF families and broader ordering-insensitive search comparators open.
The local audit also found stale test-source wording in this report, which now names `vendor/stim/src/stim/search/sat/wcnf.test.cc`.
The GPT-5.5/xhigh benchmark sidecar found no benchmark or metadata issues and confirmed that `pf6-generated-sat-wcnf` is `non-primary-report-only`, `contract-only`, has runner smoke coverage, has measurement work for both submeasurements, maps to `report-only` via `compare_note`, and stays out of the primary gate.
The GPT-5.5/xhigh core/test sidecar found that weighted WCNF headers could over-count clauses when quantization rounded a soft clause to zero and that the old `pf6-search-generated-qec-rust` oracle filter accidentally included the new SAT/WCNF test.
The encoder now counts only emitted WCNF clauses, a direct SAT regression covers the zero-quantized soft-clause case, the generated SAT/WCNF test checks emitted line counts against the header, and the oracle filters are disjoint.
A later GPT-5.5/xhigh target-signature audit found that the new generated-QEC comparator should be described as structural invariant evidence instead of upstream target-set parity, and the docs and oracle row now keep broader exact or structural target-set comparators open.
The same review found that duplicate detector or observable targets inside one returned error row could pass the aggregate parity checks, so the test helper now rejects per-row target duplicates before computing the ordering-insensitive signature.
A later GPT-5.5/xhigh high-observable search review found no confirmed issues in the `many_observables` graphlike and hypergraph tests, oracle rows, or scoped documentation and confirmed that the split manifest filters select the intended single tests.

## Verification Evidence

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
cargo test -p stab-core sat_problem --quiet
cargo test -p stab-core --test dem_search pf6_generated_qec_ --quiet
cargo test -p stab-core --test dem_search pf6_generated_sat_wcnf_qec --quiet
cargo test -p stab-core --test dem_search many_observables --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench pf6_analyzer_benchmark_rows_have_stab_compare_runners --quiet
just oracle::run --milestone PF6 --structural
just bench::smoke
just bench::baseline --only pf6-generated-sat-wcnf --out target/benchmarks/pf6-generated-sat-wcnf-probe
just bench::compare --only pf6-generated-sat-wcnf --baseline target/benchmarks/pf6-generated-sat-wcnf-probe/baseline.json --report target/benchmarks/pf6-generated-sat-wcnf-compare
just maintenance::pre-commit
```
