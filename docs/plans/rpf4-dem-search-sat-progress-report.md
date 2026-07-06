# RPF4 DEM Search And SAT Progress Report

## Scope

This report records the RPF4 progress slice for current capped-repeat graphlike search, hypergraph search, SAT problem generation, analyzer traversal, and ErrorMatcher traversal.
It also records the later zero-probability repeat-skip slices for graphlike search and hypergraph search, selected flat detector-touching zero-shift repeat folding for graphlike and hypergraph search, zero-probability variable elision and repeated-body skipping for weighted SAT/WCNF generation, and the selected flat zero-shift SAT/WCNF repeat-folding slice.

This is not a full folded-traversal implementation. Graphlike and hypergraph traversal skip repeated bodies that contain no nonzero-probability error mechanisms and fold selected flat detector-touching zero-shift repeated error bodies into compact search-graph edges, weighted SAT/WCNF omits zero-probability error variables and skips repeated zero-probability bodies before flattening, weighted SAT/WCNF folds selected flat zero-shift repeat bodies by concrete MAP parity cost for nonzero mechanisms, and unweighted SAT folds the selected all-nonzero body shape structurally. Shifted, nested, non-flat, detectorless logical-only, analyzer, ErrorMatcher, and repeated zero-probability unweighted SAT bodies still keep explicit caps or current dense target limits.

## Implemented Evidence

- Added PF4-owned core coverage for shifted-repeat graphlike search, hypergraph search, and SAT problem generation in `pf4_dem_search_and_sat_repeat_resource_policy_is_source_owned`.
- The success half proves bounded shifted-repeat DEMs produce the expected graphlike and hypergraph logical error and that SAT problem generation accounts for the expanded repeat errors.
- The rejection half proves graphlike search, hypergraph search, and SAT problem generation reject excessive repeat counts before unbounded materialization.
- Added PF4-owned core coverage in `pf4_dem_search_skips_zero_probability_repeat_bodies` proving graphlike and hypergraph search skip excessive repeated bodies containing only zero-probability errors, avoid materializing oversized graph nodes from ignored zero-probability repeat targets, and leave unweighted SAT capped for the same model.
- Added PF4-owned core coverage in `pf4_dem_search_weighted_sat_skips_zero_probability_repeat_bodies` plus SAT unit coverage in `sat_problem_likeliest_omits_zero_probability_error_variables` and `sat_problem_likeliest_skips_zero_probability_repeats`, proving weighted SAT/WCNF omits flat zero-probability error variables and skips repeated zero-probability bodies before flattening while unweighted SAT remains structurally capped for the same repeated model.
- Added PF4-owned core coverage in `pf4_dem_search_weighted_sat_rejects_shifted_zero_probability_repeat_node_explosion` plus SAT unit coverage in `sat_problem_likeliest_rejects_shifted_zero_probability_repeat_node_explosion`, proving skipped zero-probability repeats cannot shift later nonzero errors into huge dense SAT detector vectors.
- Added PF4-owned core coverage in `pf4_dem_search_sat_folds_flat_nonzero_zero_shift_repeat_bodies` plus SAT unit coverage in `sat_problem_shortest_folds_large_flat_zero_shift_repeats`, `sat_problem_likeliest_folds_large_flat_zero_shift_repeats_by_map_cost`, and `sat_problem_likeliest_treats_deterministic_error_as_hard`, proving selected large flat all-nonzero zero-shift repeat bodies are compacted for unweighted SAT and folded by concrete MAP parity cost for weighted SAT/WCNF.
- Added PF4-owned core coverage in `pf4_dem_search_folds_flat_nonzero_zero_shift_repeat_bodies`, proving selected large flat detector-touching zero-shift repeat bodies produce graphlike and hypergraph search outputs matching the compact single-body model while detectorless logical-only repeats remain capped until that corner is specified separately.
- Added `pf4_dem_search_rejects_shifted_zero_probability_repeat_node_explosion` proving shifted zero-probability repeats that place later active errors beyond the current dense search-graph node cap fail with explicit graphlike and hypergraph domain errors instead of allocating huge node vectors.
- Added PF4-owned core coverage for analyzer traversal, ErrorMatcher traversal, repeat-contained noise rejection, nested expansion rejection, and ErrorMatcher filter DEM cap behavior in `pf4_dem_analyzer_repeat_resource_policy_is_source_owned` and `pf4_error_matcher_repeat_resource_policy_is_source_owned`.
- Added implemented oracle metadata row `pf4-dem-search-sat-repeat-resource-rust` to supplement the broad `pf4-dem-folded-traversal` manifest-only row.
- Added implemented oracle metadata row `pf4-dem-analyzer-matcher-repeat-resource-rust` to supplement the broad `pf4-dem-folded-traversal` manifest-only row.
- Added report-only benchmark runners for `pf4-dem-folded-traversal`, `pf4-dem-sat-flat-repeat-fold`, and `pf4-dem-folded-graphlike-traversal`.

