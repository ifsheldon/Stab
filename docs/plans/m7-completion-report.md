# M7 Completion Report

## Milestone

M7: Circuit Generation And Early CLI.

Objective: ship the first useful `stim`-compatible CLI surface and deterministic generated-circuit fixtures for later simulator, detector, and benchmark work.

## Status

Complete against the clarified M7 contract.

M7 implements the public `stab gen` and staged `stab convert` surfaces, exact oracle rows for the supported generator family/task command shapes, structural generator tests for larger noisy circuits, CLI boundary tests for Stim-compatible argument behavior, source-owned benchmark rows for generated-on-demand primary circuits, and user-facing CLI documentation.
Strict external CLI-vs-CLI benchmark thresholds remain M12 performance-hardening work by roadmap decision.

## Tests Ported Or Created

- Added exact M7 oracle rows for `gen` repetition-code memory, rotated and unrotated surface-code memory tasks, color-code memory, the legacy `--gen` spelling, Stim's accepted ignored `gen --in` flag, `gen` argument rejection cases, `gen --rounds` i64-bound rejection, `convert 01` to `dets`, and `convert --bits_per_shot` to `dets` rejection.
- Added `crates/stab-cli/src/tests.rs` golden-output tests for every supported M7 `gen` family and task, the legacy `--gen` spellings, `convert 01` to `dets`, `convert --in_format=stim --out_format=stim`, path-based `.stim` conversion, Stim-compatible `gen --in`, Stim-compatible `gen --rounds` bounds, and `convert --bits_per_shot ... --out_format=dets` rejection.
- Added structural generator tests in `crates/stab-core/tests/circuit_generation.rs` adapted from Stim v1.16.0 generator tests for repetition, rotated surface, unrotated surface, and color-code noisy reference circuits plus invalid distance and round validation.
- Added `ops/oracle` validation coverage for nonzero-status exact oracle rows without committed stdout goldens, so rejection fixtures can compare live Stim and Stab status plus stderr class without inventing empty golden files.

## Implementation Areas

- `crates/stab-cli/src/lib.rs` implements `gen`, legacy `--gen`, Stim-compatible `gen --in`, Stim-compatible i64-bound parsing for `gen --rounds`, `.stim` canonical conversion, the staged `01` result conversion subset, and Stim-style process failure status for Clap parse errors.
- `crates/stab-core/src/circuit_generation.rs` owns typed `CodeDistance`, `RoundCount`, generation parameters, `GeneratedCircuit`, and repetition, surface, and color-code generation entry points.
- `oracle/fixtures/manifest.csv` records the M7 exact and structural oracle rows.
- `benchmarks/manifest.csv` records the M7 generated-on-demand benchmark matrix and report-only comparability notes.
- `README.md` documents the implemented M7 `gen` and `convert` CLI surface and explicit conversion deferrals.
- `docs/plans/rust-stim-drop-in-rewrite.md` clarifies M7's generated-circuit matrix strategy and report-only benchmark acceptance.

## Done Criteria

| Requirement | Status | Evidence |
| --- | --- | --- |
| Add `stab-cli` with a development binary invokable as `stab` | Satisfied | `crates/stab-cli/src/bin/stab.rs`; `cargo test -p stab-cli --test binary` |
| Implement `stim gen` compatibility for supported families and tasks | Satisfied | `crates/stab-cli/src/lib.rs`; `crates/stab-core/src/circuit_generation.rs`; `just oracle::run --milestone M7`; `cargo test -p stab-cli gen`; `cargo test -p stab-core circuit_generation` |
| Implement supported `gen` flags with typed parsing and validation | Satisfied | `gen --in` oracle row, argument rejection oracle rows, `cargo test -p stab-cli arg_parse`, `cargo test -p stab-cli cli_rejects_rounds_past_stim_i64_cli_bound` |
| Implement `stim convert` for `.stim` parse and canonical print workflows | Satisfied | `run_convert_stim`; `convert_stim_from_stdin_to_canonical_output`; `convert_stim_reads_and_writes_paths`; M4 parser/printer tests |
| Store generated-circuit fixture matrix for later reuse | Satisfied by clarified source-owned manifests | `oracle/fixtures/manifest.csv`; `benchmarks/manifest.csv`; `crates/stab-core/tests/circuit_generation.rs`; `docs/plans/rust-stim-drop-in-rewrite.md` |
| `just oracle::run --milestone M7` passes exact-output `gen` and `convert` cases | Satisfied | `just oracle::run --milestone M7` |
| `stab-cli gen` output matches Stim v1.16.0 for the M7 acceptance matrix | Satisfied | exact oracle rows for supported family/task commands plus structural generator tests for larger noisy cases |
| `stab-cli convert` reads stdin and files and emits canonical `.stim` output for supported circuits | Satisfied | `cargo test -p stab-cli convert`; README M7 CLI docs |
| `just bench::compare --milestone M7` reports generator and convert throughput | Satisfied as report-only | `just bench::baseline --only M7 --out target/benchmarks/m7-completion-baseline`; `just bench::compare --milestone M7 --baseline target/benchmarks/m7-completion-baseline/baseline.json --report target/benchmarks/m7-completion-compare` |

