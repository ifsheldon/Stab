# RPF2 Circuit Transform Progress Report

## Scope Closed In This Slice

This report records the RPF2 Rust circuit-transform slice implemented for `Circuit::flattened`, `Circuit::flattened_operations`, `Circuit::without_noise`, `Circuit::decomposed`, scoped `Circuit::inverse_qec`, scoped unitary and selected single-instruction measurement-rich `Circuit::time_reversed_for_flows`, and scoped `Circuit::with_inlined_feedback`.

Implemented behavior:

- `Circuit::flattened` materializes a circuit with repeat blocks unrolled, `SHIFT_COORDS` absorbed, instruction tags preserved, repeat tags dropped, and coordinate shifts applied to `QUBIT_COORDS` and `DETECTOR` arguments in Stim v1.16.0 order.
- `Circuit::flattened_operations` returns owned unfused instructions for the same flattened traversal, matching the structural intent of Stim's deprecated `flattened_operations` API without claiming Python tuple ergonomics.
- Materialized flattening rejects more than one million output operations with a precise domain error, while shift-only large repeats are folded into a single coordinate shift instead of being iterated.
- `Circuit::without_noise` drops ordinary noise, strips probability arguments from measurement-producing gates, preserves deterministic operations, annotations, detector and observable declarations, tags, ticks, coordinate shifts, and measurement-record references, and replaces heralded noise with deterministic zero `MPAD` records so measurement-record indexing remains stable.
- `Circuit::decomposed` now implements the public Rust counterpart to Stim's `Circuit.decomposed` for the owned RPF2 slice, including fixed-shape H/S/CX/M/R template substitution, ISWAP decomposition, MPP measurement decomposition, SPP/SPP_DAG phase-product decomposition, pair-measurement decomposition, tag preservation, noise and annotation preservation, constant MPP products, anti-Hermitian product rejection, and selected MPP or pair-measurement flow-generator preservation.
- `Circuit::inverse_qec` now implements the selected no-flow reset-measure-detector target-list packet, exact selected two-to-one detector-flow packet, exact selected noiseless MPP all-record identity-parity detector-flow packet, and selected measure-reset pass-through target-list packet for one detector, including the pinned Stim v1.16.0 `r_m_det`, `two_to_one`, `mpp`, and `pass_through` cases, multi-target record remapping where selected, sparse detector subsets for the no-flow packet, duplicate detector-record parity where selected, empty-detector behavior where selected, and tag or coordinate preservation where applicable, while broader detector-flow rewrites with interleaved operations beyond the exact two-to-one and selected MPP identity-parity packets, prior-measurement detector refs, noisy measurement/reset, feedback, observables, repeats, and multi-instruction QEC inverse behavior remain active.
- `Circuit::time_reversed_for_flows` now implements the scoped unitary Rust subset by validating unsigned Pauli-only flows against the original unitary circuit with bounded tableau validation or folded sparse validation for supported large repeats, returning the current QEC inverse subset, and swapping flow input and output endpoints while preserving idle qubits beyond the circuit width.
- The selected measurement-rich `Circuit::time_reversed_for_flows` subset validates flows through the sparse tracker and reverses flow endpoints for one noiseless plain unique-target `M`, `MX`, `MY`, `MXX`, `MYY`, or `MZZ` instruction group, with pinned Stim `M` and `MZZ` examples, source-owned basis coverage for `MX`, `MY`, `MXX`, and `MYY`, selected multi-record measurement-ordering evidence, selected plain unique-target `R`, `RX`, and `RY` reset-to-measurement conversion, and selected single-target `M`, `MX`, and `MY` measurement-to-reset conversion when the full flow batch has record dependence but no future Pauli dependence on the measured qubit.
- The selected measure-reset subset additionally supports one noiseless unique-target `MR`, `MRX`, or `MRY` instruction, including inverted result targets, mapping reset-effect output terms into the reversed measurement records and mapping measurement-record dependencies back into reset effects.
- `Circuit::with_inlined_feedback` now exposes the existing feedback-removal helper as a public method for the supported top-level single-control Pauli and MPP feedback subset, with selected `XCZ`/`YCZ` measurement-record feedback equivalent to `CX`/`CY`, selected bounded repeat-loop refolding, selected nested bounded-repeat `CY`/`CZ` detector-parity preservation, and precise rejections for excessive repeat work and unsupported classical controlled gates.

