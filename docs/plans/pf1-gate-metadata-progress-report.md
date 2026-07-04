# PF1 Gate Metadata Progress Report

## Summary

This PF1 slice implements the bounded Rust gate-metadata accessor subset from `docs/plans/partial-feature-closure-plan.md`.
It does not claim Python `GateData` parity.
H/S/CX/M/R decomposition metadata is now implemented as Rust gate-table metadata.
Measurement-rich and variable-target gate-table flow metadata is now implemented for Stim v1.16.0 `GateData.flows` shapes.
Python object shape, Python string or repr output, Python binding behavior, and unpromoted execution semantics remain deferred or owned by later execution milestones according to `docs/stab-feature-checklist.md`.

## Implemented Surfaces

- Added public `GateArgumentRule`, `GateTargetRule`, and `GateTargetGroupKind` typed metadata enums.
- Added public `Gate` accessors for aliases, argument rule, target rule, target grouping, fusing, noisy/reset/measurement/unitary/single-qubit/two-qubit/target-capability/symmetry flags, unitary inverse, and generalized inverse.
- Added public `Gate::tableau` and `Gate::has_tableau` accessors for gates with existing local Clifford tableau metadata, with fail-closed errors for gates without fixed tableau data.
- Added public `Gate::flows` and `Gate::has_flows` accessors for Stim v1.16.0 `GateData.flows` metadata, covering tableau-backed unitary gates plus representative measurement-rich and variable-target gates such as `M`, `MXX`, `MPP`, `SPP`, and `SPP_DAG`, with fail-closed errors for `MPAD`, annotation, and noisy gates without flow metadata.
- Added public `GateUnitaryMatrix`, `Gate::unitary_matrix`, and `Gate::has_unitary_matrix` accessors for fixed-shape one- or two-qubit unitary metadata, with fail-closed errors for variable-target, measurement-rich, annotation, and noisy gates.
- Added public `GateDecomposition`, `Gate::h_s_cx_m_r_decomposition`, and `Gate::has_h_s_cx_m_r_decomposition` accessors for Stim v1.16.0 H/S/CX/M/R gate-table decomposition metadata, with fail-closed errors for gates without decomposition metadata.
- Added `docs/plans/rpf1-gate-execution-support-contract.md` to separate parser validation, metadata accessors, sampler support, detection-conversion support, analyzer support, and explicit `SPP` or `SPP_DAG` rejections for every canonical gate.
- Added a metadata-column support-contract regression test so the table's validation, tableau, unitary, flow, and decomposition columns stay synchronized with Rust accessors and every canonical gate appears exactly once.
- Fixed parser validation for the owned metadata subset so `I_ERROR` and `II_ERROR` accept any-length disjoint probability lists like Stim v1.16.0, and `XCX`, `XCY`, `YCX`, and `YCY` reject measurement-record and sweep-bit targets instead of inheriting the bit-target-capable controlled-gate rule.
- Matched the implemented `GateData`-style flags more tightly by removing `MPAD` from `is_noisy`, removing `MPAD` from `is_symmetric_gate`, and using Stim's explicit symmetric two-qubit gate set for the owned accessor subset.
- Added executable oracle manifest rows for the implemented Rust accessor subset, including a selected closure row for the current PF1 Rust gate metadata surface.
- Added a report-only PF1 benchmark runner for metadata flag reads, inverse reads, tableau reads, gate-table flow reads, fixed-shape unitary matrix reads, H/S/CX/M/R decomposition reads, and alias lookup.

## Oracle Rows

Selected closure row:

- `pf1-gate-metadata-api`

Implemented supporting rows:

- `pf1-gate-metadata-rust-accessors`
- `pf1-gate-tableau-metadata`
- `pf1-gate-flow-metadata`
- `pf1-gate-unitary-matrix-metadata`
- `pf1-gate-decomposition-metadata`
- `pf1-gate-metadata-identity-error-probabilities`
- `pf1-gate-metadata-controlled-bit-targets`

The selected closure row runs `cargo test -p stab-core --test gate_metadata` and is intentionally scoped to Rust metadata accessors, metadata-column support-contract synchronization, validation regressions, and parser-versus-execution rejection boundaries for the owned `SPP` and `SPP_DAG` cases. It does not claim Python `GateData` object shape, execution-column support-contract synchronization, or promotion of `SPP` and `SPP_DAG` execution support.

## Benchmark Rows

Non-primary report-only row:

- `pf1-gate-metadata-lookup`

Recorded probe reports from the original PF1 gate metadata slice:

- `target/benchmarks/pf1-gate-flow-metadata-probe/baseline.json`
- `target/benchmarks/pf1-gate-flow-metadata-compare/compare.json`

Recorded probe rates from the original PF1 gate metadata slice after expanding `Gate::flows` to Stim v1.16.0 `GateData.flows` metadata:

- `stab_gate_metadata_flags_all_gates`: `1.441e8 gates/s`.
- `stab_gate_metadata_inverse_all_gates`: `2.093e8 gates/s`.
- `stab_gate_metadata_tableau_supported_gates`: `8.445e6 tableaus/s`.
- `stab_gate_metadata_flows_supported_gates`: `7.443e6 flows/s`.
- `stab_gate_metadata_unitary_supported_gates`: `3.781e8 entries/s`.
- `stab_gate_metadata_decomposition_text_supported_gates`: `2.579e9 bytes/s`.
- `stab_gate_metadata_decomposition_parse_supported_gates`: `8.770e6 instructions/s`.
- `stab_gate_metadata_alias_lookup_all_aliases`: `4.819e8 lookups/s`.

