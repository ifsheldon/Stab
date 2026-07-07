# PFM2 Feedback Repeat Boundary Scope

## Summary

This scope note locks the current PFM2 `Circuit::with_inlined_feedback` repeat-contained feedback boundary.
The selected implementation covers the pinned Stim v1.16.0 repeat-loop case plus one source-owned nested bounded-repeat detector-parity case, and it does not claim full repeat-contained feedback parity.

## Selected Evidence

- Pinned upstream source: `vendor/stim/src/stim/util_top/transform_without_feedback.test.cc`.
- Upstream cases already selected and implemented: `basic`, `demolition_feedback`, `loop`, `mpp`, and `interleaved_feedback_does_not_reorder_operations`.
- Additional source-owned selected cases: `XCZ` or `YCZ` measurement-record feedback equivalence, selected nested bounded-repeat `CY` and `CZ` detector-parity preservation, excessive repeat-work rejection, and unsupported classical-control rejection.
- Public Rust surface: `stab_core::Circuit::with_inlined_feedback` and `stab_core::circuit_with_inlined_feedback`.
- Public CLI dependency: `stab m2d --ran_without_feedback` uses the same scoped helper.

## Comparator And Resource Policy

- Exact canonical-output comparison owns the pinned upstream `loop` shape.
- DEM-equivalence comparison owns the selected nested bounded-repeat detector-parity shape.
- Public transform input remains bounded by `MAX_FEEDBACK_REPEAT_COUNT`, `MAX_FEEDBACK_REPEAT_WORK_UNITS`, and `MAX_FEEDBACK_REPEAT_NESTING`.
- Unsupported classical-control shapes and unselected repeat feedback shapes must fail closed or wait for a future exact-subcase plan.

## Non-Goals

- This slice does not add new feedback gate families, new repeat-body shapes, new CLI flags, Python binding behavior, or full Stim transform API parity.
- Broader repeat-contained feedback parity is under-specified until a future plan names exact repeat structures, feedback gate and target shapes, comparator behavior, resource-boundary behavior, oracle metadata, and benchmark policy.

## Verification Commands

- `cargo test -p stab-core circuit_with_inlined_feedback --quiet`
- `cargo test -p stab-core --test circuit_transforms feedback --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::list --milestone PF2`
- `just bench::smoke`
