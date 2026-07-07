# PFM6 Direct DEM Sparse High-Detector Search Scope

## Scope

This slice owns direct DEM graphlike and hypergraph search resource behavior when the model has very high declared detector ids but only a sparse set of nonzero-probability error detector targets.
It promotes sparse detector-node indexing inside the graphlike and hypergraph search graphs so selected direct DEMs no longer allocate one dense node per detector id when a small active target set is sufficient.

## Owned Subcases

- Direct graphlike search where zero-probability shifted repeat bodies advance detector offsets past the dense graph cap and later nonzero graphlike errors touch only a sparse detector set.
- Direct hypergraph search for the same high-detector sparse active-target shape.
- Direct graphlike and hypergraph search where sparse high detector ids are connected by two graphlike edges and the returned logical error keeps the original detector ids.
- Policy-aware sparse pre-scans that count only graphlike or hypergraph edges the selected search graph will materialize, including duplicate-detector cancellation before hypergraph degree filtering.
- Early rejection when the materialized sparse detector-node set exceeds the current search cap.
- Sparse-mode diagnostics that continue to use declared detector and observable counts for "no detectors" and "no observables" warnings.
- Existing dense indexing remains used for models whose full detector count is within the current dense graph cap.
- Existing repeat traversal and non-selected repeat-shape caps remain unchanged.

## Explicit Non-Scope

This slice does not implement sparse SAT/WCNF dense-target vectors, sparse ErrorMatcher graphs, generated-loop search folding, broader tie-sensitive output comparators, analyzer provenance, `stim explain_errors`, Python APIs, diagrams, CLI behavior, or any new public simulator product.
SAT/WCNF shifted zero-probability repeat cases that would place later active errors beyond the dense SAT target cap remain rejected.
Graphlike and hypergraph search still reject active repeat shapes that are outside the already selected folded or capped repeat traversal policy.

## Comparator And Evidence

Comparator class: structural Rust parity and resource behavior.
The sparse high-detector direct DEM tests compare graphlike and hypergraph output to exact expected DEM text with original high detector ids.
The resource proof is that graphlike and hypergraph search accept the selected model without allocating dense nodes up to the highest detector id, while the existing SAT rejection test continues to prove SAT/WCNF remains capped for the same dense-target family.

## Oracle Rows

New implemented oracle rows:

- `pf6-search-sparse-high-detectors-graphlike-rust`
- `pf6-search-sparse-high-detectors-hypergraph-rust`

Existing direct search rows remain scoped to pinned distance, canonical-ordering, bounded hyper-error, high-observable, and generated-QEC cases unless their descriptions are explicitly updated.

## Benchmarks

No benchmark row is added.
This is a resource-boundary and correctness admission slice for tiny direct DEMs with sparse active targets, not a throughput path.
The broader generated search and folded traversal benchmark rows remain unchanged.

## Done Criteria

- Graphlike and hypergraph search construct sparse internal node indexes when the full detector count is above the dense cap but the active detector target set is within the cap.
- Sparse pre-scans apply the same graphlike ignore policy and hypergraph degree policy used by graph construction.
- Search-state traversal maps active detector ids back to sparse node indexes without changing returned DEM detector ids.
- Tests prove accepted high-detector sparse direct DEM graphlike and hypergraph outputs.
- Tests prove the existing SAT dense-target rejection remains in force for the analogous shifted zero-probability family.
- Tests prove sparse diagnostics use declared detector and observable counts and that hypergraph duplicate detector targets are degree-filtered after cancellation.
- Documentation and oracle metadata name the exact promoted resource behavior and keep broader PF4/PF6 work open.
