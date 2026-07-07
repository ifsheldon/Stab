# RPF5 Missing Detectors Progress Report

## Summary

This RPF5 slice promotes the Rust `missing_detectors` utility beyond the M9 basic reset and single-record subset.
It adds Gaussian row reduction over detector and observable rows plus a scoped internal stabilizer-invariant tracker for deterministic reset, measurement, MPP, and pair-measurement cases.
It also promotes tableau-backed single-qubit and fixed two-qubit Clifford propagation for plain qubit target groups.
It also promotes `SPP` and `SPP_DAG` unitary Pauli-product instructions by analyzing their existing decomposition into the supported Clifford subset.
It also promotes bounded repeat traversal with explicit expansion caps for the current materialized Rust utility surface plus a selected folded final-repeat fast path for covered deterministic measurement loops, including bounded nested local repeat bodies, that prove an empty suffix without materializing every iteration.
It is not an RPF5 completion report because broader folded large-repeat traversal beyond the selected final-repeat empty-suffix cases, public measurement-rich flow solving, and transform integration remain active work.
Broader generated-code missing-detector suffix analysis beyond the pinned honeycomb and toric cases is now logged as under-specified until a future plan names exact generated families and suffix comparators.

## Implemented Surfaces

- Existing `DETECTOR` rows now participate in Gaussian elimination instead of being limited to single-record coverage.
- Repeated deterministic MPP and pair-measurement stabilizer-product measurements produce missing-detector suggestions compatible with the pinned Stim v1.16.0 subcases ported in this slice.
- Record-only `OBSERVABLE_INCLUDE` rows participate as known rows.
- `OBSERVABLE_INCLUDE` rows with Pauli targets mark that observable row ignored, matching the pinned Stim behavior used by the promoted tests.
- The pinned Stim big honeycomb-code and toric global-stabilizer generated-code suffix cases are promoted under unknown-input semantics.
- Tableau-backed single-qubit Clifford gates, fixed two-qubit Clifford gates, and SWAP-family gates propagate tracked invariants when their target groups are plain qubit targets.
- `SPP` and `SPP_DAG` unitary Pauli-product gates reuse the existing single-instruction decomposition path, so supported Hermitian Pauli products are analyzed equivalently to their decomposed `H`/`S`/`CX` circuit and anti-Hermitian products fail closed with a domain error.
- Repeat blocks are traversed by bounded materialized expansion, with explicit rejection for excessive expanded work units or repeat iterations before traversal mutates analysis state; `SPP` and `SPP_DAG` instructions are charged by their decomposed work in this budget.
- Ordinary noise gates are ignored by this diagnostic utility for the promoted cases, while unsupported gates and non-plain unitary target groups still fail closed.

## Tests

Implemented Rust tests:

- `missing_detectors_reduces_multi_record_detector_rows`
- `missing_detectors_supports_mpp_stabilizer_products`
- `missing_detectors_supports_observable_interactions`
- `missing_detectors_supports_mpp_observable_subset`
- `missing_detectors_supports_honeycomb_generated_code_suffix`
- `missing_detectors_supports_toric_global_stabilizer_product`
- `missing_detectors_handles_bounded_repeat_blocks`
- `pf5_missing_detectors_clifford_tracks_single_qubit_basis_changes`
- `pf5_missing_detectors_clifford_covers_all_single_qubit_cliffords`
- `pf5_missing_detectors_clifford_tracks_two_qubit_and_swap_gates`
- `pf5_missing_detectors_clifford_covers_all_fixed_two_qubit_tableau_gates`
- `pf5_missing_detectors_clifford_rejects_non_plain_unitary_targets`
- `pf5_missing_detectors_spp_has_pinned_outputs`
- `pf5_missing_detectors_spp_supports_unitary_products`
- `pf5_missing_detectors_spp_rejects_anti_hermitian_unitary_products`
- `pf5_missing_detectors_repeat_tracks_deterministic_measurements`
- `pf5_missing_detectors_repeat_handles_nested_rows_and_known_rows`
- `pf5_missing_detectors_repeat_folds_final_covered_deterministic_loop`
- `pf5_missing_detectors_nested_final_repeat_folds_local_bodies`
- `pf5_missing_detectors_nested_final_repeat_keeps_unselected_bodies_capped`
- `pf5_missing_detectors_repeat_keeps_unselected_large_repeats_capped`
- `pf5_missing_detectors_repeat_rejects_excessive_expansion`

