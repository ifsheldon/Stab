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
- `SPP` and `SPP_DAG` now propagate unsigned Pauli products directly in the sparse tracker when reached from unsigned flow checking. Product signs are intentionally ignored by this tracker, while anti-Hermitian products return an error that the public unsigned checker treats as `false`.
- `check_if_circuit_has_unsigned_stabilizer_flows` now skips the tableau shortcut when any requested flow depends on measurements or observables, which routes measurement-dependent flow checks directly through the sparse tracker and avoids unrolling huge measured circuits before the tracker can fold their unitary repeats.
- Matched-error value-object canonicalization is hardened for the PF6-adjacent explanation surface by sorting DEM terms, circuit locations, flipped Pauli products, and flipped measured observables, while preserving upstream-like matcher return ordering.

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
- `pf6_sparse_rev_spp_matches_decomposed_tableau_unsigned`
- `pf6_sparse_rev_spp_handles_multiple_groups_and_inverted_products`
- `pf6_sparse_rev_spp_rejects_anti_hermitian_products`
- `pf6_sparse_rev_spp_circuit_has_unsigned_stabilizer_flow_helpers_support_unsigned_semantics`
- `matched_error_canonicalize_sorts_terms_like_upstream`

The sparse tracker tests live in `crates/stab-core/src/sparse_rev_frame_tracker/tests.rs` and `crates/stab-core/src/sparse_rev_frame_tracker/unitary_repeat.rs`.
The public consumption tests live in `crates/stab-core/tests/circuit_flows.rs` and prove measurement-dependent unsigned-flow checking reaches the folded sparse-tracker path and that unsigned `SPP` or `SPP_DAG` flow checking follows decomposed-tableau Pauli-basis behavior instead of accepting false identity flows.

## Oracle Rows

Implemented row:

- `pf6-sparse-rev-unitary-repeat-rust`
- `pf6-sparse-rev-spp-rust`
- `pf6-matched-error-canonicalize-rust`

The broad row `pf6-sparse-rev-tracker` remains manifest-only because full sparse reverse tracker parity still includes analyzer/search consumption where needed, broader variable-target unitary semantics outside the promoted unsigned tracker path, future matched-error hardening only when newly promoted analyzer or search outputs require it, and provenance-adjacent behavior not promoted here.

## Benchmark Rows

Row with new report-only runner coverage:

- `pf6-sparse-rev-frame-loop`, measured as `stab_pf6_sparse_rev_unitary_repeat_flow`.

The row measures public unsigned-flow checking over a measurement-dependent fixed two-qubit `SWAP` repeat, so the sparse reverse frame tracker must fold the loop.
It remains `non-primary-report-only` and `contract-only` because this internal Rust behavior has no faithful pinned Stim CLI timing ratio and should not enter the 1.25x primary threshold file.
No separate benchmark row is added for unsigned `SPP`/`SPP_DAG` propagation because this slice is a correctness promotion inside the existing sparse-tracker and public unsigned-flow checker path, not a new production-scale throughput claim.
No separate benchmark row is added for matched-error canonicalization because this is a value-object ordering contract and `ErrorMatcher` avoids implicit canonicalization on returned locations.

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

Completed checks for the unsigned `SPP`/`SPP_DAG` propagation refresh:

```sh
cargo test -p stab-core pf6_sparse_rev_spp --quiet
cargo test -p stab-core matched_error_canonicalize_sorts_terms_like_upstream --quiet
just oracle::run --milestone PF6 --structural
```

## Remaining PF6 Sparse Tracker Work

- Analyzer and search consumption cases that specifically require sparse tracker behavior beyond unsigned-flow checking.
- Broader variable-target unitary semantics outside unsigned sparse-tracker propagation and the already promoted sampler, detection-conversion, detector-frame, and analyzer SPP subsets, including repeat-folding execution of `SPP` and `SPP_DAG` if later milestones promote that surface.
- Future matched-error value-object hardening beyond the selected canonicalization slice if newly promoted analyzer or search outputs require it.
- Full ErrorMatcher provenance, heralded matching, repeat-contained noise stack frames, and `stim explain_errors` CLI remain deferred.
