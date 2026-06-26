# M4 Completion Report

## Scope

Milestone M4 implements the `.stim` circuit data model, gate metadata, parser, validator, canonical printer, typed argument boundaries, target helpers, and M4-owned oracle and benchmark evidence for Stim v1.16.0 parity.

## Tests And Benchmarks

- Ported or created Rust tests in `crates/stab-core/tests/stim_format.rs` for parser/printer fixtures, gate aliases and canonical table metadata, inverse metadata, target parsing and classification, Pauli-product grouping, disjoint target segmentation, probability validation, typed instruction argument accessors, Stim-compatible float printing, lowercase Pauli targets, 24-bit target bounds, and large observable ids.
- Added `crates/stab-core/tests/parser_fuzz.rs` as the ignored local parser fuzz smoke target and `just rust::parser-fuzz` as its dispatch command.
- Added direct Rust oracle rows in `oracle/fixtures/manifest.csv` for M4 parser/printer, gate metadata, target, circuit instruction, decomposition subset, and probability validation coverage.
- Added M4 Stab-side benchmark runners in `ops/bench/src/baseline.rs` for dense parser throughput, sparse parser throughput, canonical print timing, and gate lookup timing.
- Tightened `bench compare --strict` so selected rows fail when a Stab runner is pending or the selected baseline report is missing the row.

## Implementation Areas

- `crates/stab-core/src/circuit.rs` owns `Circuit`, `CircuitInstruction`, repeat blocks, canonical `.stim` printing, typed argument accessors, target segmentation helpers, and Stim-compatible float formatting.
- `crates/stab-core/src/gate.rs` owns the v1.16.0 gate table, aliases, categories, inverse metadata, argument rules, and target validation.
- `crates/stab-core/src/ids.rs` and `crates/stab-core/src/target.rs` own typed IDs, probabilities, repeat counts, target parsing, target display, target accessors, lowercase Pauli parsing, and Stim's 24-bit target text boundary.
- `ops/bench/src/baseline.rs` owns M4 Stab comparison runners and strict comparison completeness checks.

## Done Criteria Status

- `just oracle::run --milestone M4` passes all seven M4 rows.
- `cargo test -p stab-core parser` and `cargo test -p stab-core gates` are covered by the full `cargo test --workspace` run and the targeted `stim_format` run.
- `just rust::parser-fuzz` passes the ignored local parser fuzz smoke.
- `just bench::compare --milestone M4 --strict` reports dense parser and sparse parser timings against the pinned C++ Stim baseline, gate lookup against the pinned C++ Stim baseline, and Stab-only canonical print timing against the explicit contract-only printer row.

## Audit And Review Outcome

- Milestone audit found that M4 canonical printer benchmark evidence was contract-only, top-level algorithm rows were assigned too early, and M4-owned probability and gate-decomposition utility scope needed narrowing.
- Implementation issues from milestone audit were fixed by moving top-level rows to M6 or M9, documenting the contract-only printer benchmark, adding typed instruction argument accessors, and keeping unresolved scope questions in the under-specification log.
- Full code review found four implementation issues: Stim-compatible float formatting, 24-bit target parsing and lowercase Pauli support, comparable dense and sparse parser benchmark runners, and `u64` observable IDs.
- All full-code-review implementation findings were fixed in code and covered by focused tests or benchmark output.

## Verification Commands

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo test -p stab-core --test stim_format`
- `cargo test -p stab-bench`
- `just oracle::run --milestone M4`
- `just bench::compare --milestone M4 --strict`
- `just rust::parser-fuzz`

## Open Under-Specification Entries

- `docs/plans/milestone-spec-gaps.md` keeps `M4: Gate Decomposition Utility Scope` open for full MPP, SPP, pair-measurement, and base-gate decomposition behavior that depends on later tableau or simulator semantics.
- `docs/plans/milestone-spec-gaps.md` keeps `M4: Probability Utility Fixture Scope` open for random hit-index sampling and biased random bit generation that belong with later RNG, bit-storage, or sampler APIs.