Remaining RPF2 work:

- Broader flow-dependent decomposition checks remain open when new RPF5 measurement-rich flow families are promoted beyond the selected MPP and pair-measurement flow-generator preservation cases.
- Full public feedback-inlining transform parity remains open beyond the scoped method, especially broader repeat-contained feedback behavior beyond the selected pinned loop-refolding and nested bounded-repeat detector-parity cases.
- Broader measurement-rich `time_reversed_for_flows` and `inverse_qec` rewrites for parser-rejected inverted reset targets, detector-flow rewrites beyond the selected no-flow, exact two-to-one, selected MPP identity-parity, and pass-through packets, feedback, noise, repeats, multi-instruction circuits, and larger QEC inverse behavior remain active follow-up work and stay logged in `docs/plans/milestone-spec-gaps.md`. Duplicate reset-only and duplicate measure-reset targets remain explicitly fail-closed until `docs/plans/milestone-spec-gaps.md` resolves whether Stab should clone Stim v1.16.0's malformed duplicate-target inverse flows, return corrected semantic flows, or keep rejecting them.
- QASM, Quirk, Crumble, diagrams, and Python-specific ergonomics remain explicitly deferred.

## Tests

Implemented source-owned tests:

- `cargo test -p stab-core --test circuit_transforms --quiet`
- `cargo test -p stab-core --test circuit_inverse_qec inverse_qec --quiet`
- `cargo test -p stab-core --test circuit_inverse_qec_mpp --quiet`
- `cargo test -p stab-core --test circuit_inverse_qec time_reversed_for_flows --quiet`

The test files port and adapt pinned Stim v1.16.0 cases from `src/stim/circuit/circuit.test.cc`, `src/stim/circuit/circuit_pybind_test.py`, `src/stim/util_top/circuit_inverse_qec.test.cc`, and tag-specific Python tests.
Coverage includes empty circuits, dropped `SHIFT_COORDS`, simple repeat unrolling, coordinate shifts through repeats, detector and observable preservation, instruction tags, repeat-tag removal, unfused flattened operations, materialized expansion rejection, folded shift-only repeats, noisy measurement probability stripping, ordinary noise removal, heralded-noise `MPAD` replacement, annotation preservation, coordinate-overflow rejection, public `decomposed` ISWAP and MPP output, decomposition tag preservation across RX, noise, MPP, detector, and SPP, constant MPP products, anti-Hermitian MPP/SPP rejection, selected MPP and pair-measurement decomposition flow-generator preservation, scoped `inverse_qec` unitary behavior and selected no-flow reset-measure-detector, exact two-to-one, exact noiseless MPP all-record identity-parity detector-flow, and measure-reset pass-through packet coverage including pinned `r_m_det`, pinned `two_to_one`, pinned `mpp`, pinned `pass_through`, selected same-basis and detector metadata variants, multi-target record remapping where selected, sparse detector subsets for the no-flow packet, duplicate detector-record parity where selected, empty-detector behavior where selected, tag and coordinate preservation where applicable, and fail-closed nearby shapes including prior-measurement detector records, unpromoted sparse two-to-one detectors, duplicate two-to-one detector records, noisy measurement or measure-reset, noisy MPP, sparse or duplicate MPP detector records, non-identity MPP detector parity, anti-Hermitian MPP products, multiple MPP detectors, nonmatching basis, inverted measurement or measure-reset targets, duplicate reset, measurement, or measure-reset targets, nonmatching target lists, reversed CX, larger target lists, and out-of-range record rejection, scoped unitary `time_reversed_for_flows` empty-flow inverse behavior, upstream-shaped flow-past-end behavior, idle extra-qubit behavior, large-repeat folding, unsatisfied-flow rejection, selected measurement-rich `M`, `MX`, `MY`, `MXX`, `MYY`, and `MZZ` flow reversal including multi-record ordering, selected plain unique-target `R`, `RX`, and `RY` reset-to-measurement conversion, selected single-target `M`, `MX`, and `MY` measurement-to-reset conversion, selected unique-target `MR`, `MRX`, and `MRY` measure-reset flow reversal including inverted result targets, measurement-rich unsatisfied-flow rejection, noisy measurement-rich rejection, multi-instruction measurement-rich rejection, duplicate measurement target rejection, duplicate reset target rejection under the logged spec-gap, duplicate measure-reset rejection under the logged spec-gap, unscoped reset observable-term and measurement-record-term rejection, scoped feedback-inlining API exposure, MPP feedback DEM preservation, selected `XCZ`/`YCZ` measurement-record feedback equivalence, selected bounded repeat-loop refolding, selected nested bounded-repeat `CY`/`CZ` feedback detector-parity preservation, excessive repeat-work rejection, and unsupported classical-control rejection.

