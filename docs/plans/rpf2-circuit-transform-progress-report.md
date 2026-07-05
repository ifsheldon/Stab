# RPF2 Circuit Transform Progress Report

## Scope Closed In This Slice

This report records the RPF2 Rust circuit-transform slice implemented for `Circuit::flattened`, `Circuit::flattened_operations`, `Circuit::without_noise`, `Circuit::decomposed`, scoped unitary and selected single-instruction measurement-rich `Circuit::time_reversed_for_flows`, and scoped `Circuit::with_inlined_feedback`.

Implemented behavior:

- `Circuit::flattened` materializes a circuit with repeat blocks unrolled, `SHIFT_COORDS` absorbed, instruction tags preserved, repeat tags dropped, and coordinate shifts applied to `QUBIT_COORDS` and `DETECTOR` arguments in Stim v1.16.0 order.
- `Circuit::flattened_operations` returns owned unfused instructions for the same flattened traversal, matching the structural intent of Stim's deprecated `flattened_operations` API without claiming Python tuple ergonomics.
- Materialized flattening rejects more than one million output operations with a precise domain error, while shift-only large repeats are folded into a single coordinate shift instead of being iterated.
- `Circuit::without_noise` drops ordinary noise, strips probability arguments from measurement-producing gates, preserves deterministic operations, annotations, detector and observable declarations, tags, ticks, coordinate shifts, and measurement-record references, and replaces heralded noise with deterministic zero `MPAD` records so measurement-record indexing remains stable.
- `Circuit::decomposed` now implements the public Rust counterpart to Stim's `Circuit.decomposed` for the owned RPF2 slice, including fixed-shape H/S/CX/M/R template substitution, ISWAP decomposition, MPP measurement decomposition, SPP/SPP_DAG phase-product decomposition, pair-measurement decomposition, tag preservation, noise and annotation preservation, constant MPP products, and anti-Hermitian product rejection.
- `Circuit::time_reversed_for_flows` now implements the scoped unitary Rust subset by validating unsigned Pauli-only flows against the original unitary circuit with bounded tableau validation or folded sparse validation for supported large repeats, returning the current QEC inverse subset, and swapping flow input and output endpoints while preserving idle qubits beyond the circuit width.
- The selected measurement-rich `Circuit::time_reversed_for_flows` subset validates flows through the sparse tracker and reverses flow endpoints while preserving record and observable terms for one noiseless plain `M`, `MX`, `MY`, `MXX`, `MYY`, or `MZZ` instruction group, with pinned Stim `M` and `MZZ` examples plus source-owned basis coverage for `MX`, `MY`, `MXX`, and `MYY`.
- `Circuit::with_inlined_feedback` now exposes the existing feedback-removal helper as a public method for the supported top-level single-control Pauli and MPP feedback subset, with precise rejections for repeat blocks and unsupported classical controlled gates.

Remaining RPF2 work:

- Flow-dependent decomposition checks remain open where they require the RPF5 measurement-rich flow semantics decision.
- Full public feedback-inlining transform parity remains open beyond the scoped method, especially exact loop refolding and repeat-block feedback behavior.
- Exact loop refolding remains open.
- Broader measurement-rich `time_reversed_for_flows` rewrites for resets, detectors, feedback, noise, repeats, multi-instruction circuits, and larger QEC inverse behavior remain active follow-up work and stay logged in `docs/plans/milestone-spec-gaps.md`.
- QASM, Quirk, Crumble, diagrams, and Python-specific ergonomics remain explicitly deferred.

## Tests

Implemented source-owned tests:

- `cargo test -p stab-core --test circuit_transforms --quiet`
- `cargo test -p stab-core --test circuit_inverse_qec time_reversed_for_flows --quiet`

