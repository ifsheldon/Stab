# RPF2 Circuit Transform Progress Report

## Scope Closed In This Slice

This report records the RPF2 Rust circuit-transform slice implemented for `Circuit::flattened`, `Circuit::flattened_operations`, `Circuit::without_noise`, `Circuit::decomposed`, and scoped `Circuit::with_inlined_feedback`.

Implemented behavior:

- `Circuit::flattened` materializes a circuit with repeat blocks unrolled, `SHIFT_COORDS` absorbed, instruction tags preserved, repeat tags dropped, and coordinate shifts applied to `QUBIT_COORDS` and `DETECTOR` arguments in Stim v1.16.0 order.
- `Circuit::flattened_operations` returns owned unfused instructions for the same flattened traversal, matching the structural intent of Stim's deprecated `flattened_operations` API without claiming Python tuple ergonomics.
- Materialized flattening rejects more than one million output operations with a precise domain error, while shift-only large repeats are folded into a single coordinate shift instead of being iterated.
- `Circuit::without_noise` drops ordinary noise, strips probability arguments from measurement-producing gates, preserves deterministic operations, annotations, detector and observable declarations, tags, ticks, coordinate shifts, and measurement-record references, and replaces heralded noise with deterministic zero `MPAD` records so measurement-record indexing remains stable.
- `Circuit::decomposed` now implements the public Rust counterpart to Stim's `Circuit.decomposed` for the owned RPF2 slice, including fixed-shape H/S/CX/M/R template substitution, ISWAP decomposition, MPP measurement decomposition, SPP/SPP_DAG phase-product decomposition, pair-measurement decomposition, tag preservation, noise and annotation preservation, constant MPP products, and anti-Hermitian product rejection.
- `Circuit::with_inlined_feedback` now exposes the existing feedback-removal helper as a public method for the supported top-level single-control Pauli and MPP feedback subset, with precise rejections for repeat blocks and unsupported classical controlled gates.

Remaining RPF2 work:

- Flow-dependent decomposition checks remain open where they require the RPF5 measurement-rich flow semantics decision.
- Full public feedback-inlining transform parity remains open beyond the scoped method, especially exact loop refolding and repeat-block feedback behavior.
- Exact loop refolding remains open.
- `time_reversed_for_flows` remains blocked on the RPF5 measurement-rich flow semantics decision and is logged in `docs/plans/milestone-spec-gaps.md`.
- QASM, Quirk, Crumble, diagrams, and Python-specific ergonomics remain explicitly deferred.

## Tests

Implemented source-owned tests:

- `cargo test -p stab-core --test circuit_transforms --quiet`

The test file ports and adapts pinned Stim v1.16.0 cases from `src/stim/circuit/circuit.test.cc`, `src/stim/circuit/circuit_pybind_test.py`, and tag-specific Python tests.
Coverage includes empty circuits, dropped `SHIFT_COORDS`, simple repeat unrolling, coordinate shifts through repeats, detector and observable preservation, instruction tags, repeat-tag removal, unfused flattened operations, materialized expansion rejection, folded shift-only repeats, noisy measurement probability stripping, ordinary noise removal, heralded-noise `MPAD` replacement, annotation preservation, coordinate-overflow rejection, public `decomposed` ISWAP and MPP output, decomposition tag preservation across RX, noise, MPP, detector, and SPP, constant MPP products, anti-Hermitian MPP/SPP rejection, scoped feedback-inlining API exposure, MPP feedback DEM preservation, repeat-block rejection, and unsupported classical-control rejection.

## Oracle Rows

Implemented:

- `pf2-circuit-flatten-without-noise-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_transforms`.
- `pf2-circuit-decomposed-public-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_transforms decomposed`.
- `pf2-feedback-inline-scoped-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_transforms feedback`.

Still manifest-only:

- `pf2-circuit-flatten-without-noise`: broad umbrella row retained as a planning row.
- `pf2-circuit-decomposed`: broad umbrella row retained as a planning row for any decomposition cases that depend on later flow semantics.
- `pf2-feedback-time-reverse`: full feedback transform and flow-time-reversal parity remain open.

## Benchmarks

Implemented non-primary report-only runners:

- `pf2-circuit-flatten-repeat`: measures Rust `Circuit::flattened` on a repeat-heavy coordinate-shift fixture and reports `stab_circuit_flatten_repeat_shifted_coords` with normalized `operations/s`.
- `pf2-circuit-without-noise`: measures Rust `Circuit::without_noise` on noisy, heralded, measurement, detector, and annotation instruction groups and reports `stab_circuit_without_noise_top_level` with normalized `source-instructions/s`.
- `pf2-circuit-decompose-mpp-spp`: measures Rust `Circuit::decomposed` on ISWAP, MPP, SPP, SPP_DAG, pair-measurement, noise, and detector operations and reports `stab_circuit_decompose_mpp_spp` with normalized `source-instructions/s`.
- `pf2-feedback-inline-batch`: measures Rust `Circuit::with_inlined_feedback` on the scoped MPP feedback fixture and reports `stab_circuit_with_inlined_feedback_mpp` with normalized `transforms/s`.

Comparability:

- These rows are `contract-only` and report-only because this harness has no faithful direct Rust baseline for pinned Stim's API timing.
- No RPF2 transform row is promoted into the 1.25x primary threshold gate.

Probe evidence:

- `just bench::compare --only pf2-circuit-flatten-repeat --baseline target/benchmarks/rpf2-flatten-probe/baseline.json --report target/benchmarks/rpf2-flatten-compare-probe` measured `stab_circuit_flatten_repeat_shifted_coords` at `0.000466460s`, about `2.635e7 operations/s`.
- `just bench::compare --only pf2-circuit-without-noise --baseline target/benchmarks/rpf2-without-noise-probe/baseline.json --report target/benchmarks/rpf2-without-noise-compare-probe` measured `stab_circuit_without_noise_top_level` at `0.000214474s`, about `4.774e7 source-instructions/s`.
- `just bench::compare --only pf2-circuit-decompose-mpp-spp --baseline target/benchmarks/rpf2-decompose-probe-baseline/baseline.json --report target/benchmarks/rpf2-decompose-compare-probe` measured `stab_circuit_decompose_mpp_spp` at `0.000060760s`, about `1.317e5 source-instructions/s`.
- `just bench::compare --only pf2-feedback-inline-batch --baseline target/benchmarks/rpf2-feedback-probe-baseline/baseline.json --report target/benchmarks/rpf2-feedback-compare-probe` measured `stab_circuit_with_inlined_feedback_mpp` at `0.000002352s`, about `4.252e5 transforms/s`.

## Verification So Far

Passed for this slice:

- `cargo fmt --all --check`
- `cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings`
- `cargo test -p stab-core --test circuit_transforms --quiet`
- `cargo test -p stab-core --test circuit_transforms decomposed --quiet`
- `cargo test -p stab-core --test circuit_transforms feedback --quiet`
- `cargo test -p stab-core circuit --quiet`
- `cargo test -p stab-bench pf2_transform --quiet`
- `cargo test -p stab-bench --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --milestone PF2`
- `just bench::smoke`

Still required before claiming the RPF2 milestone complete:

- `cargo fmt --all --check`
- `cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings`
- `cargo test -p stab-core circuit --quiet`
- `cargo test -p stab-bench --quiet`
- `just oracle::run --milestone PF2`
- `just bench::smoke`
- Milestone-audit and full-code-review for the whole RPF2 milestone after the remaining transform subfeatures are closed or explicitly logged as spec gaps.
