# RPF6 Sparse Reverse Tracker Progress Report

## Summary

This report records the promoted PF6 sparse reverse tracker loop-folding slices.
It implements supported-Clifford unitary-repeat folding inside the sparse reverse detector-frame tracker and adds source-owned generated repeat tests plus a report-only benchmark runner without claiming full sparse tracker parity.

## Implemented Surfaces

- `SparseReverseFrameTracker::undo_circuit` now recognizes repeat bodies containing the full single-qubit Clifford gate set with plain qubit targets plus fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets.
- Quantum `CY` reverse propagation now uses the same sparse-tracker sensitivity engine as `CX` and `CZ`, so detecting-region extraction and supported-Clifford unitary-repeat folding can use it without a gate-specific fallback.
- For those repeat bodies, the tracker builds a linear slot transform for X and Z sensitivity slots, exponentiates it by the repeat count, and applies the powered transform to the current detector and observable sensitivity sets.
- Deterministic generated tests cover supported fixed-shape unitary repeat bodies across every fixed two-qubit tableau-backed gate, nested repeats, multi-target single-qubit instructions, and multi-pair two-qubit instructions by comparing the folded path to a test-only traversal that deliberately bypasses repeat folding.
- Non-unitary repeat bodies, unsupported unitary gates, and non-plain classical or sweep-controlled target shapes continue to use the existing traversal path or fail through the existing gate-specific errors, so this slice does not broaden unsupported semantics.
- Unsupported sparse-tracker instruction families such as `SPP` now fail closed instead of being silently treated as identity when the tracker is reached from unsigned flow checking; executing those variable-target unitary semantics remains future work.
- `check_if_circuit_has_unsigned_stabilizer_flows` now skips the tableau shortcut when any requested flow depends on measurements or observables, which routes measurement-dependent flow checks directly through the sparse tracker and avoids unrolling huge measured circuits before the tracker can fold their unitary repeats.

## Tests

Implemented Rust tests:

- `unitary_repeat_folding_matches_naive_mixed_clifford_loop`
- `unitary_repeat_folding_matches_naive_all_single_qubit_cliffords`
- `unitary_repeat_folding_matches_naive_fixed_two_qubit_cliffords`
- `unitary_repeat_folding_matches_naive_generated_supported_unitary_loops`
- `unitary_repeat_folding_matches_naive_nested_supported_unitary_loops`
- `unitary_repeat_folding_handles_huge_periodic_loop`
- `unitary_repeat_folding_declines_non_unitary_and_unsupported_gates`
- `sparse_rev_frame_tracker_undo_tableau_cy_subset`
- `sparse_rev_frame_tracker_undo_fixed_two_qubit_gates_match_tableau`
- `check_if_circuit_has_unsigned_stabilizer_flows_folds_unitary_repeats`
- `circuit_has_unsigned_stabilizer_flow_helpers_fail_closed_on_unsupported_unitary_gates`

The sparse tracker tests live in `crates/stab-core/src/sparse_rev_frame_tracker/tests.rs` and `crates/stab-core/src/sparse_rev_frame_tracker/unitary_repeat.rs`.
The public consumption tests live in `crates/stab-core/tests/circuit_flows.rs` and prove measurement-dependent unsigned-flow checking reaches the folded sparse-tracker path and unsupported `SPP` flow checking fails closed instead of accepting identity flows.

## Oracle Rows

Implemented row:

- `pf6-sparse-rev-unitary-repeat-rust`

The broad row `pf6-sparse-rev-tracker` remains manifest-only because full sparse reverse tracker parity still includes analyzer/search consumption where needed, unsupported variable-target unitary semantics, active matched-error hardening, and provenance-adjacent behavior not promoted here.

## Benchmark Rows

Row with new report-only runner coverage:

- `pf6-sparse-rev-frame-loop`, measured as `stab_pf6_sparse_rev_unitary_repeat_flow`.

The row measures public unsigned-flow checking over a measurement-dependent fixed two-qubit `SWAP` repeat, so the sparse reverse frame tracker must fold the loop.
It remains `non-primary-report-only` and `contract-only` because this internal Rust behavior has no faithful pinned Stim CLI timing ratio and should not enter the 1.25x primary threshold file.

## Verification Evidence

Completed checks for the fixed-two-qubit repeat-folding refresh:

```sh
cargo test -p stab-core unitary_repeat --quiet
cargo test -p stab-core sparse_rev_frame_tracker_undo --quiet
cargo test -p stab-core --test circuit_flows check_if_circuit_has_unsigned_stabilizer_flows_folds_unitary_repeats --quiet
just bench::baseline --only pf6-sparse-rev-frame-loop --out target/benchmarks/pf6-sparse-rev-fixed-two-qubit-probe
just bench::compare --only pf6-sparse-rev-frame-loop --baseline target/benchmarks/pf6-sparse-rev-fixed-two-qubit-probe/baseline.json --report target/benchmarks/pf6-sparse-rev-fixed-two-qubit-compare
```

The fixed-two-qubit benchmark probe reported `stab_pf6_sparse_rev_unitary_repeat_flow=0.000010432s` and `9.586e10 folded-rounds/s`, with output written to `target/benchmarks/pf6-sparse-rev-fixed-two-qubit-compare`.
The generated repeat-folding refresh was rechecked with `cargo test -p stab-core unitary_repeat --quiet`.

## Remaining PF6 Sparse Tracker Work

- Analyzer and search consumption cases that specifically require sparse tracker behavior beyond unsigned-flow checking.
- Unsupported variable-target unitary semantics such as `SPP` repeat bodies if a later gate-execution or analyzer milestone promotes them beyond explicit fallback or rejection behavior.
- Active matched-error value-object hardening if future analyzer or search outputs require it.
- Full ErrorMatcher provenance, heralded matching, repeat-contained noise stack frames, and `stim explain_errors` CLI remain deferred.
