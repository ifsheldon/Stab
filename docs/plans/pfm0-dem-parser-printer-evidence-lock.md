# PFM0 DEM Parser And Printer Evidence Lock

## Summary

This PFM0 slice closes the checklist row for the `.dem` parser and canonical printer without changing parser behavior.
The row was still marked `Partial` because nearby DEM API, transform, coordinate, and folded-traversal rows remain partial.
Those broader gaps are real, but they are not parser or canonical-printer gaps and now stay on their own checklist rows.

## Closed Surface

- `DetectorErrorModel::from_dem_str` parses the current `.dem` model surface.
- `DetectorErrorModel::to_dem_string` prints canonical `.dem` text for the implemented model surface.
- `m10-dem-parse-print-exact` compares the canonical printer output against pinned Stim v1.16.0 text for the exact parse-print fixture.
- `m10-dem-parse-print-basic` proves parse-print-parse preservation structurally.
- `coverage-py-dem` runs the mined DetectorErrorModel semantic coverage for parse-print canonicalization, coordinates, shifts, observables, and repeat blocks through Rust tests.
- PF4 validation rows cover malformed text, invalid probabilities, invalid separators, invalid targets, invalid repeat counts, invalid tags, high detector and observable ids, detector-shift overflow, and non-finite coordinate rejection.

## Still Active Elsewhere

- DEM detector-coordinate and count resource behavior remains tracked by the DEM detector shifts, observables, coordinates, and counts row.
- DEM transform behavior remains tracked by the DEM transforms row.
- Folded traversal and public consumer caps remain tracked by the DEM flattening and large repeat traversal row.
- Full DEM public API parity remains partial because diagrams, Python ergonomics, and complete folded traversal across every consumer are not closed by parser/printer evidence.

## Verification Commands

```sh
cargo test -p stab-core dem_parse_print --quiet
cargo test -p stab-core --test dem_api pf4_dem_public_validation_ --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone M10 --exact
just oracle::run --milestone M10 --structural
just oracle::matrix --check
just oracle::list
```

## Acceptance

The `.dem` parser and canonical-printer row can remain `Done` as long as these exact and structural evidence rows stay implemented.
Any future parser or printer behavior change must rerun the verification commands above and update this report, the checklist, oracle metadata, and the active milestone plan in the same change set.