The test file ports and adapts pinned Stim v1.16.0 cases from `src/stim/circuit/circuit.test.cc`, `src/stim/circuit/circuit_pybind_test.py`, and tag-specific Python tests.
Coverage includes empty circuits, dropped `SHIFT_COORDS`, simple repeat unrolling, coordinate shifts through repeats, detector and observable preservation, instruction tags, repeat-tag removal, unfused flattened operations, materialized expansion rejection, folded shift-only repeats, noisy measurement probability stripping, ordinary noise removal, heralded-noise `MPAD` replacement, annotation preservation, coordinate-overflow rejection, public `decomposed` ISWAP and MPP output, decomposition tag preservation across RX, noise, MPP, detector, and SPP, constant MPP products, anti-Hermitian MPP/SPP rejection, scoped unitary `time_reversed_for_flows` empty-flow inverse behavior, upstream-shaped flow-past-end behavior, idle extra-qubit behavior, large-repeat folding, unsatisfied-flow rejection, selected measurement-rich `M`, `MX`, `MY`, `MXX`, `MYY`, and `MZZ` flow reversal, measurement-rich unsatisfied-flow rejection, noisy measurement-rich rejection, multi-instruction measurement-rich rejection, scoped feedback-inlining API exposure, MPP feedback DEM preservation, repeat-block rejection, and unsupported classical-control rejection.

## Oracle Rows

Implemented:

- `pf2-circuit-flatten-without-noise-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_transforms`.
- `pf2-circuit-decomposed-public-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_transforms decomposed`.
- `pf2-feedback-inline-scoped-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_transforms feedback`.
- `pf2-time-reverse-flow-unitary-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_inverse_qec unitary_subset`.
- `pf2-time-reverse-flow-measurement-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_inverse_qec measurement_rich_subset`.

Still manifest-only:

- `pf2-circuit-flatten-without-noise`: broad umbrella row retained as a planning row.
- `pf2-circuit-decomposed`: broad umbrella row retained as a planning row for any decomposition cases that depend on later flow semantics.
- `pf2-feedback-time-reverse`: full feedback transform and broader measurement-rich flow-time-reversal parity remain open.

## Benchmarks

Implemented non-primary report-only runners:

- `pf2-circuit-flatten-repeat`: measures Rust `Circuit::flattened` on a repeat-heavy coordinate-shift fixture and reports `stab_circuit_flatten_repeat_shifted_coords` with normalized `operations/s`.
- `pf2-circuit-without-noise`: measures Rust `Circuit::without_noise` on noisy, heralded, measurement, detector, and annotation instruction groups and reports `stab_circuit_without_noise_top_level` with normalized `source-instructions/s`.
- `pf2-circuit-decompose-mpp-spp`: measures Rust `Circuit::decomposed` on ISWAP, MPP, SPP, SPP_DAG, pair-measurement, noise, and detector operations and reports `stab_circuit_decompose_mpp_spp` with normalized `source-instructions/s`.
- `pf2-feedback-inline-batch`: measures Rust `Circuit::with_inlined_feedback` on the scoped MPP feedback fixture and reports `stab_circuit_with_inlined_feedback_mpp` with normalized `transforms/s`.
- `pf2-time-reverse-flow`: measures scoped unitary Rust `Circuit::time_reversed_for_flows` on an upstream-shaped unitary circuit with idle far-qubit flows and reports `stab_circuit_time_reversed_for_flows_unitary` with normalized `flows/s`.
- `pf2-time-reverse-flow-measurement`: measures selected measurement-rich Rust `Circuit::time_reversed_for_flows` on the pinned `MZZ` flow-through shape and reports `stab_circuit_time_reversed_for_flows_measurement` with normalized `flows/s`.

Comparability:

- These rows are `contract-only` and report-only because this harness has no faithful direct Rust baseline for pinned Stim's API timing.
- No RPF2 transform row is promoted into the 1.25x primary threshold gate.

Probe evidence:

