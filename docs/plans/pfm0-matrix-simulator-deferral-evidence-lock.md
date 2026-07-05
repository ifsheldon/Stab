# PFM0 Matrix And Simulator Deferral Evidence Lock

## Summary

This PFM0 reconciliation slice closes the `Conversion to or from numpy, state vectors, and arbitrary unitaries` checklist row as a deferred surface with scoped Rust evidence.
The previous `Deferred or partial` wording mixed three different things: implemented Rust `unitary_to_tableau` semantics, scoped graph/vector simulator cross-check evidence, and intentionally deferred Python/numpy/state-vector/API parity.
No runtime behavior changes in this slice.

## Closed Row

| Checklist row | Current status | Closure boundary |
| --- | --- | --- |
| `Conversion to or from numpy, state vectors, and arbitrary unitaries` | Deferred with scoped Rust subset | Stab keeps the M6-owned `unitary_to_tableau` Rust subset and M12 semantic graph/vector cross-checks, while Python/numpy conversion APIs, state-vector APIs, arbitrary unitary conversion APIs, `tableau_to_unitary`, random tableau/unitary round trips, and public graph/vector simulator products remain deferred. |

## Evidence

Implemented Rust semantic evidence:

- `unitary_to_tableau` covers the M6 algebra-only subset from pinned Stim's `stabilizers_vs_amplitudes` tests.
- The M6 test covers all 46 canonical known-unitary gate-data matrices, selected controlled-gate endian behavior, Stim-style phase smoothing, and non-Clifford or malformed matrix rejection.
- `crates/stab-core/tests/simulator_cross_checks.rs` covers scoped graph-state normal-form and vector-state examples adapted from pinned Stim's graph and vector simulator tests, but only as semantic cross-checks against Stab tableau behavior plus test-local little-endian amplitude expectations.

Source-owned tests and oracle checks:

- `cargo test -p stab-core --test stabilizers_vs_amplitudes --quiet`
- `cargo test -p stab-core --test simulator_cross_checks --quiet`
- `just oracle::run --milestone M6 --structural`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::list`
- `just oracle::matrix --check`

Supporting reports:

- `docs/plans/m6-completion-report.md` records the M6-owned `unitary_to_tableau` subset and explicitly defers full state-vector round trips, `tableau_to_unitary`, and amplitude-simulator parity.
- `docs/plans/m12-progress-report.md` records the scoped graph/vector simulator cross-checks and explicitly states that public graph or vector simulator APIs remain outside M12.
- `docs/plans/rust-stim-drop-in-rewrite.md` scopes `unitary_to_tableau` to the algebra subset and keeps public graph/vector simulator APIs out of the active milestones.
- `docs/plans/stim-test-porting-plan.md` maps graph/vector simulator tests to scoped semantic cross-checks instead of public simulator API parity.
- `oracle/compatibility-matrix.csv` classifies `cpp-amplitudes` as deferred Future matrix/state-vector work instead of active completed-M12 work.
- `oracle/fixtures/manifest.csv` keeps `coverage-util-top-stabilizers-vs-amplitudes` scoped to the M6 `unitary_to_tableau` subset and names tableau-to-unitary, amplitude-simulator, and state-vector API parity as deferred beyond the current Rust and CLI GOAL.

## Deferred Boundaries

The following surfaces remain intentionally outside this evidence lock:

- Python `PauliString.from_numpy`, `PauliString.to_numpy`, `PauliString.from_unitary_matrix`, `PauliString.to_unitary_matrix`, `Tableau.from_numpy`, `Tableau.to_numpy`, `Tableau.from_state_vector`, `Tableau.to_state_vector`, `Tableau.from_unitary_matrix`, and `Tableau.to_unitary_matrix` APIs.
- Full numpy array shape, dtype, endian, mutability, and error-message parity.
- Full state-vector API parity and arbitrary unitary synthesis.
- `tableau_to_unitary` and random tableau/unitary round trips.
- Public graph simulator, vector simulator, `TableauSimulator`, and `FlipSimulator` products.
- Python bindings and Python class operator ergonomics.

## Documentation Updates

This slice updates `docs/stab-feature-checklist.md` so the matrix/state-vector conversion row no longer appears as active non-deferred work.
The active plan and inventory point to this evidence lock when explaining why scoped Rust semantic tests do not imply a public numpy, state-vector, arbitrary-unitary, or simulator-product milestone.

## Verification

Before committing this slice, run the targeted checks listed in the Evidence section plus:

```sh
just bench::list
cargo fmt --all --check
just maintenance::pre-commit
```

Milestone-audit should verify that this document does not close Python/numpy API parity, `tableau_to_unitary`, state-vector APIs, arbitrary unitary conversion, or public graph/vector simulator products.
Full-code-review should verify that the checklist wording does not overclaim beyond the selected M6 and M12 Rust semantic evidence.
