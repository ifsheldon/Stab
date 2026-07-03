# PF1 Gate Metadata Progress Report

## Summary

This PF1 slice implements the bounded Rust gate-metadata accessor subset from `docs/plans/partial-feature-closure-plan.md`.
It does not claim Python `GateData` parity.
Flow metadata, tableau metadata, unitary-matrix metadata, decomposition metadata, Python object shape, Python string or repr output, and Python binding behavior remain partial or deferred according to `docs/stab-feature-checklist.md`.

## Implemented Surfaces

- Added public `GateArgumentRule`, `GateTargetRule`, and `GateTargetGroupKind` typed metadata enums.
- Added public `Gate` accessors for aliases, argument rule, target rule, target grouping, fusing, noisy/reset/measurement/unitary/single-qubit/two-qubit/target-capability/symmetry flags, unitary inverse, and generalized inverse.
- Fixed parser validation for the owned metadata subset so `I_ERROR` and `II_ERROR` accept any-length disjoint probability lists like Stim v1.16.0, and `XCX`, `XCY`, `YCX`, and `YCY` reject measurement-record and sweep-bit targets instead of inheriting the bit-target-capable controlled-gate rule.
- Matched the implemented `GateData`-style flags more tightly by removing `MPAD` from `is_noisy`, removing `MPAD` from `is_symmetric_gate`, and using Stim's explicit symmetric two-qubit gate set for the owned accessor subset.
- Added an executable oracle manifest row for the implemented Rust accessor subset while leaving the broad PF1 gate-metadata manifest row as the remaining extraction contract.
- Added a report-only PF1 benchmark runner for metadata flag reads, inverse reads, and alias lookup.

## Oracle Rows

Implemented row:

- `pf1-gate-metadata-rust-accessors`
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

Fresh probe rates from the current worktree after review fixes:

- `stab_gate_metadata_flags_all_gates`: `1.431e8 gates/s`.
- `stab_gate_metadata_inverse_all_gates`: `2.077e8 gates/s`.
- `stab_gate_metadata_alias_lookup_all_aliases`: `4.973e8 lookups/s`.

This benchmark remains `non-primary-report-only` because pinned Stim exposes the comparable rich `GateData` surface through Python bindings and C++ internals, not through a faithful Rust direct baseline.
It was not added to `benchmarks/m12-primary-thresholds.json`.

## Verification Evidence

Passed during implementation:

```sh
cargo test -p stab-core --test gate_metadata --quiet
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

Milestone-audit and full-code-review sidecars found eight issues, all fixed before this report was finalized:

- `I_ERROR` and `II_ERROR` incorrectly used one-probability validation instead of any-length disjoint probability lists.
- `XCX`, `XCY`, `YCX`, and `YCY` were overclassified as bit-target-capable controlled gates.
- `is_symmetric_gate` missed `XCX`, `YCY`, and `CZ`, and incorrectly treated `MPAD` as symmetric.
- `is_noisy` incorrectly treated `MPAD` as noisy instead of matching Stim's `GateData.is_noisy_gate` flag.
- The new PF1 tests initially lived in oversized `stim_format.rs`; they now live in `crates/stab-core/tests/gate_metadata.rs`.
- The `is_noisy` Rustdoc initially described a broader semantic predicate; it now states that it follows Stim v1.16.0 `GateData.is_noisy_gate`.
- The broad implemented-oracle sweep initially exposed a stale detection-conversion rejection fixture using invalid `XCX sweep[0] 0`; it now uses valid-but-unsupported `XCZ sweep[0] 0` so parser validation and converter capability are tested at the correct boundaries.
- The same broad oracle sweep exposed a stale feedback-inlining rejection fixture using invalid `XCX rec[-1] 1`; it now uses valid-but-unsupported `XCZ rec[-1] 1` so parser validation and transformer capability stay separated.

## Remaining PF1 Gate Metadata Work

- Public flow metadata accessors.
- Public tableau metadata accessors.
- Public unitary-matrix metadata accessors.
- Public decomposition metadata accessors.
- Unsupported metadata error behavior for accessors that cannot be represented by Stab's Rust API.
- Any Python `GateData` class shape or binding behavior, which remains deferred.
