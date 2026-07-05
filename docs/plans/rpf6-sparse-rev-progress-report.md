# RPF6 Sparse Reverse Tracker Progress Report

## Summary

This report records the promoted PF6 sparse reverse tracker loop-folding slice.
It implements supported-Clifford unitary-repeat folding inside the sparse reverse detector-frame tracker and adds source-owned tests plus a report-only benchmark runner without claiming full sparse tracker parity.

## Implemented Surfaces

- `SparseReverseFrameTracker::undo_circuit` now recognizes repeat bodies containing the full single-qubit Clifford gate set plus `CX`, `CY`, and `CZ`.
- Quantum `CY` reverse propagation now uses the same sparse-tracker sensitivity engine as `CX` and `CZ`, so detecting-region extraction and supported-Clifford unitary-repeat folding can use it without a gate-specific fallback.
- For those repeat bodies, the tracker builds a linear slot transform for X and Z sensitivity slots, exponentiates it by the repeat count, and applies the powered transform to the current detector and observable sensitivity sets.
- Non-unitary repeat bodies and unsupported unitary gates continue to use the existing traversal path or fail through the existing gate-specific errors, so this slice does not broaden unsupported semantics.
- `check_if_circuit_has_unsigned_stabilizer_flows` now skips the tableau shortcut when any requested flow depends on measurements or observables, which routes measurement-dependent flow checks directly through the sparse tracker and avoids unrolling huge measured circuits before the tracker can fold their unitary repeats.

## Tests

Implemented Rust tests:

- `unitary_repeat_folding_matches_naive_mixed_clifford_loop`
- `unitary_repeat_folding_matches_naive_all_single_qubit_cliffords`
- `unitary_repeat_folding_handles_huge_periodic_loop`
- `unitary_repeat_folding_declines_non_unitary_and_unsupported_gates`
- `sparse_rev_frame_tracker_undo_tableau_cy_subset`
- `check_if_circuit_has_unsigned_stabilizer_flows_folds_unitary_repeats`

The sparse tracker tests live in `crates/stab-core/src/sparse_rev_frame_tracker.rs` and `crates/stab-core/src/sparse_rev_frame_tracker/unitary_repeat.rs`.
The public consumption test lives in `crates/stab-core/tests/circuit_flows.rs` and proves measurement-dependent unsigned-flow checking reaches the folded sparse-tracker path.

## Oracle Rows

Implemented row:

- `pf6-sparse-rev-unitary-repeat-rust`

The broad row `pf6-sparse-rev-tracker` remains manifest-only because full sparse reverse tracker parity still includes broader all-unitary fuzzing, analyzer/search consumption where needed, active matched-error hardening, and provenance-adjacent behavior not promoted here.

## Benchmark Rows

Row with new report-only runner coverage:

- `pf6-sparse-rev-frame-loop`, measured as `stab_pf6_sparse_rev_unitary_repeat_flow`.

The row measures public unsigned-flow checking over a measurement-dependent supported-Clifford unitary repeat, so the sparse reverse frame tracker must fold the loop.
It remains `non-primary-report-only` and `contract-only` because this internal Rust behavior has no faithful pinned Stim CLI timing ratio and should not enter the 1.25x primary threshold file.

## Remaining PF6 Sparse Tracker Work

- Broader all-unitary fuzzing beyond the promoted fixed two-qubit direct propagation and the single-qubit Clifford plus `CX`/`CY`/`CZ` loop-folding subset.
- Analyzer and search consumption cases that specifically require sparse tracker behavior beyond unsigned-flow checking.
- Active matched-error value-object hardening if future analyzer or search outputs require it.
- Full ErrorMatcher provenance, heralded matching, repeat-contained noise stack frames, and `stim explain_errors` CLI remain deferred.