- `just bench::compare --only pf2-circuit-flatten-repeat --baseline target/benchmarks/rpf2-flatten-probe/baseline.json --report target/benchmarks/rpf2-flatten-compare-probe` measured `stab_circuit_flatten_repeat_shifted_coords` at `0.000466460s`, about `2.635e7 operations/s`.
- `just bench::compare --only pf2-circuit-without-noise --baseline target/benchmarks/rpf2-without-noise-probe/baseline.json --report target/benchmarks/rpf2-without-noise-compare-probe` measured `stab_circuit_without_noise_top_level` at `0.000214474s`, about `4.774e7 source-instructions/s`.
- `just bench::compare --only pf2-circuit-decompose-mpp-spp --baseline target/benchmarks/rpf2-decompose-probe-baseline/baseline.json --report target/benchmarks/rpf2-decompose-compare-probe` measured `stab_circuit_decompose_mpp_spp` at `0.000060760s`, about `1.317e5 source-instructions/s`.
- `just bench::compare --only pf2-feedback-inline-batch --baseline target/benchmarks/rpf2-feedback-probe-baseline/baseline.json --report target/benchmarks/rpf2-feedback-compare-probe` measured `stab_circuit_with_inlined_feedback_mpp` at `0.000002352s`, about `4.252e5 transforms/s`.
- `just bench::compare --only pf2-time-reverse-flow --baseline target/benchmarks/rpf2-time-reverse-flow-probe/baseline.json --report target/benchmarks/rpf2-time-reverse-flow-compare` measured `stab_circuit_time_reversed_for_flows_unitary` at `0.000009764s`, about `4.097e5 flows/s`.
- `just bench::compare --only pf2-time-reverse-flow-measurement --baseline target/benchmarks/rpf2-time-reverse-flow-measurement-probe/baseline.json --report target/benchmarks/rpf2-time-reverse-flow-measurement-compare` measured `stab_circuit_time_reversed_for_flows_measurement` at `0.000003706s`, about `1.079e6 flows/s`.

## Verification So Far

Passed for this slice:

- `cargo fmt --all --check`
- `cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings`
- `cargo test -p stab-core --test circuit_transforms --quiet`
- `cargo test -p stab-core --test circuit_inverse_qec time_reversed_for_flows --quiet`
- `cargo test -p stab-core --test circuit_transforms decomposed --quiet`
- `cargo test -p stab-core --test circuit_transforms feedback --quiet`
- `cargo test -p stab-core circuit --quiet`
- `cargo test -p stab-bench pf2_transform --quiet`
- `cargo test -p stab-bench --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --milestone PF2 --structural`
- `just bench::smoke`
- `just bench::baseline --only pf2-time-reverse-flow-measurement --out target/benchmarks/rpf2-time-reverse-flow-measurement-probe`
- `just bench::compare --only pf2-time-reverse-flow-measurement --baseline target/benchmarks/rpf2-time-reverse-flow-measurement-probe/baseline.json --report target/benchmarks/rpf2-time-reverse-flow-measurement-compare`

## Audit And Review

Milestone-audit for the selected measurement-rich time-reversal slice found the promoted scope complete against the current PFM2 and PFM5 text: the Rust API remains additive, accepts only one noiseless plain measurement instruction group, verifies requested flows through the existing sparse tracker, keeps noisy, repeated, multi-instruction, detector, reset, feedback, and broader QEC inverse behavior fail-closed, and is represented by oracle row `pf2-time-reverse-flow-measurement-rust` plus report-only benchmark row `pf2-time-reverse-flow-measurement`.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment.
The Rust/API sidecar found no confirmed findings and identified the broader Stim v1.16.0 QEC inverse behavior as residual out-of-scope risk.
The docs and benchmark sidecar found no confirmed findings and confirmed that checklist rows remain `Partial`, broad PF2 and PF5 parent rows stay open, and the new benchmark row is report-only rather than primary-gated.
Local review found one evidence gap before sidecar closure: the selector accepted all six measurement bases while tests initially exercised only `M` and `MZZ`; `time_reversed_for_flows_measurement_rich_subset_covers_selected_bases` now covers `MX`, `MY`, `MXX`, and `MYY`.

Still required before claiming the RPF2 milestone complete:

- `cargo fmt --all --check`
- `cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings`
- `cargo test -p stab-core circuit --quiet`
- `cargo test -p stab-bench --quiet`
- `just oracle::run --milestone PF2`
- `just bench::smoke`
- Milestone-audit and full-code-review for the whole RPF2 milestone after the remaining transform subfeatures are closed or explicitly logged as spec gaps.
