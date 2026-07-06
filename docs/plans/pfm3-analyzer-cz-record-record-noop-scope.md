# PFM3 Analyzer `CZ` Record-Record No-Op Scope

## Purpose

This PFM3 slice evidence-locks one selected analyzer target-shape subcase: `CZ rec[-a] rec[-b]` is accepted by `circuit_to_detector_error_model` and `stab analyze_errors` as a classical-only no-op.
The behavior matches pinned Stim v1.16.0, which emits the same detector error model as the circuit without the classical-only `CZ`.

This is an analyzer-only scope note.
It does not promote record-record controlled gates for the sampler, measurement-to-detection conversion, detection sampling, feedback inlining, sparse reverse frame tracking, detecting regions beyond the already separate PF5 slice, or any new public API.

## Comparator

The comparator is exact DEM output parity against pinned Stim v1.16.0 for the selected public analyzer command shape.

Representative accepted probe:

```text
X_ERROR(0.25) 0
M 1 2
CZ rec[-1] rec[-2]
M 0
DETECTOR rec[-1]
```

Pinned Stim v1.16.0 emits:

```text
error(0.25) D0
```

Representative rejected neighboring probe:

```text
X_ERROR(0.25) 0
M 1 2
CY rec[-1] rec[-2]
M 0
DETECTOR rec[-1]
```

Pinned Stim v1.16.0 rejects the second target as non-qubit, and Stab must continue to reject this shape instead of silently widening the no-op rule to non-`CZ` controlled Pauli gates.

## Owned Positive Scope

- `CZ rec[-a] rec[-b]` no-op behavior in `circuit_to_detector_error_model`.
- The same no-op behavior through public `stab analyze_errors` stdin/stdout execution.
- Existing `CZ sweep[i] sweep[j]`, `CZ rec[-a] sweep[j]`, and `CZ sweep[j] rec[-a]` analyzer no-op behavior remains part of the selected classical-only `CZ` analyzer matrix.

## Owned Negative Scope

- Selected non-`CZ` record-record controlled-Pauli groups remain fail-closed: `CX` and `CY` reject a record second target, and `XCZ` and `YCZ` reject a record first target.
- Other non-`CZ` record-record controlled-Pauli spellings are rejected by gate target validation before analyzer execution.
- Non-`CZ` record-sweep and sweep-record groups remain governed by the existing invalid target-position rejection tests.
- No `detect --sweep`, typed detector-sampler sweep API, Python API, JS/WASM, diagram, GPU, or deprecated `--detector_hypergraph` surface is added.
- Broader analyzer sweep-shape parity and broader legal non-tableau gate execution remain active PFM3 work.

## Tests

- `cargo test -p stab-core --test dem_analyzer_classical sweep --quiet`
- `cargo test -p stab-cli analyze_errors_sweep_controls --quiet`
- `cargo test -p stab-oracle fixtures --quiet`

The executable evidence lives in `crates/stab-core/tests/dem_analyzer_classical.rs` and `crates/stab-cli/src/tests/m10.rs`.
The oracle metadata rows are `pf3-analyze-errors-sweep-core` and `pf3-analyze-errors-sweep-cli`.

## Benchmarks

No benchmark row is added or promoted for this slice.
The existing `pf3-analyze-errors-sweep` benchmark is a non-primary report-only Rust runner for the selected analyzer sweep-control and `CZ` classical-only no-op matrix.
Because this slice only adds exact target-shape evidence to an existing tiny analyzer matrix and has no faithful isolated pinned-Stim timing surface, it must not be cited as a primary performance gate.

## Documentation And Metadata To Sync

- `docs/stab-feature-checklist.md`
- `docs/plans/non-deferred-partial-feature-milestones.md`
- `docs/plans/partial-feature-inventory.md`
- `docs/plans/rpf3-sweep-gate-progress-report.md`
- `docs/plans/rust-stim-drop-in-rewrite.md`
- `benchmarks/manifest.csv`
- `oracle/fixtures/manifest.csv`
