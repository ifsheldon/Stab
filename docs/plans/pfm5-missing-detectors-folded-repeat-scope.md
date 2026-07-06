# PFM5 Missing-Detectors Folded Repeat Scope

## Summary

This PFM5 slice promotes one bounded proof of folded repeat traversal in the Rust `missing_detectors` utility.
It does not claim general folded missing-detector output, generated-code suffix closure, Python API parity, or arbitrary repeat-contained flow solving.

## Owned Surface

- Public Rust API: `stab_core::missing_detectors`.
- Checklist row: detector-analysis utility APIs.
- Active plan row: PFM5 `missing_detectors` folded large-repeat traversal.
- Comparator class: structural Rust parity and resource-boundary evidence.
- Oracle row: extend the existing `pf5-missing-detectors-repeat-rust` structural row.
- Benchmark row: no new benchmark row because this slice changes a resource-boundary fast path that returns an empty suffix and does not introduce a representative throughput workload separate from `pf5-missing-detectors-generated-code` or `pf5-missing-detectors-mpp`.

## Selected Positive Scope

Stab should accept a final top-level `REPEAT` whose expanded count would exceed the current materialized repeat budget when all of the following are true:

- The repeat has only bounded, flat body traversal in this slice.
- Every measurement-record target inside the repeat body refers to a measurement produced inside the same repeat body iteration.
- The repeat body contains no `OBSERVABLE_INCLUDE` instruction.
- The bounded prefix has no missing-detector suffix before the repeat.
- Processing one iteration of the repeat body produces no missing-detector suffix.
- Processing one iteration leaves the invariant tracker unchanged from the state immediately before the repeat.
- The repeat is final, so no later instruction can depend on the skipped measurement-record history.

The selected positive examples are deterministic measurement loops whose local detector rows cover each repeated deterministic measurement, including a known-input loop and a reset-in-prefix loop.

## Explicit Rejections And Non-Goals

- Repeats that would produce a non-empty missing-detector suffix remain capped instead of materializing enormous output.
- Repeats whose body changes tracker state remain capped unless a later slice proves a separate fold rule.
- Repeats whose body references measurement records from before the body remain capped because one folded body is not enough evidence for cross-iteration or prefix-dependent row behavior.
- Repeats whose body contains `OBSERVABLE_INCLUDE` remain capped because observable rows merge by observable id across iterations.
- Repeats whose prefix or body cannot be processed by the current missing-detector analyzer remain capped by the original repeat budget instead of returning proof-run errors.
- Nested large repeats remain capped by the existing repeat budget.
- Generated honeycomb, toric, and broader generated-code suffix analysis are not expanded by this slice.
- Python binding behavior and exact C++ implementation internals remain out of scope.

## Tests

- Add a positive resource-boundary test proving a large final deterministic repeat with local detector rows returns an empty suffix instead of failing the expanded-repeat budget.
- Add a positive test for the reset-in-prefix form to cover the tracker-unchanged proof shape selected by this slice.
- Keep the existing excessive-repeat tests for non-selected output-heavy and work-heavy repeat bodies.
- Add negative tests proving cross-iteration record references, observable rows, unsupported local bodies, and tracker-changing repeated bodies remain capped.

## Done Criteria

- The code validates the selected fold before the existing materialized repeat-budget check.
- The fold is additive and falls back to the existing capped path when any proof condition fails.
- Tests cover success, fallback rejection, and unchanged legacy cap behavior.
- `docs/plans/rpf5-missing-detectors-progress-report.md`, `docs/plans/non-deferred-partial-feature-milestones.md`, `docs/plans/partial-feature-inventory.md`, `docs/stab-feature-checklist.md`, and oracle metadata stay synchronized with the selected scope.
