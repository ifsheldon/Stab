# PFM4 DEM Search Invalid Target Boundary Scope

## Summary

This scope note locks a PFM4 search-traversal correction: raw numeric `error` targets and separator-only `error` target lists are not valid public DEM search inputs in Stab.
They should be treated as typed-boundary validation evidence, not as active folded graphlike or hypergraph traversal work.

## Boundary

- `error` instructions cannot target raw numeric values.
- `error` target separators cannot be first, consecutive, or last, so a separator-only target list is rejected before a `DetectorErrorModel` can be constructed.
- Valid separated error target groups contain detector or logical-observable targets and are already handled by the existing graphlike, hypergraph, flattening, and ErrorMatcher paths that preserve decomposition separators.
- Search traversal does not need to promote a separator-only compact-repeat fold because the public parser and `DemInstruction::error` constructor reject that shape.

## Evidence

- `pf4_dem_public_validation_rejects_malformed_inputs` covers malformed DEM text and programmatic constructor validation, including invalid separators, separator-only target lists, and raw numeric error targets.
- Oracle row `pf4-dem-validation-negative-rust` selects the PF4 validation evidence.
- Existing graphlike, hypergraph, and ErrorMatcher rows continue to cover valid separator-preserving behavior where decomposition separators appear between detector or logical-observable target groups.

## Non-Goals

- This note does not change graphlike, hypergraph, SAT/WCNF, analyzer, ErrorMatcher, DEM sampler, CLI, Python, diagram, or simulator-product behavior.
- This note does not promote raw numeric error targets, malformed separator lists, nonzero-shift active repeats, shifted nested repeats, non-flat active repeats, or broader mixed-instruction active repeats.
- This note does not add a benchmark row because rejected typed-boundary validation is not a throughput traversal path.

## Verification

```sh
cargo test -p stab-core --test dem_api pf4_dem_public_validation_ --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF4 --structural
just bench::smoke
```