## Oracle Rows

Implemented:

- `pf2-circuit-flatten-without-noise-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_transforms`.
- `pf2-circuit-decomposed-public-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_transforms decomposed`.
- `pf2-feedback-inline-scoped-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_transforms feedback`.
- `pf2-inverse-qec-reset-measure-detector-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_inverse_qec reset_measure_detector`.
- `pf2-inverse-qec-two-to-one-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_inverse_qec two_to_one`.
- `pf2-inverse-qec-measure-reset-pass-through-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_inverse_qec pass_through`.
- `pf2-inverse-qec-mpp-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_inverse_qec_mpp`.
- `pf2-inverse-qec-unpromoted-measurement-rewrites-rust`: structural `cargo-test` row for `cargo test -p stab-core --test circuit_inverse_qec unpromoted_measurement_rewrites`.
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
- `pf2-circuit-decompose-mpp-spp`: measures Rust `Circuit::decomposed` on ISWAP, MPP, SPP, SPP_DAG, pair-measurement, noise, and detector operations and reports `stab_circuit_decompose_mpp_spp` with normalized `source-instructions/s`. The selected flow-generator preservation evidence uses the same decomposition hot path, so no separate benchmark row is required for this test-only semantic check.
- `pf2-feedback-inline-batch`: measures Rust `Circuit::with_inlined_feedback` on the scoped MPP feedback fixture, selected bounded repeat-loop fixture, and selected `XCZ`/`YCZ` feedback fixture and reports `stab_circuit_with_inlined_feedback_mpp` with normalized `transforms/s`, `stab_circuit_with_inlined_feedback_repeat_loop` with normalized `repeat-iterations/s`, and `stab_circuit_with_inlined_feedback_xcz_ycz` with normalized `transforms/s`. The selected nested bounded-repeat `CY`/`CZ` feedback evidence is structural test coverage only because it uses the same bounded repeat traversal path and does not introduce a separate hot path.
- `pf2-time-reverse-flow`: measures scoped unitary Rust `Circuit::time_reversed_for_flows` on an upstream-shaped unitary circuit with idle far-qubit flows and reports `stab_circuit_time_reversed_for_flows_unitary` with normalized `flows/s`.
- `pf2-time-reverse-flow-measurement`: measures selected measurement-rich Rust `Circuit::time_reversed_for_flows` on pinned `M` and `MZZ` flow-through shapes with multi-record ordering, selected plain unique-target `R`, `RX`, and `RY` reset-to-measurement, selected single-target `M`, `MX`, and `MY` measurement-to-reset, and unique-target `MR`, `MRX`, and `MRY` measure-reset shapes including inverted result targets and reports `stab_circuit_time_reversed_for_flows_measurement` with normalized `flows/s`.

Comparability:

- These rows are `contract-only` and report-only because this harness has no faithful direct Rust baseline for pinned Stim's API timing.
- No RPF2 transform row is promoted into the 1.25x primary threshold gate.

Probe evidence:

