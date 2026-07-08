# PFM2 Feedback Upstream Evidence Repair Scope

## Summary

This PFM2 slice repairs PF2 evidence for two pinned Stim v1.16.0 `circuit_with_inlined_feedback` cases that the existing feedback boundary already claims: `demolition_feedback` and `interleaved_feedback_does_not_reorder_operations`.
The goal is not to broaden feedback-inlining behavior.
The goal is to make the public `Circuit::with_inlined_feedback` claim executable through focused integration tests and PF2 oracle metadata so future audits do not rely only on helper-module tests or broad feedback selectors.

## Owned Surface

- Public Rust API: `stab_core::Circuit::with_inlined_feedback` and `stab_core::circuit_with_inlined_feedback`.
- Public CLI dependency: `stab m2d --ran_without_feedback` continues to use the same helper but receives no new CLI flags or output formats in this slice.
- Checklist rows: circuit transforms and full feedback-inlining transform parity.
- Pinned upstream source: `vendor/stim/src/stim/util_top/transform_without_feedback.test.cc`.
- Comparator class: exact canonical transformed circuit text for the selected upstream cases.
- Oracle row: add `pf2-feedback-inline-pinned-upstream-rust` as focused structural evidence for the public method selector.
- Benchmark row: no new benchmark row because this repairs exact-output evidence for existing public-method behavior; `pf2-feedback-inline-batch` remains the report-only performance row for the scoped helper.

## Selected Positive Scope

- Port the pinned `demolition_feedback` case exactly, proving that detector and observable rewrites preserve the same canonical output as Stim v1.16.0.
- Port the three pinned interleaved-ordering examples exactly, proving that empty controlled-Pauli instructions are dropped without reordering neighboring operations and that feedback removal does not move later measurements before earlier measurements.
- Keep the existing helper-module `basic`, `demolition_feedback`, `interleaved_order`, `loop`, and `mpp` tests plus the existing `XCZ` or `YCZ`, bounded repeat, and rejection integration tests as separate evidence.

## Explicit Rejections And Non-Goals

- Do not add new accepted feedback gate families.
- Do not change repeat-contained feedback behavior.
- Do not change `m2d --ran_without_feedback` CLI semantics.
- Do not claim broader `Circuit.with_inlined_feedback` parity beyond the selected pinned upstream cases and already documented source-owned cases.
- Broader repeat-contained feedback remains under-specified until a future exact-subcase plan names repeat structures, comparator behavior, resource behavior, oracle metadata, and benchmark policy.

## Tests

- Add `with_inlined_feedback_matches_pinned_demolition_feedback`.
- Add `with_inlined_feedback_matches_pinned_interleaved_ordering`.
- The tests should assert exact canonical Stab output matching the pinned Stim expected circuits, not only DEM equivalence or absence of feedback instructions.

## Done Criteria

- The new focused tests pass without weakening existing rejection coverage.
- `oracle/fixtures/manifest.csv` includes a focused structural row for the pinned upstream evidence repair.
- `docs/plans/rpf2-circuit-transform-progress-report.md`, `docs/plans/pfm2-feedback-repeat-boundary-scope.md`, `docs/plans/partial-feature-inventory.md`, and `docs/stab-feature-checklist.md` point the selected upstream public-method claim to focused evidence instead of relying only on helper-module tests or broad feedback selectors.
- `cargo test -p stab-core --test circuit_transforms with_inlined_feedback_matches_pinned --quiet`, `cargo test -p stab-oracle fixtures --quiet`, `just oracle::run --milestone PF2 --structural`, and `just bench::smoke` pass before commit.
