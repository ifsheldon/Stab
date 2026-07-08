# PFM5 Missing-Detectors Observable-Neutral Final Repeat Scope

## Summary

This PFM5 slice extends the selected folded final-repeat proof in `missing_detectors` to one narrow observable-row case.
The selected case is a final oversized `REPEAT` body where top-level record-only `OBSERVABLE_INCLUDE` rows are redundant because detector rows independently prove that one body iteration has no missing-detector suffix and leaves the invariant tracker unchanged.
The implementation must not use observable rows as the proof of coverage, because repeated observable rows merge by observable id across iterations and can hide missing detector suggestions that appear in small pinned Stim v1.16.0 probes.

## Owned Surface

- Public Rust API: `stab_core::missing_detectors`.
- Checklist row: detector-analysis utility APIs.
- Active plan row: PFM5 `missing_detectors` folded large-repeat traversal.
- Comparator class: structural Rust parity, resource-boundary evidence, and small pinned Stim v1.16.0 semantic probes.
- Oracle row: add `pf5-missing-detectors-observable-neutral-final-repeat-rust` as a focused structural row.
- Benchmark row: no new benchmark row because this is a resource-boundary proof that returns an empty suffix and does not introduce a representative throughput workload separate from existing PF5 report-only rows.

## Selected Positive Scope

Stab may fold a final top-level oversized `REPEAT` body that contains top-level `OBSERVABLE_INCLUDE` rows when all selected folded-final-repeat conditions still hold after removing those observable rows from a proof body.
The only observable rows allowed by this slice are top-level `OBSERVABLE_INCLUDE` instructions whose targets are all local measurement-record references produced inside the same repeat-body iteration.
The proof body must still process successfully, produce an empty missing-detector suffix, and leave the invariant tracker equal to its pre-repeat state without relying on the skipped observable rows.
Positive examples include `M 0; OBSERVABLE_INCLUDE(0) rec[-1]; DETECTOR rec[-1]` and multi-measurement bodies where detector rows cover each local measurement record independently of the observable row.

## Explicit Rejections And Non-Goals

- Observable-only or observable-dependent bodies remain capped instead of folded.
- Duplicate observable rows without detector evidence remain capped because small pinned Stim probes produce missing-detector suggestions.
- `OBSERVABLE_INCLUDE` rows with Pauli targets, combiner targets, or any non-record target remain capped.
- Nested observable rows remain capped until a later scope proves nested observable-neutral folding separately.
- Cross-iteration record references, tracker-changing bodies, nested large repeats, unsupported local bodies, and non-empty suffix bodies keep the existing folded-repeat fallback behavior.
- This slice does not claim general folded missing-detector output, generated-code suffix closure, Python API parity, or arbitrary repeat-contained flow solving.

## Tests

- Add positive tests where huge final repeat bodies with top-level record-only observable rows and independent detector rows return an empty suffix under known-input and reset-prefix shapes.
- Add negative tests where observable-only bodies, duplicate observable-dependent bodies, Pauli observable targets, and nested observable rows still fail through the existing expanded-repeat cap.
- Keep the existing folded final-repeat and nested-final-repeat tests unchanged so the new proof does not weaken cross-iteration, tracker-changing, unsupported-body, or nested-large-repeat rejection.

## Done Criteria

- The implementation removes only selected top-level record-only observable rows from the folded proof body.
- The proof succeeds only when the stripped body independently satisfies the existing empty-suffix and unchanged-tracker checks.
- The materialized API behavior remains unchanged for selected small repeats and for non-selected huge repeats.
- The progress report, roadmap, partial-feature inventory, feature checklist, and oracle manifest name the focused evidence row without claiming broad observable-row folding.
- `cargo test -p stab-core --test missing_detectors pf5_missing_detectors_observable_neutral_final_repeat --quiet`, `cargo test -p stab-oracle fixtures --quiet`, `just oracle::run --milestone PF5 --structural`, and `just bench::smoke` pass before commit.
