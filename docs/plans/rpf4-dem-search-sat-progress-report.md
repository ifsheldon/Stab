# RPF4 DEM Search And SAT Progress Report

## Scope

This report records the RPF4 progress slice for current capped-repeat graphlike search, hypergraph search, SAT problem generation, analyzer traversal, and ErrorMatcher traversal.
It also records the later zero-probability repeat-skip slices for graphlike search and hypergraph search, plus zero-probability variable elision and repeated-body skipping for weighted SAT/WCNF generation.

This is not a full folded-traversal implementation. Graphlike and hypergraph traversal skip repeated bodies that contain no nonzero-probability error mechanisms, and weighted SAT/WCNF omits zero-probability error variables and skips repeated zero-probability bodies before flattening, while active repeated error bodies still expand after passing their effective traversal budgets and dense target caps. Unweighted SAT problem generation remains capped because it treats zero-probability errors as structural clauses.

## Implemented Evidence

- Added PF4-owned core coverage for shifted-repeat graphlike search, hypergraph search, and SAT problem generation in `pf4_dem_search_and_sat_repeat_resource_policy_is_source_owned`.
- The success half proves bounded shifted-repeat DEMs produce the expected graphlike and hypergraph logical error and that SAT problem generation accounts for the expanded repeat errors.
- The rejection half proves graphlike search, hypergraph search, and SAT problem generation reject excessive repeat counts before unbounded materialization.
- Added PF4-owned core coverage in `pf4_dem_search_skips_zero_probability_repeat_bodies` proving graphlike and hypergraph search skip excessive repeated bodies containing only zero-probability errors, avoid materializing oversized graph nodes from ignored zero-probability repeat targets, and leave unweighted SAT capped for the same model.
- Added PF4-owned core coverage in `pf4_dem_search_weighted_sat_skips_zero_probability_repeat_bodies` plus SAT unit coverage in `sat_problem_likeliest_omits_zero_probability_error_variables` and `sat_problem_likeliest_skips_zero_probability_repeats`, proving weighted SAT/WCNF omits flat zero-probability error variables and skips repeated zero-probability bodies before flattening while unweighted SAT remains structurally capped for the same repeated model.
- Added PF4-owned core coverage in `pf4_dem_search_weighted_sat_rejects_shifted_zero_probability_repeat_node_explosion` plus SAT unit coverage in `sat_problem_likeliest_rejects_shifted_zero_probability_repeat_node_explosion`, proving skipped zero-probability repeats cannot shift later nonzero errors into huge dense SAT detector vectors.
- Added `pf4_dem_search_rejects_shifted_zero_probability_repeat_node_explosion` proving shifted zero-probability repeats that place later active errors beyond the current dense search-graph node cap fail with explicit graphlike and hypergraph domain errors instead of allocating huge node vectors.
- Added PF4-owned core coverage for analyzer traversal, ErrorMatcher traversal, repeat-contained noise rejection, nested expansion rejection, and ErrorMatcher filter DEM cap behavior in `pf4_dem_analyzer_repeat_resource_policy_is_source_owned` and `pf4_error_matcher_repeat_resource_policy_is_source_owned`.
- Added implemented oracle metadata row `pf4-dem-search-sat-repeat-resource-rust` to supplement the broad `pf4-dem-folded-traversal` manifest-only row.
- Added implemented oracle metadata row `pf4-dem-analyzer-matcher-repeat-resource-rust` to supplement the broad `pf4-dem-folded-traversal` manifest-only row.
- Added report-only benchmark runners for `pf4-dem-folded-traversal` and `pf4-dem-folded-graphlike-traversal`.

## Benchmark Rows

- `pf4-dem-folded-traversal` now measures current capped-repeat hypergraph search, hypergraph zero-probability repeat skipping, capped unweighted SAT problem generation, weighted SAT zero-probability variable elision and repeated-body skipping, analyzer traversal, and ErrorMatcher traversal with normalized expanded-error, skipped-error, or expanded-instruction work.
- `pf4-dem-folded-graphlike-traversal` now measures current capped-repeat graphlike search and graphlike zero-probability repeat skipping with normalized expanded-error or skipped-error work.
- Both rows remain non-primary report-only because they do not prove Stim-ratio performance and do not represent true folded traversal.

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

The latest focused local compare measured `stab_pf4_dem_weighted_sat_zero_probability_repeat_skip=0.000001142s`, which normalizes to approximately `8.757e11 skipped-zero-probability-errors/s`.

## Audit And Review

- `milestone-audit` status for this promoted slice is complete with follow-up work still open for full folded traversal. The implemented evidence covers weighted SAT zero-probability variable elision, repeated-body skipping, unweighted structural caps, and dense-target rejection for shifted skipped repeats.
- `full-code-review` used two GPT-5.5/xhigh subagents. The docs and benchmark reviewer found a provenance issue where the M10 WCNF oracle row had absorbed PF4 resource-hardening claims; the row was restored to upstream WCNF parity wording, and the PF4 oracle row now owns the new resource behavior. The core reviewer found a P1 dense detector allocation risk after shifted skipped repeats; weighted SAT now rejects detector or observable vectors above the dense SAT target cap, with focused unit and PF4 oracle-filtered tests.

## Still Open

- True folded graphlike, hypergraph, SAT, analyzer, and ErrorMatcher traversal remains open if Stab chooses to avoid expansion even within the current cap; the sampler now has folded compilation, direct detector sampling, and zero-probability repeat skipping, graphlike and hypergraph skip zero-probability repeated bodies, and weighted SAT/WCNF omits zero-probability variables and skips zero-probability repeated bodies, but repeated nonzero-probability body execution can still be optimized when dense outputs do not require per-occurrence work.
- Broader RPF6 analyzer and search parity remains active beyond the repeat-resource guardrails recorded here.
- The broad RPF4 milestone must not be closed until these remaining traversal consumers are either folded, capped with tests and benchmarks, or explicitly deferred.
