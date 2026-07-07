# PFM5 Detecting-Regions Target-Shape Evidence Lock

## Scope

This evidence lock closes a stale PFM5 planning ambiguity around detecting-region target shapes that are parsed by the `.stim` parser but intentionally unsupported by the current Rust `circuit_detecting_regions` utility.
It does not add new production behavior.
The owned subcases are the source-owned fail-closed detecting-region boundary for non-`CZ` classical-bit controlled-Pauli groups:

- `CX`, `CY`, `XCZ`, and `YCZ` with two sweep-bit targets.
- `CX`, `CY`, `XCZ`, and `YCZ` with mixed measurement-record and sweep-bit targets.
- `CX`, `CY`, `XCZ`, and `YCZ` with two measurement-record targets.

These cases previously appeared in a remaining-work list even though they are already tested as explicit rejections under the selected PF5 target-shape row.

## Explicit Non-Scope

- Promoting any of these non-`CZ` classical-bit target groups as detecting-region no-ops.
- Changing analyzer, sampler, detection-conversion, sparse-tracker, flow-generator, or feedback-transform behavior.
- Broader generated-code region tables, coordinate-prefix filters, broader gauge behavior, broader missing-detector analysis, measurement-rich flow solving, Python binding ergonomics, diagrams, JS/WASM, or simulator products.

## Comparator And Evidence

Comparator class: structural Rust API fail-closed evidence.
Pinned Stim v1.16.0 parser acceptance alone is not sufficient to promote a detecting-region shape.
For this Stab Rust utility surface, unpromoted target shapes must reject with actionable domain errors before returning misleading regions.

Existing evidence:

- `detecting_regions_target_shape_keeps_non_cz_sweep_sweep_fail_closed` in `crates/stab-core/tests/detecting_regions_cz_sweep_sweep.rs` covers non-`CZ` sweep/sweep groups.
- `detecting_regions_target_shape_keeps_non_cz_record_sweep_fail_closed` in `crates/stab-core/tests/detecting_regions_cz_sweep_sweep.rs` covers non-`CZ` record/sweep groups.
- `detecting_regions_target_shape_keeps_non_cz_record_record_fail_closed` in `crates/stab-core/tests/detecting_regions_cz_classical_noop.rs` covers non-`CZ` record/record groups.
- Oracle row `pf5-detecting-regions-target-shapes-rust` runs `cargo test -p stab-core detecting_regions_target_shape`, which selects the fail-closed tests above.

## Benchmarks

No benchmark row is added.
This is a validation and documentation evidence-lock slice.
The relevant target-shape behavior changes only validation and no-op traversal branches in an existing Rust utility row, and the existing `pf5-detecting-regions-targets` report-only benchmark remains the representative utility workload for target-filter traversal.

## Documentation Updates

The active plan, feature checklist, partial-feature inventory, roadmap, and RPF5 progress report should describe these non-`CZ` classical-bit groups as source-owned fail-closed evidence, not as active unclassified target-shape work.
Broader detecting-region work remains active only for target-shape families not covered by the promoted positive set or the source-owned fail-closed set, plus broader generated-code and gauge behavior.

## Done Criteria

- The PF5 target-shape oracle row continues to select the fail-closed tests.
- Documentation no longer lists non-`CZ` sweep/sweep, record/sweep, or record/record detecting-region groups as active unclassified work.
- No production behavior changes.
