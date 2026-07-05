# RPF6 Search Progress Report

## Summary

This report records the promoted PF6 generated-QEC graphlike, hypergraph, and SAT/WCNF search subset.
It adds executable Rust evidence and report-only benchmark runners for generated rotated-surface-code and repetition-code logical-error search and selected generated-QEC SAT/WCNF encoding without claiming full PF6 search parity.

## Implemented Surfaces

- Generated rotated-surface-code graphlike search with decomposed graphlike DEMs, matching the pinned Stim v1.16.0 generated-search instruction count of 5.
- Generated repetition-code graphlike search with decomposed graphlike DEMs, matching the pinned Stim v1.16.0 generated-search instruction count of 7.
- Generated rotated-surface-code graphlike search with ungraphlike DEMs and `ignore_ungraphlike_errors=true`, matching the pinned Stim v1.16.0 instruction count of 5.
- Generated rotated-surface-code graphlike search rejection for ungraphlike DEMs when `ignore_ungraphlike_errors=false`.
- Generated rotated-surface-code and repetition-code hypergraph search with the pinned Stim v1.16.0 instruction counts of 5 and 7.
- Generated rotated-surface-code and repetition-code shortest and weighted SAT/WCNF encoding produce nontrivial WDIMACS output with positive soft and hard clauses.
- Ungraphlike generated rotated-surface-code DEMs still produce structural shortest SAT/WCNF output instead of depending on graphlike decomposition.
- Weighted SAT/WCNF output now keeps the header clause count equal to the emitted clause lines even when low quantization rounds a soft clause to zero.

## Tests

Implemented Rust tests:

- `pf6_generated_qec_graphlike_search_matches_upstream_instruction_counts`
- `pf6_generated_qec_hypergraph_search_matches_upstream_instruction_counts`
- `pf6_generated_sat_wcnf_qec_encoding_is_structural`

The tests live in `crates/stab-core/tests/dem_search.rs` and are derived from `vendor/stim/src/stim/search/graphlike/algo.test.cc`, `vendor/stim/src/stim/search/hyper/algo.test.cc`, and `vendor/stim/src/stim/search/sat/wcnf.test.cc`.
They assert the generated-code search result sizes that pinned Stim v1.16.0 asserts, require error-only DEM output, require at least one logical observable in the returned logical error, cover the ungraphlike generated surface-code rejection path, and structurally verify WDIMACS headers, emitted clause counts, soft clauses, hard clauses, and weighted top weights for selected generated-QEC SAT/WCNF encodings.

## Oracle Rows

Implemented rows:

- `pf6-search-generated-qec-rust`
- `pf6-search-generated-sat-wcnf-rust`

The broad row `pf6-search-generated` remains manifest-only because full generated-circuit search parity still includes broader generated-code families, ordering-insensitive search outputs beyond instruction counts, broader SAT or WCNF families, additional resource behavior, and sparse reverse tracker integration.

## Benchmark Rows

Rows with new report-only runner coverage:

- `pf6-graphlike-search-generated`, measured as `stab_pf6_graphlike_search_generated_surface`.
- `pf6-hypergraph-search-generated`, measured as `stab_pf6_hypergraph_search_generated_surface`.
- `pf6-generated-sat-wcnf`, measured as `stab_pf6_shortest_sat_generated_surface` and `stab_pf6_likeliest_sat_generated_surface`.

These rows measure generated rotated-surface-code DEM search or SAT/WCNF encoding after source-owned Rust analysis and decomposition.
They remain `non-primary-report-only` and `contract-only` because pinned Stim exposes these search and SAT APIs through C++ API and perf surfaces, not a faithful public CLI baseline for Stab.
They were not added to `benchmarks/m12-primary-thresholds.json`.
Fresh local probe command `just bench::compare --only pf6-generated-sat-wcnf --baseline target/benchmarks/pf6-generated-sat-wcnf-probe/baseline.json --report target/benchmarks/pf6-generated-sat-wcnf-compare` measured `stab_pf6_shortest_sat_generated_surface=0.002952415s`, or approximately `8.129e3 detectors/s`, and `stab_pf6_likeliest_sat_generated_surface=0.001445527s`, or approximately `1.660e4 detectors/s`, as report-only evidence on the local machine.

## Remaining PF6 Search Work

- Broader generated-circuit search families beyond the promoted rotated-surface-code and repetition-code cases.
- Exact or structural comparators for ordering-insensitive search outputs beyond result instruction counts.
- Broader generated SAT or WCNF encoding families beyond the selected generated-QEC structural row.
- Loop-folded generated search behavior.
- Sparse reverse detector-frame tracker analyzer/search integration beyond the promoted supported-Clifford generated repeat-folding subset.
- Full ErrorMatcher provenance, heralded matching, repeat-contained noise stack frames, and `stim explain_errors` CLI remain deferred.

## Audit And Review Notes

Milestone-audit found the selected generated-QEC SAT/WCNF evidence complete against the current PFM6 text after keeping broader generated SAT/WCNF families and ordering-insensitive search comparators open.
The local audit also found stale test-source wording in this report, which now names `vendor/stim/src/stim/search/sat/wcnf.test.cc`.
The GPT-5.5/xhigh benchmark sidecar found no benchmark or metadata issues and confirmed that `pf6-generated-sat-wcnf` is `non-primary-report-only`, `contract-only`, has runner smoke coverage, has measurement work for both submeasurements, maps to `report-only` via `compare_note`, and stays out of the primary gate.
The GPT-5.5/xhigh core/test sidecar found that weighted WCNF headers could over-count clauses when quantization rounded a soft clause to zero and that the old `pf6-search-generated-qec-rust` oracle filter accidentally included the new SAT/WCNF test.
The encoder now counts only emitted WCNF clauses, a direct SAT regression covers the zero-quantized soft-clause case, the generated SAT/WCNF test checks emitted line counts against the header, and the oracle filters are disjoint.

## Verification Evidence

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
cargo test -p stab-core sat_problem --quiet
cargo test -p stab-core --test dem_search pf6_generated_qec_ --quiet
cargo test -p stab-core --test dem_search pf6_generated_sat_wcnf_qec --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench pf6_analyzer_benchmark_rows_have_stab_compare_runners --quiet
just oracle::run --milestone PF6 --structural
just bench::smoke
just bench::baseline --only pf6-generated-sat-wcnf --out target/benchmarks/pf6-generated-sat-wcnf-probe
just bench::compare --only pf6-generated-sat-wcnf --baseline target/benchmarks/pf6-generated-sat-wcnf-probe/baseline.json --report target/benchmarks/pf6-generated-sat-wcnf-compare
just maintenance::pre-commit
```