These tests cover Gaussian cleanup for multi-record detector rows, repeated MPP stabilizer-product constraints, unknown-input behavior, record-only observable rows, ignored Pauli observable rows, the promoted honeycomb and toric generated-code suffixes, all single-qubit Clifford gates, every canonical fixed two-qubit tableau gate, hand-pinned non-self-inverse `S`, signed `ISWAP_DAG`, exact expected outputs for representative `SPP`, `SPP_DAG`, inverted, multi-group, and unknown-input cases, `SPP` and `SPP_DAG` parity against explicit decomposition for complex products, anti-Hermitian `SPP` and `SPP_DAG` rejection, nondeterministic post-Clifford measurement cases, bounded repeat traversal through deterministic measurements, nested repeats, known detector and observable rows after repeats, selected folded final-repeat traversal for covered deterministic measurement loops with body-local record references, bounded nested local repeat bodies, detector rows after nested bounded measurements, and unchanged tracker state, fallback rejection for cross-iteration record references, nested cross-iteration record references, observable rows, unsupported local bodies, tracker-changing repeated bodies, nested large repeats, excessive repeat rejection including decomposed `SPP` repeat work, and fail-closed behavior for non-plain unitary targets.

## Oracle Rows

Implemented rows:

- `pf5-missing-detectors-row-reduction-rust`
- `pf5-missing-detectors-mpp-observable-rust`
- `pf5-missing-detectors-generated-honeycomb-rust`
- `pf5-missing-detectors-generated-toric-rust`
- `pf5-missing-detectors-clifford-rust`
- `pf5-missing-detectors-spp-rust`
- `pf5-missing-detectors-repeat-rust`
- `pf5-missing-detectors-nested-final-repeat-rust`

Still broad and manifest-only:

- `pf5-missing-detectors-extended`

## Benchmark Rows

Report-only runner coverage:

- `pf5-missing-detectors-mpp`
- `pf5-missing-detectors-generated-code`

The row measures the promoted MPP and observable-row workload through the Rust public utility API.
The generated-code row measures the promoted honeycomb and toric generated-code suffix workloads through the Rust public utility API.
No additional generated-code suffix benchmark row should be added until broader generated-code missing-detector subcases are specified with exact workloads.
Both rows remain `non-primary-report-only` because pinned Stim does not provide a faithful CLI timing ratio for this Rust utility surface.
They are not part of the 1.25x primary threshold file.
The `SPP` and `SPP_DAG` slice is structural parity work that reuses the existing decomposition path and is not separately benchmarked; the generated-code row remains the performance-oriented missing-detectors workload.
The bounded and selected folded final-repeat traversal slices are structural resource-boundary work and are not separately benchmarked.

## Evidence Repair

The 2026-07-06 MPP and observable evidence-hardening slice found that `pf5-missing-detectors-mpp-observable-rust` used the broad `missing_detectors_supports_` Cargo filter, which also matched generated-code suffix tests.
The slice adds `missing_detectors_supports_mpp_observable_subset` as a focused integration-test mirror for the promoted repeated MPP stabilizer-product, record-only observable-row, Pauli-observable-row, and unknown-input cases.
The oracle manifest now narrows `pf5-missing-detectors-mpp-observable-rust` to `cargo-test|-p|stab-core|--test|missing_detectors|missing_detectors_supports_mpp_observable_subset`.
The scope note is `docs/plans/pfm5-missing-detectors-mpp-observable-evidence-scope.md`.

Focused checks for the MPP and observable evidence-hardening slice:

```sh
cargo test -p stab-core --test missing_detectors missing_detectors_supports_mpp_observable_subset --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF5 --structural
```

The 2026-07-06 toric evidence-repair slice found that this report and `oracle/fixtures/manifest.csv` already named `missing_detectors_supports_toric_global_stabilizer_product`, but the manifest used a package-wide Cargo filter that also matched an internal unit test with the same name.
The slice adds the integration-test mirror beside the other PF5 missing-detectors rows and narrows `pf5-missing-detectors-generated-toric-rust` to `--test missing_detectors`, without changing `missing_detectors` behavior.
The focused integration test proves the existing implementation already matches the pinned Stim v1.16.0 toric global-stabilizer suffix expectation.
The scope note is `docs/plans/pfm5-missing-detectors-toric-evidence-repair-scope.md`.

Focused checks for the repair:

```sh
cargo test -p stab-core --test missing_detectors missing_detectors_supports_toric_global_stabilizer_product --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF5 --structural
```

## Verification Evidence

