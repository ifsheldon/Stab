# PF1 Gate Metadata Progress Report

## Summary

This PF1 slice implements the bounded Rust gate-metadata accessor subset from `docs/plans/partial-feature-closure-plan.md`.
It does not claim Python `GateData` parity.
H/S/CX/M/R decomposition metadata is now implemented as Rust gate-table metadata.
Measurement-rich or variable-target flow metadata, Python object shape, Python string or repr output, and Python binding behavior remain partial or deferred according to `docs/stab-feature-checklist.md`.

## Implemented Surfaces

- Added public `GateArgumentRule`, `GateTargetRule`, and `GateTargetGroupKind` typed metadata enums.
- Added public `Gate` accessors for aliases, argument rule, target rule, target grouping, fusing, noisy/reset/measurement/unitary/single-qubit/two-qubit/target-capability/symmetry flags, unitary inverse, and generalized inverse.
- Added public `Gate::tableau` and `Gate::has_tableau` accessors for gates with existing local Clifford tableau metadata, with fail-closed errors for gates without fixed tableau data.
- Added public `Gate::flows` and `Gate::has_flows` accessors for tableau-backed unitary flow metadata, with fail-closed errors for measurement-rich, variable-target, annotation, and noisy gates that are owned by later flow milestones.
- Added public `GateUnitaryMatrix`, `Gate::unitary_matrix`, and `Gate::has_unitary_matrix` accessors for fixed-shape one- or two-qubit unitary metadata, with fail-closed errors for variable-target, measurement-rich, annotation, and noisy gates.
- Added public `GateDecomposition`, `Gate::h_s_cx_m_r_decomposition`, and `Gate::has_h_s_cx_m_r_decomposition` accessors for Stim v1.16.0 H/S/CX/M/R gate-table decomposition metadata, with fail-closed errors for gates without decomposition metadata.
- Added `docs/plans/rpf1-gate-execution-support-contract.md` to separate parser validation, metadata accessors, sampler support, detection-conversion support, analyzer support, and explicit `SPP` or `SPP_DAG` rejections for every canonical gate.
- Fixed parser validation for the owned metadata subset so `I_ERROR` and `II_ERROR` accept any-length disjoint probability lists like Stim v1.16.0, and `XCX`, `XCY`, `YCX`, and `YCY` reject measurement-record and sweep-bit targets instead of inheriting the bit-target-capable controlled-gate rule.
- Matched the implemented `GateData`-style flags more tightly by removing `MPAD` from `is_noisy`, removing `MPAD` from `is_symmetric_gate`, and using Stim's explicit symmetric two-qubit gate set for the owned accessor subset.
- Added executable oracle manifest rows for the implemented Rust accessor subset while leaving the broad PF1 gate-metadata manifest row as the remaining extraction contract.
- Added a report-only PF1 benchmark runner for metadata flag reads, inverse reads, tableau reads, tableau-backed flow reads, fixed-shape unitary matrix reads, H/S/CX/M/R decomposition reads, and alias lookup.

## Oracle Rows

Implemented row:

- `pf1-gate-metadata-rust-accessors`
- `pf1-gate-tableau-metadata`
- `pf1-gate-flow-metadata`
- `pf1-gate-unitary-matrix-metadata`
- `pf1-gate-decomposition-metadata`
- `pf1-gate-metadata-identity-error-probabilities`
- `pf1-gate-metadata-controlled-bit-targets`

Still broad and manifest-only:

- `pf1-gate-metadata-api`

## Benchmark Rows

Non-primary report-only row:

- `pf1-gate-metadata-lookup`

Probe reports:

- `target/benchmarks/pf1-gate-metadata-probe/baseline.json`
- `target/benchmarks/pf1-gate-metadata-compare/compare.json`

Fresh probe rates from the current worktree after adding tableau, tableau-backed flow, fixed-shape unitary matrix, and H/S/CX/M/R decomposition metadata accessors:

- `stab_gate_metadata_flags_all_gates`: `1.394e8 gates/s`.
- `stab_gate_metadata_inverse_all_gates`: `2.088e8 gates/s`.
- `stab_gate_metadata_tableau_supported_gates`: `8.542e6 tableaus/s`.
- `stab_gate_metadata_flows_supported_gates`: `8.825e6 flows/s`.
- `stab_gate_metadata_unitary_supported_gates`: `4.106e8 entries/s`.
- `stab_gate_metadata_decomposition_text_supported_gates`: `2.598e9 bytes/s`.
- `stab_gate_metadata_decomposition_parse_supported_gates`: `8.703e6 instructions/s`.
- `stab_gate_metadata_alias_lookup_all_aliases`: `4.973e8 lookups/s`.

This benchmark remains `non-primary-report-only` because pinned Stim exposes the comparable rich `GateData` surface through Python bindings and C++ internals, not through a faithful Rust direct baseline.
It was not added to `benchmarks/m12-primary-thresholds.json`.

## Verification Evidence

Passed during implementation:

```sh
cargo test -p stab-core --test gate_metadata --quiet
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
just bench::baseline --only pf1-gate-metadata-lookup --out target/benchmarks/pf1-gate-metadata-probe
just bench::compare --only pf1-gate-metadata-lookup --baseline target/benchmarks/pf1-gate-metadata-probe/baseline.json --report target/benchmarks/pf1-gate-metadata-compare
```

## Audit And Review

Milestone-audit and full-code-review sidecars found ten issues, all fixed before this report was finalized:

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

## Remaining PF1 Gate Metadata Work

- Measurement-rich, non-unitary, or variable-target flow metadata accessors.
- Implementation of `SPP` and `SPP_DAG` sampler, detector conversion, and analyzer semantics if later RPF3 or RPF6 milestones promote those gates from explicit rejection to supported execution.
- Unsupported metadata error behavior for any additional accessors that cannot be represented by Stab's Rust API.
- Any Python `GateData` class shape or binding behavior, which remains deferred.
