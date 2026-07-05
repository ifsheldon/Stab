# RPF4 DEM Search And SAT Progress Report

## Scope

This report records the RPF4 progress slice for current capped-repeat graphlike search, hypergraph search, SAT problem generation, analyzer traversal, and ErrorMatcher traversal.

This is not a full folded-traversal implementation. These APIs still expand repeat bodies after passing their flattening or expansion budgets, so the accepted behavior is bounded current traversal plus explicit rejection of hostile repeat expansion.

## Implemented Evidence

- Added PF4-owned core coverage for shifted-repeat graphlike search, hypergraph search, and SAT problem generation in `pf4_dem_search_and_sat_repeat_resource_policy_is_source_owned`.
- The success half proves bounded shifted-repeat DEMs produce the expected graphlike and hypergraph logical error and that SAT problem generation accounts for the expanded repeat errors.
- The rejection half proves graphlike search, hypergraph search, and SAT problem generation reject excessive repeat counts before unbounded materialization.
- Added PF4-owned core coverage for analyzer traversal, ErrorMatcher traversal, repeat-contained noise rejection, nested expansion rejection, and ErrorMatcher filter DEM cap behavior in `pf4_dem_analyzer_repeat_resource_policy_is_source_owned` and `pf4_error_matcher_repeat_resource_policy_is_source_owned`.
- Added implemented oracle metadata row `pf4-dem-search-sat-repeat-resource-rust` to supplement the broad `pf4-dem-folded-traversal` manifest-only row.
- Added implemented oracle metadata row `pf4-dem-analyzer-matcher-repeat-resource-rust` to supplement the broad `pf4-dem-folded-traversal` manifest-only row.
- Added report-only benchmark runners for `pf4-dem-folded-traversal` and `pf4-dem-folded-graphlike-traversal`.

## Benchmark Rows

- `pf4-dem-folded-traversal` now measures current capped-repeat hypergraph search, SAT problem generation, analyzer traversal, and ErrorMatcher traversal with normalized expanded-error or expanded-instruction work.
- `pf4-dem-folded-graphlike-traversal` now measures current capped-repeat graphlike search with normalized expanded-error work.
- Both rows remain non-primary report-only because they do not prove Stim-ratio performance and do not represent true folded traversal.

## Still Open

- True folded graphlike, hypergraph, SAT, analyzer, and ErrorMatcher traversal remains open if Stab chooses to avoid expansion even within the current cap; the sampler now has folded compilation, direct detector sampling, and zero-probability repeat skipping, but repeated nonzero-probability body execution can still be optimized when dense outputs do not require per-occurrence work.
- Broader RPF6 analyzer and search parity remains active beyond the repeat-resource guardrails recorded here.
- The broad RPF4 milestone must not be closed until these remaining traversal consumers are either folded, capped with tests and benchmarks, or explicitly deferred.