- `just bench::compare --only pf2-circuit-flatten-repeat --baseline target/benchmarks/rpf2-flatten-probe/baseline.json --report target/benchmarks/rpf2-flatten-compare-probe` measured `stab_circuit_flatten_repeat_shifted_coords` at `0.000466460s`, about `2.635e7 operations/s`.
- `just bench::compare --only pf2-circuit-without-noise --baseline target/benchmarks/rpf2-without-noise-probe/baseline.json --report target/benchmarks/rpf2-without-noise-compare-probe` measured `stab_circuit_without_noise_top_level` at `0.000214474s`, about `4.774e7 source-instructions/s`.
- `just bench::compare --only pf2-circuit-decompose-mpp-spp --baseline target/benchmarks/rpf2-decompose-probe-baseline/baseline.json --report target/benchmarks/rpf2-decompose-compare-probe` measured `stab_circuit_decompose_mpp_spp` at `0.000060760s`, about `1.317e5 source-instructions/s`.
- `just bench::compare --only pf2-feedback-inline-batch --baseline target/benchmarks/pf2-feedback-repeat-probe-baseline/baseline.json --report target/benchmarks/pf2-feedback-repeat-probe-compare` measured `stab_circuit_with_inlined_feedback_mpp` at `0.000002594s`, about `3.855e5 transforms/s`, and `stab_circuit_with_inlined_feedback_repeat_loop` at `0.000052458s`, about `5.719e5 repeat-iterations/s`.
- `just bench::compare --only pf2-feedback-inline-batch --baseline target/benchmarks/pf2-feedback-xcz-ycz-probe-baseline/baseline.json --report target/benchmarks/pf2-feedback-xcz-ycz-probe-compare` measured the refreshed row with `stab_circuit_with_inlined_feedback_mpp` at `0.000002528s`, about `3.956e5 transforms/s`, `stab_circuit_with_inlined_feedback_repeat_loop` at `0.000051750s`, about `5.797e5 repeat-iterations/s`, and `stab_circuit_with_inlined_feedback_xcz_ycz` at `0.000001318s`, about `7.587e5 transforms/s`.
- `just bench::compare --only pf2-time-reverse-flow --baseline target/benchmarks/rpf2-time-reverse-flow-probe/baseline.json --report target/benchmarks/rpf2-time-reverse-flow-compare` measured `stab_circuit_time_reversed_for_flows_unitary` at `0.000009764s`, about `4.097e5 flows/s`.
- `just bench::compare --only pf2-time-reverse-flow-measurement --baseline target/benchmarks/pf2-inverted-measure-reset-probe/baseline.json --report target/benchmarks/pf2-inverted-measure-reset-compare` measured `stab_circuit_time_reversed_for_flows_measurement` at `0.000035944s`, about `1.085e6 flows/s`, with the refreshed corpus including inverted result-target measure-reset cases.

## Verification So Far