## Audit Outcome

Milestone audit found missing `gen --in` compatibility, missing `gen --rounds` i64-bound validation, under-specified generated-circuit matrix scope, report-only benchmark ambiguity, and missing public CLI docs.
The implementation issues were fixed with new CLI parsing behavior and oracle/test rows.
The specification issues were resolved in `docs/plans/rust-stim-drop-in-rewrite.md` and `docs/plans/milestone-spec-gaps.md`.
The documentation issue was fixed in `README.md`.

Resolved M7 spec entries:

- `2026-06-27 - M7: Generated Fixture Matrix Scope`
- `2026-06-27 - M7: Convert Command Circuit Versus Result-Format Scope`
- `2026-06-27 - M7: Generator Benchmark Comparability`

## Full Code Review Outcome

Full code review found that `convert --bits_per_shot ... --out_format=dets` silently dropped data, `gen --in` was rejected even though Stim accepts it, and Clap parse failures returned non-Stim status code 2.
These were fixed by rejecting raw-width `dets` output, accepting hidden no-op `gen --in`, applying Stim-compatible i64 bounds to `gen --rounds`, and returning status 1 for parse failures.
Focused regression tests and M7 oracle rows cover each fixed behavior.

Residual review risk: `crates/stab-cli/src/lib.rs` and `crates/stab-core/src/sampling.rs` are watch-list files near the 1200-line large-file threshold, but neither crosses the project threshold in this slice.

## CQ2 Generation Qualification Addendum

The later CQ2 source-ownership pass replaces M7's broad structural generation evidence with focused exact owners for the complete pinned repetition, surface, and color cases; the generator parameter and value-object contracts; private helper semantics; the complete upstream no-noise detector-count matrix; invalid color-family combinations; and materialization admission. The Python-only `Circuit.generated` string-dispatch error symbol remains deferred with Python bindings, while portable typed Rust constraints are owned independently.

Materializing generator entry points now preflight an exact projected physical-qubit count against a 131,072 limit before building coordinate maps, instruction vectors, layout text, or CLI output. Repetition-code distance 2047 remains accepted; rotated surface distance 256 is the last accepted and 257 projects 132,097 qubits; unrotated surface distance 181 is the last accepted and 182 projects 131,769 qubits; color-code distance 341 is the last valid accepted distance and 343 projects 132,355 qubits. CLI regression coverage proves rejection occurs before `--out` is created. Pinned Stim accepts nominal distances through 2047, so this is an explicit bounded-materialization difference for surface and color families.

The qualification tests also preserve folded maximum-round behavior and deterministically analyze every valid upstream matrix cell into an error-free detector error model. Representative sampled execution for every task, a 256-shot multiword batch, and the pinned Python shape retain the sampler contract without turning a deterministic proof into a multi-minute probabilistic loop.

## Verification Commands

- `cargo fmt --check --all`
- `cargo clippy -p stab-core -p stab-cli -p stab-oracle --all-targets -- -D warnings`
- `cargo test -p stab-core --test circuit_generation --quiet`
- `cargo test -p stab-core --lib circuit_generation::tests:: --quiet`
- `cargo test -p stab-cli --quiet`
- `cargo test -p stab-cli gen --quiet`
- `cargo test -p stab-cli convert --quiet`
- `cargo test -p stab-cli arg_parse --quiet`
- `cargo test -p stab-cli cli_rejects_rounds_past_stim_i64_cli_bound --quiet`
- `cargo test -p stab-cli sample_rejects_values_past_stim_i64_cli_bound --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `cargo test -p stab-oracle repository_fixture_manifest_passes_validation --quiet`
- `cargo test -p stab-oracle validation_allows_failure_exact_rows_without_expected_stdout --quiet`
- `just oracle::matrix --check`
- `just oracle::run --milestone M7`
- `just oracle::record --check-clean`
- `just bench::baseline --only M7 --out target/benchmarks/m7-completion-baseline`
- `just bench::compare --milestone M7 --baseline target/benchmarks/m7-completion-baseline/baseline.json --report target/benchmarks/m7-completion-compare`
- `git diff --check`