## Benchmark Rows

- `pf4-dem-folded-traversal` now measures current capped-repeat hypergraph search, hypergraph zero-probability repeat skipping, selected flat detector-touching zero-shift hypergraph repeat folding, capped unweighted SAT problem generation, weighted SAT zero-probability variable elision and repeated-body skipping, analyzer traversal, and ErrorMatcher traversal with normalized expanded-error, skipped-error, folded-error, or expanded-instruction work.
- `pf4-dem-sat-flat-repeat-fold` measures selected unweighted SAT flat all-nonzero zero-shift repeat folding and selected weighted SAT flat zero-shift concrete-MAP repeat folding with normalized folded-error work.
- `pf4-dem-folded-graphlike-traversal` now measures current capped-repeat graphlike search, graphlike zero-probability repeat skipping, and selected flat detector-touching zero-shift graphlike repeat folding with normalized expanded-error, skipped-error, or folded-error work.
- These rows remain non-primary report-only because they do not prove Stim-ratio performance and do not represent true folded traversal.

## Verification

- `cargo test -p stab-core sat_problem --quiet`
- `cargo test -p stab-core --test dem_search pf4_dem_search_ --quiet`
- `cargo test -p stab-bench pf4_dem_transform_benchmark_rows_have_stab_compare_runners --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `cargo test -p stab-bench --quiet`
- `just oracle::run --milestone PF4 --structural`
- `just bench::smoke`
- `just bench::baseline --only pf4-dem-folded-traversal --out target/benchmarks/pf4-weighted-sat-zero-prob-baseline`
- `just bench::compare --only pf4-dem-folded-traversal --baseline target/benchmarks/pf4-weighted-sat-zero-prob-baseline/baseline.json --report target/benchmarks/pf4-weighted-sat-zero-prob-compare`
- `just bench::baseline --only pf4-dem-sat-flat-repeat-fold --out target/benchmarks/pfm4-dem-sat-flat-repeat-baseline`
- `just bench::compare --only pf4-dem-sat-flat-repeat-fold --baseline target/benchmarks/pfm4-dem-sat-flat-repeat-baseline/baseline.json --report target/benchmarks/pfm4-dem-sat-flat-repeat-compare`
- `just bench::baseline --only pf4-dem-folded-graphlike-traversal --out target/benchmarks/pfm4-dem-search-flat-repeat-graphlike-baseline`
- `just bench::compare --only pf4-dem-folded-graphlike-traversal --baseline target/benchmarks/pfm4-dem-search-flat-repeat-graphlike-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-flat-repeat-graphlike-compare`
- `just bench::baseline --only pf4-dem-folded-traversal --out target/benchmarks/pfm4-dem-search-flat-repeat-hyper-baseline`
- `just bench::compare --only pf4-dem-folded-traversal --baseline target/benchmarks/pfm4-dem-search-flat-repeat-hyper-baseline/baseline.json --report target/benchmarks/pfm4-dem-search-flat-repeat-hyper-compare`

The latest focused local compare for the selected flat-repeat SAT slice measured `stab_pf4_dem_sat_flat_repeat_fold=0.000005550s`, which normalizes to approximately `3.604e11 folded-errors/s`, and `stab_pf4_dem_weighted_sat_flat_repeat_fold=0.000007688s`, which normalizes to approximately `2.601e11 folded-errors/s`.
The latest focused local compare for the selected flat-repeat search slice measured `stab_pf4_dem_graphlike_flat_repeat_fold=0.000001024s`, which normalizes to approximately `1.953e12 folded-errors/s`, and `stab_pf4_dem_hyper_flat_repeat_fold=0.000001064s`, which normalizes to approximately `1.880e12 folded-errors/s`.
The earlier broad folded-traversal compare measured `stab_pf4_dem_weighted_sat_zero_probability_repeat_skip=0.000001148s`, which normalizes to approximately `8.711e11 skipped-zero-probability-errors/s`.

## Audit And Review

- `milestone-audit` status for the original promoted slice is complete with follow-up work still open for full folded traversal. The implemented evidence covers weighted SAT zero-probability variable elision, repeated-body skipping, unweighted structural caps, and dense-target rejection for shifted skipped repeats.
- `milestone-audit` status for the selected flat zero-shift SAT/WCNF repeat-folding slice is complete. The implemented evidence covers unweighted all-nonzero structural folding, weighted concrete MAP parity-cost folding, deterministic weighted hard-clause handling, SAT/WCNF oracle provenance, and report-only benchmark evidence, while keeping shifted, nested, non-flat, analyzer, ErrorMatcher, graphlike, hypergraph, and unweighted zero-probability structural repeat work outside the slice.
- `milestone-audit` status for the selected flat detector-touching zero-shift graphlike and hypergraph search repeat-folding slice is complete. The implemented evidence covers compact-model semantic parity for graphlike and hypergraph search, detectorless logical-only exclusion tests, PF4 oracle metadata, report-only benchmark evidence, and explicit non-goals for shifted, nested, non-flat, analyzer, ErrorMatcher, sampled-error, and replay paths.
- `full-code-review` used two GPT-5.5/xhigh subagents. The docs and benchmark reviewer found a provenance issue where the M10 WCNF oracle row had absorbed PF4 resource-hardening claims; the row was restored to upstream WCNF parity wording, and the PF4 oracle row now owns the new resource behavior. The core reviewer found a P1 dense detector allocation risk after shifted skipped repeats; weighted SAT now rejects detector or observable vectors above the dense SAT target cap, with focused unit and PF4 oracle-filtered tests.
- A follow-up `full-code-review` pass for the selected flat zero-shift SAT/WCNF repeat-folding slice also used two GPT-5.5/xhigh sidecars. The core reviewer found that odd-parity marginal probability would change weighted SAT concrete MAP semantics; weighted folding now preserves concrete MAP parity cost and has a regression counterexample. The docs and benchmark reviewer found scope/provenance drift; the SAT flat-fold evidence now has its own SAT-sourced oracle row and benchmark row, and stale verification commands were corrected.
- A follow-up `full-code-review` pass for the selected flat detector-touching zero-shift graphlike and hypergraph search repeat-folding slice also used two GPT-5.5/xhigh sidecars. The core sidecar and the docs/oracle/benchmark sidecar found no evidence-backed blocking issues.

## Still Open

- Broader folded graphlike, hypergraph, analyzer, and ErrorMatcher traversal remains open if Stab chooses to avoid expansion even within the current cap; SAT/WCNF still needs broader shifted, nested, non-flat, and zero-probability unweighted repeat handling beyond the selected flat zero-shift body shape. The sampler now has folded compilation, direct detector sampling, zero-probability repeat skipping, deterministic zero-shift parity folding, and selected stochastic zero-shift folding, graphlike and hypergraph skip zero-probability repeated bodies and fold selected flat detector-touching zero-shift repeated bodies, and weighted SAT/WCNF omits zero-probability variables, skips zero-probability repeated bodies, and folds selected flat zero-shift repeated bodies by concrete MAP parity cost.
- Broader RPF6 analyzer and search parity remains active beyond the repeat-resource guardrails recorded here.
- The broad RPF4 milestone must not be closed until these remaining traversal consumers are either folded, capped with tests and benchmarks, or explicitly deferred.