This benchmark remains `non-primary-report-only` because pinned Stim exposes the comparable rich `GateData` surface through Python bindings and C++ internals, not through a faithful Rust direct baseline.
It was not added to `benchmarks/m12-primary-thresholds.json`.

## Verification Evidence

Passed during implementation:

```sh
cargo test -p stab-core --test gate_metadata --quiet
cargo test -p stab-core --test gate_metadata gate_metadata_api_contract --quiet
cargo test -p stab-core --test gate_metadata gate_tableau_metadata --quiet
cargo test -p stab-core --test gate_metadata gate_flow_metadata --quiet
cargo test -p stab-core gate_unitary_matrix --quiet
cargo test -p stab-core --test gate_metadata gate_decomposition_metadata --quiet
cargo test -p stab-core --test gate_metadata gate_execution_contract --quiet
cargo test -p stab-core --test dem_analyzer_mpp spp --quiet
cargo test -p stab-core sampling --quiet
cargo test -p stab-core feedback --quiet
cargo test -p stab-bench pf1_gate_metadata --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench manifest --quiet
just oracle::run --milestone PF1
just oracle::run --implemented-only
just bench::smoke
just bench::baseline --only pf1-gate-metadata-lookup --out target/benchmarks/pf1-gate-flow-metadata-probe
just bench::compare --only pf1-gate-metadata-lookup --baseline target/benchmarks/pf1-gate-flow-metadata-probe/baseline.json --report target/benchmarks/pf1-gate-flow-metadata-compare
```

## Audit And Review

Milestone-audit and full-code-review sidecars found the following issues, all fixed before this report was finalized:

- `I_ERROR` and `II_ERROR` incorrectly used one-probability validation instead of any-length disjoint probability lists.
- `XCX`, `XCY`, `YCX`, and `YCY` were overclassified as bit-target-capable controlled gates.
- `is_symmetric_gate` missed `XCX`, `YCY`, and `CZ`, and incorrectly treated `MPAD` as symmetric.
- `is_noisy` incorrectly treated `MPAD` as noisy instead of matching Stim's `GateData.is_noisy_gate` flag.
- The new PF1 tests initially lived in oversized `stim_format.rs`; they now live in `crates/stab-core/tests/gate_metadata.rs`.
- The `is_noisy` Rustdoc initially described a broader semantic predicate; it now states that it follows Stim v1.16.0 `GateData.is_noisy_gate`.
- The broad implemented-oracle sweep initially exposed a stale detection-conversion rejection fixture using invalid `XCX sweep[0] 0`; it now uses valid-but-unsupported `XCZ sweep[0] 0` so parser validation and converter capability are tested at the correct boundaries.
- The same broad oracle sweep exposed a stale feedback-inlining rejection fixture using invalid `XCX rec[-1] 1`; it now uses valid-but-unsupported `XCZ rec[-1] 1` so parser validation and transformer capability stay separated.
- The fixed-shape unitary matrix accessor initially relied on tableau conversion for most supported gates, which missed global-phase-sensitive exact matrix drift; it now compares all 46 supported gates against the upstream-derived matrix corpus.
- The unitary matrix accessor initially returned nested vectors despite documenting a fixed one- or two-qubit shape; it now returns the `GateUnitaryMatrix` enum and materializes nested rows only for generic matrix consumers.
- The RPF1 review found that `SPP` and `SPP_DAG` detection conversion still accepted `skip_reference_sample=true`; conversion planning now rejects both gates before reference-sample selection, and tests cover both reference modes plus the public conversion helper.
- The RPF1 review found that the first decomposition benchmark mixed metadata lookup with parsing and printing; it is now split into pure decomposition-text reads measured in bytes per second and parseability checks measured in instructions per second.
- The RPF1 review found that exact raw decomposition text coverage was representative instead of exhaustive; the gate metadata test now compares all 61 strings against the pinned `vendor/stim/src/stim/gates/gate_data_*.cc` files.
- The PFM1 gate-flow metadata slice resolved the remaining measurement-rich and variable-target `GateData.flows` metadata decision by adding exact metadata tests for `M`, `MXX`, `MPP`, and `SPP`, representative circuit satisfaction checks for measurement-rich gates, and unchanged execution-boundary rejection tests for `SPP` and `SPP_DAG`.
- The PFM1 gate-flow metadata review found stale `Flow` metadata cells in [rpf1-gate-execution-support-contract.md](rpf1-gate-execution-support-contract.md) and a stale issue-count summary in this report; both are fixed in the same change set as the metadata implementation.

## Remaining PF1 Gate Metadata Work

- No active PF1 Rust metadata accessor subcase remains for the current Rust metadata surface.
- Implementation of `SPP` and `SPP_DAG` sampler, detector conversion, and analyzer semantics if later RPF3 or RPF6 milestones promote those gates from explicit rejection to supported execution.
- Any Python `GateData` class shape or binding behavior, which remains deferred.

## PFM0 Refresh

The PFM0 refresh promoted `pf1-gate-metadata-api` from a manifest-only extraction row to executable structural evidence and synchronized the checklist row for gate validation flags and categories to `Done for current Rust metadata surface`.
Gate semantic execution remains partial and separately owned by PFM3, PFM5, and PFM6 surfaces where unpromoted execution semantics are still active.
