# PFM5 Missing-Detectors Nested Final-Repeat Scope

## Summary

This PFM5 slice extends the selected folded final-repeat proof in Rust `missing_detectors` to bounded nested repeat bodies.
It remains a narrow resource-boundary promotion and does not claim general folded missing-detector output, generated-code suffix closure, Python API parity, or arbitrary repeat-contained flow solving.

## Owned Surface

- Public Rust API: `stab_core::missing_detectors`.
- Checklist row: detector-analysis utility APIs.
- Active plan row: PFM5 `missing_detectors` folded large-repeat traversal.
- Comparator class: structural Rust parity and resource-boundary evidence.
- Oracle row: add `pf5-missing-detectors-nested-final-repeat-rust` as a focused structural row, while leaving `pf5-missing-detectors-repeat-rust` as the broader repeat traversal rollup.
- Benchmark row: no new benchmark row because this slice only expands the eligibility proof for an empty-suffix resource-boundary fast path and does not introduce a representative throughput workload separate from `pf5-missing-detectors-generated-code` or `pf5-missing-detectors-mpp`.

## Selected Positive Scope

Stab should accept a final top-level `REPEAT` whose expanded count exceeds the current materialized repeat budget when the final repeat body contains bounded nested repeat blocks and all of the following are true:

- Every measurement-record target inside each nested repeat body refers only to measurements produced inside that nested body iteration.
- Instructions after a bounded nested repeat may refer to measurement records produced by that nested repeat within the same outer body iteration.
- The final repeat body contains no `OBSERVABLE_INCLUDE` instruction.
- The bounded prefix has no missing-detector suffix before the final repeat.
- Processing one full bounded body of the final repeat produces no missing-detector suffix.
- Processing one full bounded body leaves the invariant tracker unchanged from the state immediately before the final repeat.
- The final repeat is the last top-level item, so no later instruction can depend on skipped measurement-record history.

The selected positive examples are deterministic final-repeat loops where nested bounded repeats produce measurements that are covered by detector rows either inside the nested body or later in the same outer body.

## Explicit Rejections And Non-Goals

- Nested repeat bodies that reference measurement records from before the nested body remain capped because one nested folded body is not enough evidence for cross-iteration or prefix-dependent row behavior.
- Nested large repeats inside the proof body remain capped by the existing repeat budget.
- Repeats whose body changes tracker state remain capped unless a later slice proves a separate fold rule.
- Repeats whose body contains `OBSERVABLE_INCLUDE` remain capped for this nested-final-repeat slice because observable rows merge by observable id across iterations; the later [pfm5-missing-detectors-observable-neutral-final-repeat-scope.md](pfm5-missing-detectors-observable-neutral-final-repeat-scope.md) selects only top-level record-only observable rows that are redundant under independent detector evidence.
- Repeats whose prefix or body cannot be processed by the current missing-detector analyzer remain capped by the original repeat budget instead of returning proof-run errors.
- Generated honeycomb, toric, and broader generated-code suffix analysis are not expanded by this slice.
- Python binding behavior and exact C++ implementation internals remain out of scope.

## Tests

- Add `pf5_missing_detectors_nested_final_repeat_folds_local_bodies` proving a final large deterministic repeat with bounded nested local detector rows returns an empty suffix instead of failing the expanded-repeat budget.
- Add `pf5_missing_detectors_nested_final_repeat_folds_local_bodies` coverage proving detector rows after a bounded nested repeat may refer to records produced by that nested repeat within the same outer body.
- Add `pf5_missing_detectors_nested_final_repeat_keeps_unselected_bodies_capped` proving nested cross-iteration record references, nested large repeats, and public-API over-depth nested repeats remain capped.
- Keep the existing fallback tests for observable-dependent rows, nested observable rows, unsupported local bodies, tracker-changing bodies, and excessive repeat expansion.

## Done Criteria

- The code recursively validates folded-body record locality without materializing oversized repeat counts.
- The fold remains additive and falls back to the existing capped path when any proof condition fails.
- Tests cover success, fallback rejection, and unchanged legacy cap behavior.
- `docs/plans/rpf5-missing-detectors-progress-report.md`, `docs/plans/non-deferred-partial-feature-milestones.md`, `docs/plans/partial-feature-inventory.md`, `docs/stab-feature-checklist.md`, roadmap text, and oracle metadata stay synchronized with the selected scope.
