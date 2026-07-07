# PFM5 Missing-Detectors MPAD Scope

## Summary

This PFM5 slice promotes measurement-pad support in the Rust `missing_detectors` utility.
Pinned Stim v1.16.0 treats deterministic and probabilistic `MPAD` targets as measurement records whose values are deterministic enough for missing-detector suggestion purposes, and this slice matches that selected behavior for the public Rust API.

This slice does not broaden generated-code suffix analysis, folded large-repeat traversal, flow solving, detecting regions, CLI behavior, Python bindings, or full detector-utility parity.

## Selected Surface

- Public Rust API: `stab_core::missing_detectors`.
- Active plan row: PFM5 `missing_detectors` utility families.
- Comparator: structural Rust API parity against pinned Stim v1.16.0 expected suffix text from `Circuit.missing_detectors`.
- Oracle row: add `pf5-missing-detectors-mpad-rust` as a focused structural row.
- Benchmark row: add report-only `pf5-missing-detectors-mpad` because this is a new promoted `missing_detectors` target family, but keep it out of primary timing gates because there is no faithful pinned Stim CLI timing ratio for this Rust utility surface.

## Included Cases

- `MPAD 0`, `MPAD 1`, and multi-target `MPAD 0 1` produce missing-detector suggestions for uncovered pad records.
- `MPAD(0.5) 0` follows pinned Stim missing-detector behavior and still produces an uncovered-record suggestion.
- Existing detector rows and record-only observable rows reduce or cover `MPAD` measurement rows using the same Gaussian cleanup as other deterministic records.
- Known-input and unknown-input modes agree for the selected pad-only examples.
- Small repeats containing `MPAD` are supported by the existing bounded traversal path.
- Huge repeats containing `MPAD` remain capped by the existing missing-detector repeat budget.

## Explicit Non-Goals

- No new public CLI command or flag.
- No new folded final-repeat fast path for unbounded uncovered `MPAD` loops.
- No generated-code suffix expansion.
- No claim that broader missing-detector gauge, row-reduction, unknown-input, folded-repeat, or generated-code families are complete.

## Tests And Evidence

- Add `pf5_missing_detectors_mpad_measurement_pads_match_pinned_stim` to `crates/stab-core/tests/missing_detectors.rs`.
- Add `pf5-missing-detectors-mpad-rust` to `oracle/fixtures/manifest.csv`, selecting the focused integration test.
- Add report-only benchmark metadata and runner coverage for `pf5-missing-detectors-mpad`.
- Update `docs/plans/rpf5-missing-detectors-progress-report.md`, `docs/plans/non-deferred-partial-feature-milestones.md`, `docs/plans/partial-feature-inventory.md`, `docs/stab-feature-checklist.md`, and roadmap text where they describe the selected `missing_detectors` MPAD evidence.
