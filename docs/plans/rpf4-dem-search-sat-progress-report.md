# RPF4 DEM Search And SAT Progress Report

## Scope

This report records the RPF4 progress slice for current capped-repeat graphlike search, hypergraph search, and SAT problem generation.

This is not a full folded-traversal implementation. These APIs still expand repeat bodies after passing the shared flattening budget, so the accepted behavior is bounded current traversal plus explicit rejection of hostile repeat expansion.

## Implemented Evidence

- Added PF4-owned core coverage for shifted-repeat graphlike search, hypergraph search, and SAT problem generation in `pf4_dem_search_and_sat_repeat_resource_policy_is_source_owned`.
- The success half proves bounded shifted-repeat DEMs produce the expected graphlike and hypergraph logical error and that SAT problem generation accounts for the expanded repeat errors.
- The rejection half proves graphlike search, hypergraph search, and SAT problem generation reject excessive repeat counts before unbounded materialization.
- Added implemented oracle metadata row `pf4-dem-search-sat-repeat-resource-rust` to supplement the broad `pf4-dem-folded-traversal` manifest-only row.
- Added report-only benchmark runners for `pf4-dem-folded-traversal` and `pf4-dem-folded-graphlike-traversal`.

## Benchmark Rows

- `pf4-dem-folded-traversal` now measures current capped-repeat hypergraph search and SAT problem generation with normalized expanded-error work.
- `pf4-dem-folded-graphlike-traversal` now measures current capped-repeat graphlike search with normalized expanded-error work.
- Both rows remain non-primary report-only because they do not prove Stim-ratio performance and do not represent true folded traversal.

## Still Open

- True folded graphlike, hypergraph, and SAT traversal remains open if Stab chooses to avoid expansion even within the current cap.
- Matcher-adjacent and analyzer-adjacent DEM traversal still need PF4-owned evidence or explicit documented caps.
- The broad RPF4 milestone must not be closed until these remaining traversal consumers are either folded, capped with tests and benchmarks, or explicitly deferred.