Passed for this slice:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings`
- `cargo test -p stab-core --test circuit_transforms --quiet`
- `cargo test -p stab-core --test circuit_inverse_qec --quiet`
- `cargo test -p stab-core --test circuit_inverse_qec_mpp --quiet`
- `cargo test -p stab-core --test circuit_inverse_qec time_reversed_for_flows --quiet`
- `cargo test -p stab-core --test circuit_transforms decomposed --quiet`
- `cargo test -p stab-core --test circuit_transforms feedback --quiet`
- `cargo test -p stab-core circuit --quiet`
- `cargo test -p stab-bench pf2_transform --quiet`
- `cargo test -p stab-bench --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --milestone PF2 --structural`
- `just oracle::run --implemented-only`
- `just bench::smoke`
- `just bench::baseline --only pf2-feedback-inline-batch --out target/benchmarks/pf2-feedback-repeat-probe-baseline`
- `just bench::compare --only pf2-feedback-inline-batch --baseline target/benchmarks/pf2-feedback-repeat-probe-baseline/baseline.json --report target/benchmarks/pf2-feedback-repeat-probe-compare`
- `just bench::baseline --only pf2-feedback-inline-batch --out target/benchmarks/pf2-feedback-xcz-ycz-probe-baseline`
- `just bench::compare --only pf2-feedback-inline-batch --baseline target/benchmarks/pf2-feedback-xcz-ycz-probe-baseline/baseline.json --report target/benchmarks/pf2-feedback-xcz-ycz-probe-compare`
- `just bench::baseline --only pf2-time-reverse-flow-measurement --out target/benchmarks/pf2-inverted-measure-reset-probe`
- `just bench::compare --only pf2-time-reverse-flow-measurement --baseline target/benchmarks/pf2-inverted-measure-reset-probe/baseline.json --report target/benchmarks/pf2-inverted-measure-reset-compare`

## Audit And Review

Milestone-audit for the selected measurement-rich time-reversal slice found the earlier promoted scope complete against the current PFM2 and PFM5 text: the Rust API remains additive, accepts only one noiseless plain measurement instruction group, verifies requested flows through the existing sparse tracker, keeps noisy, repeated, multi-instruction, detector, reset-only, feedback, and broader QEC inverse behavior fail-closed, and is represented by oracle row `pf2-time-reverse-flow-measurement-rust` plus report-only benchmark row `pf2-time-reverse-flow-measurement`.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or benchmark alignment.
The Rust/API sidecar found a P1 compatibility bug where reset-convertible single-target `M`, `MX`, and `MY` flows were accepted but returned the keep-measurement inverse shape; the implementation now applies Stim's batch-level measurement-to-reset decision for the selected single-target cases and adds `time_reversed_for_flows_measurement_rich_subset_turns_measurements_into_resets`.
The same sidecar found a P2 scope wording mismatch around inverted measure-reset targets; docs, manifests, and error text initially limited that slice to the single-target measure-reset shape, and a later slice first extended it only to plain unique-target groups while leaving duplicate or inverted result-target semantics for follow-up measure-reset work.
The docs and benchmark sidecar found a P2 selector mismatch where the oracle row claimed multi-instruction rejection without the test name matching the row filter, plus a P3 missing upstream provenance path; the rejection test was renamed into the `measurement_rich_subset` filter and this report now names `src/stim/util_top/circuit_inverse_qec.test.cc`.
Local review found one evidence gap before sidecar closure: the selector accepted all six measurement bases while tests initially exercised only `M` and `MZZ`; `time_reversed_for_flows_measurement_rich_subset_covers_selected_bases` now covers `MX`, `MY`, `MXX`, and `MYY`.
The selected measure-reset slice originally promoted only the single-target measure-reset shape in the same API, added `time_reversed_for_flows_measurement_rich_subset_reverses_measure_resets`, refreshed the report-only benchmark corpus, and kept broader reset-only operations, duplicate or inverted measure-reset groups, detectors, feedback, noise, repeats, multi-instruction circuits, and larger QEC inverse behavior open before later slices expanded it first to plain unique-target groups and then to inverted result targets.

The current selected reset-to-measurement and measurement-ordering slice promotes one noiseless plain unique-target `R`, `RX`, or `RY` instruction, selected multi-record `M` and `MZZ` measurement ordering, and unique-target `MR`, `MRX`, and `MRY` measure-reset flow reversal in the same API, adds `time_reversed_for_flows_measurement_rich_subset_reverses_resets`, `time_reversed_for_flows_measurement_rich_subset_reverses_multi_target_resets`, `time_reversed_for_flows_measurement_rich_subset_preserves_measurement_ordering`, `time_reversed_for_flows_measurement_rich_subset_reverses_multi_target_measure_resets`, `time_reversed_for_flows_measurement_rich_subset_reverses_inverted_measure_resets`, `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_measurement_targets`, `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_reset_targets`, `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_measure_reset_targets`, and `time_reversed_for_flows_measurement_rich_subset_rejects_unscoped_reset_terms`, refreshes the report-only benchmark corpus, and keeps duplicate reset-only and duplicate measure-reset behavior fail-closed under logged spec gaps while parser-rejected inverted reset targets, detectors, feedback, noise, repeats, multi-instruction circuits, and larger QEC inverse behavior remain open.
Full-code-review for this measurement-ordering and unique-target measure-reset slice used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or evidence alignment.
The Rust/API sidecar found a P1 compatibility bug where multi-record selected measurement and measure-reset inverses kept original target order and therefore assigned the wrong inverse `rec[...]` offsets; the implementation now builds the selected inverse with reversed target groups, remaps measurement-record terms to inverse measurement order, and updates `time_reversed_for_flows_measurement_rich_subset_preserves_measurement_ordering` plus `time_reversed_for_flows_measurement_rich_subset_reverses_multi_target_measure_resets` to assert Stim-style `M 1 0`, `MZZ 2 3 0 1`, and `MR 1 0` inverses.
The same sidecar found a P2 large-group performance risk where unique-target validation scanned a vector for every target; `measurement_groups_are_plain_unique` and `measure_reset_targets` now use set-backed duplicate detection while preserving deterministic target order for inverse construction.
The docs/evidence sidecar reported no confirmed findings and confirmed that the docs, oracle row, benchmark manifest, and PF2 runner stayed scoped to selected measurement-ordering and the then-promoted plain unique-target measure-reset support with broader QEC inverse surfaces still open.
Full-code-review for this reset-to-measurement slice used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or evidence alignment.
The Rust/API sidecar found a P1 fail-open compatibility bug where selected `R`, `RX`, or `RY` reset reversal accepted observable-bearing flows and then dropped the observable in the reversed flow; the implementation now rejects observable terms on selected reset and measure-reset flows, rejects measurement-record terms on selected reset-only input flows, and adds `time_reversed_for_flows_measurement_rich_subset_rejects_unscoped_reset_terms`.
The docs/evidence sidecar found a P3 stale evidence list in `rpf5-flow-progress-report.md`; that report now names the new reset positive and negative tests.
The current multi-target reset follow-up used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or evidence alignment.
The Rust/API sidecar reported no confirmed findings.
The docs/evidence sidecar found an under-specified arity evidence question: the implementation accepts arbitrary plain unique-target reset groups, while the initial new test only exercised two targets.
The test now also covers `R 0 1 2` reversing to `M 2 1 0` with `rec[-3] xor rec[-2] xor rec[-1]`, so the broad plain-unique-target wording has direct source-owned evidence.
The duplicate reset-only scope-reconciliation pass probed Stim v1.16.0 with `uv run --with stim==1.16.0 python` and found malformed inverse flows for duplicate reset targets, such as `R 0 0` producing `M 0 0` with `Z -> rec[-4] xor rec[-3]`. Stab therefore keeps `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_reset_targets` as the source-owned fail-closed behavior until `docs/plans/milestone-spec-gaps.md` resolves the compatibility decision.
The inverted measure-reset slice probed Stim v1.16.0 and found coherent self-validating inverse flows for inverted result targets such as `MR !0 1`, while duplicate measure-reset targets such as `MR 0 0` produced malformed out-of-range inverse flows. Stab now implements `time_reversed_for_flows_measurement_rich_subset_reverses_inverted_measure_resets`, keeps `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_measure_reset_targets` fail-closed, updates the report-only benchmark corpus, and logs the duplicate measure-reset compatibility choice in `docs/plans/milestone-spec-gaps.md`.
Milestone-audit for the selected nested bounded-repeat feedback evidence found the promoted scope complete against the current PFM2 text after keeping the claim limited to detector-parity preservation instead of full repeat-contained feedback parity.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/test correctness and docs/oracle alignment.
The Rust/test sidecar found one helper precision issue: the recursive assertion checked all classical controls, including sweep controls that feedback inlining intentionally preserves.
That issue is fixed by checking only measurement-record controls.
The docs/oracle sidecar found one evidence-row mismatch: the M9 `coverage-util-top-transform-without-feedback` row claimed the nested-repeat evidence even though its selector does not run the integration test.
That issue is fixed by leaving the nested-repeat claim on PF2 row `pf2-feedback-inline-scoped-rust`, whose selector runs `cargo test -p stab-core --test circuit_transforms feedback`.
Residual risk remains that the nested bounded-repeat case is source-owned DEM-equivalence evidence rather than an exact pinned upstream nested fixture, so broader repeat-contained feedback stays open.
Milestone-audit for the selected decomposition flow-generator preservation slice found the promoted scope complete against the current PFM2 text after limiting the claim to selected MPP and pair-measurement decomposition cases.
Full-code-review used GPT-5.5/xhigh sidecars for Rust/test correctness and docs/oracle alignment.
The Rust/test sidecar found one test-quality issue: the flow-preservation assertions could pass if `Circuit::decomposed` accidentally returned the original product-measurement circuit unchanged.
That issue is fixed by recursively rejecting surviving `MPP`, `MXX`, `MYY`, and `MZZ` gates in the decomposed output before comparing flow generators.
The docs/oracle sidecar found one closure-documentation issue: the progress report claimed the selected decomposition flow-preservation evidence without recording this slice's audit and review closure.
That issue is fixed by this audit and review entry.
Residual risk remains that broader decomposition flow semantics must be selected separately when new RPF5 measurement-rich flow families are promoted.
Milestone-audit and full-code-review for the selected `XCZ`/`YCZ` feedback slice used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or evidence alignment.
The Rust/API sidecar found a P1 compatibility bug where `with_inlined_feedback` and the sparse reverse tracker accepted upstream-invalid reversed `XCZ rec[-1] q` and `YCZ rec[-1] q` target positions while adding valid `XCZ q rec[-1]` and `YCZ q rec[-1]` support; the implementation now validates record-target placement per gate, preserves ordinary qubit-qubit `XCZ` and `YCZ` tableau propagation, keeps `CZ` symmetric, and adds `sparse_rev_frame_tracker_rejects_invalid_feedback_target_positions` plus the existing `circuit_with_inlined_feedback_rejects_unsupported_feedback_gate` regression.
The docs/evidence sidecar found a P2 evidence mismatch where the M9 `coverage-util-top-transform-without-feedback` row and roadmap bullet claimed the selected `XCZ`/`YCZ` integration test even though that row's selector only runs the core helper unit tests; the M9 row now points the `XCZ`/`YCZ` claim to the PF2 and PF7 rows whose selectors run the matching integration and CLI tests.
Milestone-audit for the initial selected direct `Circuit::inverse_qec` reset-measure-detector slice found the promoted scope complete after narrowing it to the pinned single-target `r_m_det` shape, keeping then-unimplemented multi-target reset-measure-detector parity and duplicate detector-record simplification as broader detector-flow work.
Full-code-review for that initial inverse-QEC slice used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or oracle alignment.
The Rust/API sidecar found a P1 overclaim where the selector accepted multi-target reset-measure-detector circuits unchanged even though Stim reverses target order, plus a P2 fail-open duplicate detector-record case where Stim would simplify parity.
That issue was fixed in the initial slice by requiring one reset target, one matching measurement target, and exactly one detector target `rec[-1]`, with tests rejecting noisy measurement, nonmatching basis, inverted measurement target, duplicate reset or measurement targets, empty detectors, duplicate detector records, multi-target records, and out-of-range records until the broader target-list packet was explicitly scoped.
The docs/oracle sidecar reported no confirmed findings after the stale direct-rejection wording was removed; the scope doc, roadmap, feature checklist, inventory, progress report, and oracle manifest named the selected single-target support and left broader detector-flow rewrites open before the later target-list slice expanded the same packet.
Milestone-audit for the selected multi-target inverse-QEC reset-measure-detector slice found the promoted scope complete against `pfm2-inverse-qec-multitarget-detector-scope.md`: the selector covers pinned `r_m_det`, same-basis target lists, sparse detector subsets, duplicate detector-record parity, empty detectors including target-list empty-detector behavior, detector metadata preservation when the detector survives, and fail-closed noisy, nonmatching, inverted, duplicate reset or measurement, and out-of-range cases while broader detector-flow rewrites with interleaved operations remain active.
Full-code-review for this multi-target inverse-QEC slice used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or oracle alignment.
The Rust/API sidecar found no confirmed code-level compatibility findings and noted that the linked `pfm2-inverse-qec-multitarget-detector-scope.md` scope document must be included in the change set.
The docs/oracle sidecar found a P2 closure gap because this report claimed the new target-list behavior before recording milestone-audit and full-code-review closure; this entry fixes that gap.
The same docs/oracle sidecar found a P3 oracle wording gap where the M6 and PF2 manifest rows did not restate the one-detector bound; those rows now say the executable reset-measure-detector target-list subset is for one detector and keep multi-detector detector-flow rewrites open.
Milestone-audit for the selected measure-reset pass-through inverse-QEC slice found the promoted scope complete against `pfm2-inverse-qec-measure-reset-pass-through-scope.md` after keeping the claim limited to a top-level matching reset, measurement, measure-reset, and one detector whose targets reference only the selected measure-reset group.
Local audit found one P3 evidence gap where the docs and oracle row named fail-closed nonmatching target-list behavior before pass-through-specific negative tests existed; `circuit_inverse_qec_keeps_unpromoted_measurement_rewrites_fail_closed` now covers nonmatching reset/measurement and measurement/measure-reset target lists in the four-instruction packet.
Full-code-review for this pass-through inverse-QEC slice used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or oracle alignment.
The Rust/API sidecar found a P3 stale public Rustdoc issue where `Circuit::inverse_qec` still named only reset-measure-detector support, and a P3 one-sided sparse remapping evidence issue where the positive pass-through tests covered all records and `rec[-1]` but not the mirror `rec[-2]` remap.
Those issues are fixed by updating the method Rustdoc and adding the two-target `DETECTOR rec[-2]` positive case that expects Stim-style `DETECTOR rec[-1]` after target reversal.
The docs/oracle sidecar reported no confirmed findings and confirmed that the scope note, roadmap, feature checklist, inventory, progress report, and oracle manifest describe the selected one-detector pass-through packet without implying broader detector-flow support.
Residual risk remains that this packet uses structural Rust-test evidence adapted from pinned Stim behavior instead of checked-in exact-output oracle fixtures for every accepted target-list variant, which matches the documented comparator policy for this narrow transform selector.
Milestone-audit for the selected exact two-to-one inverse-QEC slice found the promoted scope complete against `pfm2-inverse-qec-two-to-one-scope.md` after keeping the selector limited to one plain two-target `R`, one matching `CX` pair, one matching plain two-target `M`, and one detector containing exactly `rec[-1] rec[-2]`.
Full-code-review for this exact two-to-one inverse-QEC slice used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or oracle alignment.
The docs/oracle sidecar found a P2 selector-provenance issue where the reset-measure-detector oracle row still used the broad `inverse_qec` filter and therefore selected newer two-to-one tests; the reset row now filters on `reset_measure_detector`, the broad fail-closed cases live in `pf2-inverse-qec-unpromoted-measurement-rewrites-rust`, and the progress report records that separate row.
The Rust/API sidecar found P3 evidence gaps around source-owned tag or coordinate preservation and fail-closed nearby two-to-one shapes.
Those issues are fixed by narrowing the tag or coordinate claim to source-owned regression coverage informed by a recorded pinned-Stim probe, and by adding fail-closed coverage for empty detectors, detector records outside the selected group, duplicate reset or measurement targets, nonmatching reset or measurement targets, asymmetric extra reset or measurement targets, nonmatching `CX` targets, multi-pair `CX`, and record-only detector construction.
Residual risk remains that the tagged two-to-one case is not an oracle-managed exact-output fixture; the untagged pinned upstream row is the executable parity comparator, and the tag or coordinate case is documented as source-owned evidence until the oracle harness grows pinned-Stim probe fixtures for transform APIs.
Milestone-audit for the selected MPP inverse-QEC slice found the promoted scope complete against `pfm2-inverse-qec-mpp-scope.md` after adding the identity-parity guard that requires the combined all-record MPP detector parity to reduce to identity.
Local audit found one P3 evidence gap where empty detector and empty MPP product rejections were documented before direct tests existed; `circuit_inverse_qec_rejects_unpromoted_mpp_shapes` now covers both.
Full-code-review for this selected MPP inverse-QEC slice used GPT-5.5/xhigh sidecars for Rust/API behavior and docs or oracle alignment.
The Rust/API sidecar found a P1 fail-open compatibility bug where the selector accepted arbitrary Hermitian MPP products such as `MPP X0*Y1*Z2` even though pinned Stim v1.16.0 rejects the detector as reaching the start of the circuit; the implementation now validates the combined selected MPP parity as identity, moves the arbitrary single-product case into rejection coverage, and keeps deterministic identity-product coverage with `MPP !X0*X0`.
The same sidecar found a P3 scope wording issue where noisy MPP cases were described as Stim-rejected too broadly; `pfm2-inverse-qec-mpp-scope.md` now describes noisy MPP as a deliberate Stab fail-closed deferral for this packet because pinned Stim accepts some deterministic noisy identity products.
The docs/oracle sidecar found a P2 metadata overclaim where the M6 umbrella `coverage-util-top-circuit-inverse-qec` row implied it ran the separate MPP test binary; that row now points MPP evidence to `pf2-inverse-qec-mpp-rust`, whose selector runs `cargo test -p stab-core --test circuit_inverse_qec_mpp`.
Residual risk remains that tagged MPP metadata preservation is source-owned probe evidence rather than an oracle-managed exact-output fixture, and noisy deterministic identity MPP remains intentionally fail-closed until a later scope selects noisy MPP parity.

Still required before claiming the RPF2 milestone complete:

- `cargo fmt --all --check`
- `cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings`
- `cargo test -p stab-core circuit --quiet`
- `cargo test -p stab-bench --quiet`
- `just oracle::run --milestone PF2`
- `just bench::smoke`
- Milestone-audit and full-code-review for the whole RPF2 milestone after the remaining transform subfeatures are closed or explicitly logged as spec gaps.