Current folded-final-repeat and bounded nested final-repeat slice checks:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-core --test missing_detectors pf5_missing_detectors_repeat --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF5 --structural
just oracle::run --implemented-only
just bench::smoke
just maintenance::pre-commit
```

Target checks for this slice:

```sh
cargo test -p stab-core missing_detectors --quiet
cargo test -p stab-core --test missing_detectors --quiet
cargo test -p stab-core --test missing_detectors pf5_missing_detectors_clifford --quiet
cargo test -p stab-core --test missing_detectors pf5_missing_detectors_spp --quiet
cargo test -p stab-core --test missing_detectors pf5_missing_detectors_repeat --quiet
cargo test -p stab-core --test missing_detectors pf5_missing_detectors_nested_final_repeat --quiet
cargo test -p stab-bench pf5::detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench --all-targets -- -D warnings
just oracle::run --milestone PF5
just bench::smoke
just bench::compare --milestone PF5
```

## Remaining RPF5 Work

- Broader generated-code missing-detector suffix analysis beyond the promoted honeycomb and toric cases is under-specified in `docs/plans/milestone-spec-gaps.md`; do not implement or claim another generated-code suffix row until exact generated families, suffix comparators, resource behavior, oracle metadata, and benchmark policy are selected.
- Broader folded large-repeat traversal beyond the selected final covered deterministic measurement-loop fast paths with flat or bounded nested local bodies, and generated-code gate families beyond tableau-backed single-qubit and fixed two-qubit Clifford propagation plus `SPP` or `SPP_DAG` decomposition in the invariant tracker.
- Public measurement-rich flow semantics beyond the promoted unsigned `has_flow`, unsigned `has_all_flows`, unsigned diagnostic Rust helper, scoped signed sampled Rust checker, and current generator subsets, including Python-style signed sampled binding shape, broader composed `flow_generators`, solver or generator diagnostics, and transform integration.
- Continue keeping benchmark harness smoke tests split out of `ops/bench/src/baseline/tests.rs`, because the file is close to the project’s 1200-line threshold.

## Audit And Review

Local milestone-audit for the selected MPP and observable evidence-hardening slice found the scope, integration test, and narrowed oracle command complete against `docs/plans/pfm5-missing-detectors-mpp-observable-evidence-scope.md`.
The audit found no new under-specification requiring an entry in `docs/plans/milestone-spec-gaps.md`.

Full-code-review used GPT-5.5/xhigh sidecars for Rust or oracle evidence and docs or milestone alignment.
The Rust or oracle sidecar found no P0, P1, or P2 issues and confirmed that `pf5-missing-detectors-mpp-observable-rust` now points uniquely to the focused integration test.
The docs or milestone-alignment sidecar found no P0, P1, or P2 issues and confirmed no spec-gap entry is needed.
No remaining P0, P1, or P2 findings are known for this MPP and observable evidence-hardening slice.

Local milestone-audit for the selected toric evidence-repair slice found the updated scope, integration test, and narrowed oracle command complete against `docs/plans/pfm5-missing-detectors-toric-evidence-repair-scope.md`.
The audit found no new under-specification requiring an entry in `docs/plans/milestone-spec-gaps.md`.

Full-code-review used GPT-5.5/xhigh sidecars for Rust or oracle evidence and docs or milestone alignment.
The Rust or oracle sidecar found a P2 evidence-ownership ambiguity: `pf5-missing-detectors-generated-toric-rust` used a package-wide Cargo filter, which matched both the existing internal unit test and the new integration test with the same name.
The oracle manifest now narrows the row to `cargo-test|-p|stab-core|--test|missing_detectors|missing_detectors_supports_toric_global_stabilizer_product`, and the scope note plus this report now describe the slice as an ambiguity repair instead of a zero-test repair.
The docs or milestone-alignment sidecar found no P0, P1, or P2 issues.
No remaining P0, P1, or P2 findings are known for this toric evidence-repair slice.

Local milestone-audit for the selected folded-final-repeat slice found the scope complete after fixing two proof-boundary issues: observable rows are now explicit fold non-goals because they merge by observable id across iterations, and proof-run analyzer errors now fall back to the original repeat-budget path instead of changing the public error class.

Full-code-review used GPT-5.5/xhigh sidecars for Rust compatibility and docs or oracle evidence.
The Rust sidecar found a P2 fallback bug where unsupported local bodies such as `SHIFT_COORDS` or `MPAD` could return proof-run analyzer errors before the original expanded-repeat budget error; the fold proof now treats processing errors as ineligibility and the regression covers unsupported local bodies.
The docs or oracle sidecar found a P2 scope-alignment gap where the positive scope under-specified the observable-row exclusion and the audit block still described an older SPP slice; the scope note, report, inventory, checklist, roadmap, and oracle metadata now describe selected folded final-repeat traversal plus explicit observable-row fallback.
No remaining P0, P1, or P2 findings are known for this folded-final-repeat slice.

Local milestone-audit for the bounded nested final-repeat slice found two P2 issues in the first pass: the recursive fold proof ran before the repeat-depth budget on public-API constructed deep repeat trees, and the nested-final cases were embedded in aggregate repeat tests instead of a unique oracle row.
The implementation now validates the prefix and one bounded final-repeat body before recursive fold eligibility checks, adds a public-API over-depth nested-repeat regression, splits the nested-final cases into `pf5_missing_detectors_nested_final_repeat_` tests, and adds `pf5-missing-detectors-nested-final-repeat-rust` as focused oracle evidence.
Full-code-review used GPT-5.5/xhigh sidecars for Rust compatibility and docs or oracle evidence; no P0 or P1 findings were reported, and the P2 findings above are fixed.
No remaining P0, P1, or P2 findings are known for this bounded nested final-repeat slice.
